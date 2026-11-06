//! The chip's blocks, wired together as `hdl/rtl/dmg_cpu_b.vhd` wires them,
//! edge by edge of the crystal.
//!
//! The clock_out and resets, the divider and the timer, IF, OAM DMA, LCDC,
//! the line timing, the OAM scan, OAM, render, the sprite store, VRAM's
//! bus, the bus side (the external bus, HRAM, the boot ROM's mapping, the
//! joypad, the serial port), the APU and the CPU core. What comes in with
//! each edge (`ChipInputs`) is the pins' (the clock, the reset, the test
//! modes) and the world outside's (the joypad's lines, SIN, SCK); the
//! memories (VRAM, the board's work RAM and cartridge, the boot ROM) are
//! the edge's to borrow. Every read gets its byte from the blocks that
//! drive the data bus, IE's (the core's) among them, or the precharged FF
//! where nothing does.
//!
//! A step is the edge that starts a phase, and what happens in that phase:
//! the flip-flops clocked by the edge take what was before it, the latches
//! follow, and the CPU's access in the phase reaches the registers. The
//! core settles twice: first with the data bus and IF as the phase finds
//! them, which gives the blocks its address, its strobes and what it
//! writes; then with what the blocks drive on the data bus and IF as they
//! leave it.

use crate::{
    Phase,
    apu::{ApuInputs, NetApu},
    bus::{BusInputs, BusOutputs, NetBoard, NetBus},
    clocks::{ClockInputs, ClockOutputs, NetClocks},
    dma::NetDma,
    interrupts::{NetIf, TIMER},
    latch_race::LatchRace,
    lcd_control::NetLcdControl,
    lcd_timing::NetLcd,
    oam::OamInputs,
    oam_scan::{Clocks as ScanClocks, NetScan},
    render::{NetRender, PinLog, RenderInputs},
    sm83::{CoreInputs, CoreOutputs, HighByte, NetCore},
    timer::{Access, NetTimer, Reg},
};

/// IF's VBlank and STAT bits.
const VBLANK: u8 = 0x01;
const STAT: u8 = 0x02;
const SERIAL: u8 = 0x08;
const JOYPAD: u8 = 0x10;

/// What LCDC gives the line timing and the scan.
#[derive(Clone, Copy)]
struct Lcdc {
    /// The PPU's reset: the LCD is off, or the chip is reset.
    ppu_reset: bool,
    /// Sprites of 16 lines.
    tall: bool,
}

/// The chip's pins at an edge, and the world outside in the phase it
/// starts.
#[derive(Clone, Copy, Debug)]
#[expect(clippy::struct_excessive_bools, reason = "the pins' levels")]
pub struct ChipInputs {
    /// The clock, the reset and the test pins as they are after the edge.
    pub ck1: bool,
    pub reset_pin_n: bool,
    pub t1: bool,
    pub t2: bool,
    pub test_reset: bool,
    /// The world outside: the joypad's lines, SIN, and SCK as the other end
    /// of the link drives it.
    pub p10_13: u8,
    pub sin: bool,
    pub sck: bool,
}

flat_eq!(ChipInputs {
    ck1,
    reset_pin_n,
    t1,
    t2,
    test_reset,
    p10_13,
    sin,
    sck,
});

impl ChipInputs {
    /// The half periods of the crystal for which the board holds /RESET low
    /// after the power-on (`hdl/board/gameboy.vhd`: 64 periods); it rises
    /// with the edge that ends them.
    pub const RESET_EDGES: u64 = 128;

    /// The pins at the crystal's `edge`-th edge after the power-on (0: as
    /// it starts), as the board drives them: the crystal toggles CK1 at
    /// each edge, /RESET is low for `RESET_EDGES`, the test pins are low,
    /// no button is pressed (the joypad's lines float, and the chip pulls
    /// them up), and nothing is on the link.
    // @hw die: the board's crystal and reset (gameboy.vhd)
    #[must_use]
    pub const fn board(edge: u64) -> Self {
        Self {
            ck1: edge % 2 == 1,
            reset_pin_n: edge >= Self::RESET_EDGES,
            t1: false,
            t2: false,
            test_reset: false,
            p10_13: 0x0F,
            sin: true,
            sck: true,
        }
    }
}

/// What the blocks take in a phase: the pins, and the core's outputs.
#[derive(Clone, Copy)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "the pins' and strobes' levels"
)]
struct PhaseInputs {
    /// The test pins, and the core's write strobe as it was at the edge.
    t1: bool,
    t2: bool,
    write: bool,
    /// The CPU core's access in the phase: its address, its read, the
    /// chip's data bus (and the bus while render's mode 3 is high: STAT's
    /// mode bits, in a read of it), its acknowledges.
    addr: u16,
    read: bool,
    data: u8,
    data_mode3: u8,
    acks: u8,
    /// The core's request (gated by the chip's decoding of the address)
    /// and write strobe as they are in the phase.
    mreq_in_phase: bool,
    write_in_phase: bool,
    /// What drives the address's high byte, and the core's decoder's
    /// request (`ctl_mreq`, which the chip's decodings gate into `mreq`), in
    /// the phase: they decide whether the request pulses for a moment as an
    /// access starts.
    high_byte: HighByte,
    ctl_mreq: bool,
    /// The world outside: the joypad's lines, SIN, and SCK as the other end
    /// of the link drives it.
    p10_13: u8,
    sin: bool,
    sck: bool,
}

/// The memories the chip's edge borrows: VRAM, the board (its work RAM and
/// cartridge) and the boot ROM's image.
pub struct ChipMemories {
    pub vram: Box<[u8; 0x2000]>,
    pub board: NetBoard,
    pub boot_rom: Vec<u8>,
}

/// The chip's transcribed blocks, between edges of the crystal. VRAM, on
/// the board, is the edge's to borrow.
#[derive(Clone, Copy)]
#[expect(clippy::struct_excessive_bools, reason = "the strobes' last levels")]
pub struct NetChip {
    pub clocks: NetClocks,
    pub timer: NetTimer,
    pub interrupts: NetIf,
    pub dma: NetDma,
    /// `dma_phi` in the last phase.
    pub dma_phi: bool,
    pub control: NetLcdControl,
    pub lcd: NetLcd,
    pub scan: NetScan,
    pub render: NetRender,
    /// The LCD's pins as render changed them in the last edge.
    pub render_pins: PinLog,
    /// The bus side: the external bus, HRAM, the boot ROM's mapping, the
    /// joypad, the serial port.
    pub bus: NetBus,
    pub apu: NetApu,
    /// The chip's write strobe in the phase (`cpu_wr_sync`).
    pub cpu_wr: bool,
    /// The serial port's and the joypad's interrupt lines in the last phase.
    pub int_serial: bool,
    pub int_jp: bool,
    /// The CPU's write strobe and the data bus in the last phase: the
    /// latches a write opens take the bus as it was first.
    pub wr_before: bool,
    pub data_before: u8,
    /// The CPU core, and its outputs as the last phase ended: what the
    /// clocks take at the edge, and what the chip decodes until the new
    /// ones reach it.
    pub core: NetCore,
    pub core_out: CoreOutputs,
    /// The bits of the data bus the core pulled low in the last phase, and
    /// those the blocks drove low and high: each drives them for a while
    /// after the edge.
    pub core_low_before: u8,
    pub blocks_low_before: u8,
    pub blocks_high_before: u8,
    /// What a read of LCDC, STAT, LY or LYC has taken so far: the core's data bus
    /// is precharged high and only pulled low while the read drives it,
    /// from E until it takes it in H.
    pub read_bus: Option<u8>,
    /// IF's read latch: the flags while the CPU does not read IF, held while
    /// it does (a flag that rises in the read's last phase is still not
    /// driven as the next begins).
    pub if_latch: u8,
}

/// `first` if a block drives it, else `second`.
const fn first_of(first: Option<u8>, second: Option<u8>) -> Option<u8> {
    match first {
        Some(_) => first,
        None => second,
    }
}

/// What a step does that the CPU and the other blocks see.
#[derive(Clone, Copy)]
#[expect(clippy::struct_excessive_bools, reason = "the strobes' levels")]
pub struct ChipEvents {
    /// The interrupt requests that rose in the phase: the timer's, the STAT
    /// line's, VBlank's.
    pub timer_rose: bool,
    pub stat_rose: bool,
    pub vblank_rose: bool,
    /// Mode 2 for OAM and STAT: the scan runs and the DMA does not hide it.
    pub mode2: bool,
    /// The OAM scan's save strobes, bit `i` for slot `i`.
    pub save: u16,
    /// The byte a read takes, in H.
    pub read: Option<u8>,
    /// What the blocks drive on the data bus in the phase, for a read.
    pub driven: Option<u8>,
    /// The bits of the data bus pulled strongly low in the phase, which the
    /// core's bridge passes.
    pub data_low: u8,
    /// The core's outputs as the phase ends.
    pub core: CoreOutputs,
    /// The clocks decoded from the phase.
    pub clocks: ClockOutputs,
    /// What the bus side gave in the phase: the pins, its interrupts.
    pub bus: BusOutputs,
}

impl NetChip {
    /// The chip at power-on, as the VHDL's initial values have it: most
    /// flip-flops and latches low, the keepers and precharged lines high.
    /// The core's outputs have not reached the chip yet: only STOP's
    /// enable (`osc_ena`) is high, as the board pulls it. Then the wires
    /// and latches settle on that state and on the pins as they are
    /// (`pins`, before the crystal's first edge), with no edge for any
    /// flip-flop: the VHDL's signals start undefined, and a rise from
    /// undefined is no edge. /RESET low sets ASOL at once.
    #[must_use]
    pub fn power_on(pins: ChipInputs, vram: &[u8; 0x2000]) -> Self {
        let mut chip = Self {
            clocks: NetClocks::POWER_ON,
            timer: NetTimer::POWER_ON,
            interrupts: NetIf::POWER_ON,
            dma: NetDma::POWER_ON,
            dma_phi: false,
            control: NetLcdControl::POWER_ON,
            lcd: NetLcd::POWER_ON,
            scan: NetScan::POWER_ON,
            render: NetRender::power_on(),
            render_pins: PinLog::default(),
            bus: NetBus::power_on(),
            apu: NetApu::power_on(),
            cpu_wr: false,
            int_serial: false,
            int_jp: false,
            wr_before: false,
            data_before: 0xFF,
            core: NetCore::POWER_ON,
            core_out: CoreOutputs {
                stop_n: true,
                db: 0xFF,
                ..CoreOutputs::default()
            },
            core_low_before: 0,
            blocks_low_before: 0,
            blocks_high_before: 0,
            read_bus: None,
            if_latch: 0,
        };
        let clock_inputs = chip.clock_inputs(pins);
        chip.clocks.step(clock_inputs);
        let clock_out = chip.clocks.outputs(clock_inputs);
        let core_inputs = chip.core_inputs(clock_out, pins, 0, 0);
        chip.core.settle(&core_inputs);
        chip.apu.settle_wires();
        chip.render.settle_wires(vram);
        chip
    }

    /// An edge of the crystal and the phase it starts.
    pub fn edge(&mut self, pins: ChipInputs, memories: &mut ChipMemories) -> ChipEvents {
        // LCDC as the edge finds it: its latches open after the edge.
        // @fact lcdc-write-latch-delay
        let lcdc_at_edge = Lcdc {
            ppu_reset: self.control.ppu_reset(self.clocks.reset2()),
            tall: self.control.tall(),
        };
        // The clock_out, with the divider's last bit and the core's outputs
        // as they were at the edge.
        let clock_inputs = self.clock_inputs(pins);
        self.clocks.step(clock_inputs);
        let phase = self.clocks.phase();
        let clock_out = self.clocks.outputs(clock_inputs);
        self.cpu_wr = clock_out.cpu_wr_sync;

        // The core, with the data bus and IF as the phase finds them.
        let kept = self.core.db;
        let first = self.core.phase(&self.core_inputs(
            clock_out,
            pins,
            self.core_out.addr,
            self.blocks_low_before,
        ));
        self.interrupts.ie = self.core.ints.ie;
        let mut inputs = self.phase_inputs(clock_out, pins, first);

        let timer_rose = self.step_timer(phase, clock_out, inputs);
        self.step_apu(clock_out, inputs);
        self.step_dma(clock_out, inputs);
        let bus = self.step_bus(phase, clock_out, inputs, memories);
        let write_lcdc = if self.cpu_wr & (inputs.addr == 0xFF40) {
            Some(inputs.data)
        } else {
            None
        };
        self.control.settle(clock_out.reset2, write_lcdc);
        let lcdc = Lcdc {
            ppu_reset: self.control.ppu_reset(clock_out.reset2),
            tall: self.control.tall(),
        };
        // Mode 0 as the edge finds it (render's, which changes after the
        // line timing's flip-flops).
        let mode0_at_edge = self.render.mode0();
        let lcd_before = self.lcd;
        let (mut stat_rose, level, vblank_rose) =
            self.step_lcd(lcdc_at_edge, lcdc, mode0_at_edge, inputs);
        let (mode2, save) = self.step_scan(lcd_before, lcdc_at_edge, lcdc);
        // OAM's data lines follow the data bus while the PPU leaves them: it
        // has a read's byte by now, but VRAM's and OAM's own, and STAT's
        // mode bits follow render's mode 3 as it steps.
        if inputs.read {
            let not_mode3 = self.drivers(mode2, false, inputs.addr, bus);
            let mode3 = self.drivers(mode2, true, inputs.addr, bus);
            inputs.data = Self::data(clock_out, pins, first, not_mode3);
            inputs.data_mode3 = Self::data(clock_out, pins, first, mode3);
        }
        self.step_render(clock_out, lcdc, save, inputs, bus, &mut memories.vram);
        // Then mode 0, which rises after the edge's flip-flops: for a
        // moment only, if a sprite at X 167 stops mode 3 again.
        let at_edge = self.lcd.int_stat(mode0_at_edge);
        let pulse = self.render.mode0_pulse & self.lcd.int_stat(true);
        let now = self.lcd.int_stat(self.render.mode0());
        stat_rose |= (at_edge & !level) | (pulse & !at_edge) | (now & !at_edge);

        // IF: the sources' rises, the CPU's write and the acknowledges.
        let rises = if bus.int_serial & !self.int_serial {
            SERIAL
        } else {
            0
        } | if bus.int_jp & !self.int_jp { JOYPAD } else { 0 }
            | if timer_rose { TIMER } else { 0 }
            | if stat_rose { STAT } else { 0 }
            | if vblank_rose { VBLANK } else { 0 };
        let write_if = if self.cpu_wr & (inputs.addr == 0xFF0F) {
            Some(inputs.data)
        } else {
            None
        };
        self.interrupts.phase(rises, write_if, inputs.acks);
        self.int_serial = bus.int_serial;
        self.int_jp = bus.int_jp;

        let (read, driven) = self.step_read(phase, mode2, inputs, bus);
        // IF's read latch follows the flags until a read of IF starts, and
        // holds what it found until it ends.
        if !(inputs.read & (inputs.addr == 0xFF0F)) {
            self.if_latch = self.interrupts.flags;
        }

        // The core again, with what the blocks drive on the data bus and IF
        // as they leave it.
        let data_low = self.data_low(clock_out, pins, first, driven);
        let core = self.core.settle_from(
            &self.core_inputs(clock_out, pins, first.addr, data_low),
            kept,
        );
        self.interrupts.ie = self.core.ints.ie;
        self.wr_before = self.cpu_wr;
        self.data_before = Self::data(clock_out, pins, core, driven);
        self.core_out = core;
        ChipEvents {
            timer_rose,
            stat_rose,
            vblank_rose,
            mode2,
            save,
            read,
            driven,
            data_low,
            core,
            clocks: clock_out,
            bus,
        }
    }

    /// The core drives the data bus: it is not precharging it (phase A),
    /// and the test mode T1 alone does not release it.
    // @hw die: the core's data bus drivers (sm83.vhd: d)
    const fn core_drives(clock_out: ClockOutputs, pins: ChipInputs) -> bool {
        clock_out.exec_phase & !(pins.t1 & !pins.t2)
    }

    /// The bits of the data bus pulled strongly low in the phase: by a
    /// block, or by the core where no block drives a 1 against it (the two
    /// fight to an X, which the core's bridge does not pass). Each also
    /// drives what it did in the phase before for a while after the edge:
    /// the core's outputs reach the bus 39 ns after it.
    // @hw die: DBUS_BRIDGE (b2b_wand_inj_a)
    const fn data_low(
        &mut self,
        clock_out: ClockOutputs,
        pins: ChipInputs,
        core: CoreOutputs,
        driven: Option<u8>,
    ) -> u8 {
        let (now_low, now_high) = match driven {
            Some(byte) => (!byte, byte),
            None => (0, 0),
        };
        let core_low = if Self::core_drives(clock_out, pins) {
            !core.db
        } else {
            0
        };
        let blocks_low = now_low | self.blocks_low_before;
        let blocks_high = now_high | self.blocks_high_before;
        let low = blocks_low | ((core_low | self.core_low_before) & !blocks_high);
        self.core_low_before = core_low;
        self.blocks_low_before = now_low;
        self.blocks_high_before = now_high;
        low
    }

    /// The data bus as the phase ends: what the core and the blocks drive
    /// (where they fight, an X, taken as 0), else the precharge.
    const fn data(
        clock_out: ClockOutputs,
        pins: ChipInputs,
        core: CoreOutputs,
        driven: Option<u8>,
    ) -> u8 {
        let blocks = match driven {
            Some(byte) => byte,
            None => 0xFF,
        };
        if Self::core_drives(clock_out, pins) {
            core.db & blocks
        } else {
            0xFF
        }
    }

    /// The chip decodes FE00 to FFFF and the boot ROM's addresses: the
    /// core's request does not leave it then (unless in the test mode T2
    /// alone, for the boot ROM).
    // @hw die: TUNA SYRO, TULO TUTU; ZLYZ (oa211)
    const fn internal_access(&self, addr: u16) -> (bool, bool) {
        (addr >> 9 == 0x7F, !self.bus.unmapped & (addr >> 8 == 0))
    }

    /// What the core takes in the phase: the clocks, the resets, the
    /// joypad's wake-up, the test modes, the chip's decoding of its address
    /// (`addr`), IF, and the data bus's bits pulled low (`data_low`).
    const fn core_inputs(
        &self,
        clock_out: ClockOutputs,
        pins: ChipInputs,
        addr: u16,
        data_low: u8,
    ) -> CoreInputs {
        let (internal_access, shadow_access) = self.internal_access(addr);
        CoreInputs {
            clk: clock_out.clk_1mhz,
            exec_phase: clock_out.exec_phase,
            data_phase: clock_out.data_phase,
            write_phase: clock_out.clk_t4,
            pch_phase: clock_out.pch_phase,
            sys_reset: self.clocks.cpu_sys_reset,
            pwron_reset: !pins.reset_pin_n,
            clk_ready: clock_out.osc_stable,
            wake: self.bus.jp_wakeup,
            oe_n: pins.t1 & !pins.t2,
            internal_access,
            shadow_access,
            shadow_override: pins.t2 & !pins.t1,
            int: self.interrupts.flags,
            d: !data_low,
        }
    }

    /// What the blocks take in the phase: the pins, and the core's outputs
    /// (`core`; its request gated by the chip's decoding of its address).
    const fn phase_inputs(
        &self,
        clock_out: ClockOutputs,
        pins: ChipInputs,
        core: CoreOutputs,
    ) -> PhaseInputs {
        let (internal, boot) = self.internal_access(core.addr);
        let override_boot = pins.t2 & !pins.t1;
        PhaseInputs {
            t1: pins.t1,
            t2: pins.t2,
            write: self.core_out.wr,
            addr: core.addr,
            read: core.rd,
            data: Self::data(clock_out, pins, core, None),
            data_mode3: Self::data(clock_out, pins, core, None),
            acks: core.inta & 0x1F,
            mreq_in_phase: core.ctl_mreq & !internal & (!boot | override_boot),
            write_in_phase: core.wr,
            high_byte: core.high_byte,
            ctl_mreq: core.ctl_mreq,
            p10_13: pins.p10_13,
            sin: pins.sin,
            sck: pins.sck,
        }
    }

    /// The clock_out' inputs at an edge: the pins and the CPU core's, and the
    /// divider's last bit.
    #[must_use]
    pub const fn clock_inputs(&self, pins: ChipInputs) -> ClockInputs {
        ClockInputs {
            ck1: pins.ck1,
            reset_pin_n: pins.reset_pin_n,
            t1: pins.t1,
            t2: pins.t2,
            osc_ena: self.core_out.stop_n,
            clk_ena: self.core_out.halt_n,
            mreq: self.core_out.mreq,
            write: self.core_out.wr,
            test_reset: pins.test_reset,
            div_16hz: self.timer.count & 0x8000 != 0,
        }
    }

    /// The DMA's clock: high in A to D, and while the CPU is halted.
    // @hw die: the chip's dma_phi <= not data_phase
    #[must_use]
    pub const fn dma_phi(clock_out: ClockOutputs) -> bool {
        !clock_out.data_phase
    }

    /// The divider and the timer: the A edge, then the access in E to G,
    /// and the resets (/RESET and STOP's stopped oscillator clear the
    /// divider). Whether the interrupt rose.
    const fn step_timer(
        &mut self,
        phase: Phase,
        clock_out: ClockOutputs,
        inputs: PhaseInputs,
    ) -> bool {
        let int_timer = self.timer.int_timer;
        let access = match Reg::at(inputs.addr) {
            Some(reg) if self.cpu_wr => Access::Write(reg, inputs.data),
            _ => Access::None,
        };
        self.timer.phase(phase, access);
        self.timer
            .hold(clock_out.reset | !self.core_out.stop_n, clock_out.reset2);
        self.timer.int_timer & !int_timer
    }

    /// The APU, on the clocks decoded from the phase, the resets, the
    /// divider's bit 10 as the timer leaves it, and the CPU's access.
    // @hw die: the chip's apu_control and channels' ports
    fn step_apu(&mut self, clock_out: ClockOutputs, inputs: PhaseInputs) {
        self.apu.edge(ApuInputs {
            clk: self.clocks.clk,
            data_phase: clock_out.data_phase,
            clk_t4: clock_out.clk_t4,
            pch_phase: clock_out.pch_phase,
            reset2: clock_out.reset2,
            div10: self.timer.count & 0x400 != 0,
            t1_only: inputs.t1 & !inputs.t2,
            addr: inputs.addr,
            data: inputs.data,
            cpu_rd: inputs.read,
            cpu_wr: self.cpu_wr,
        });
    }

    /// OAM DMA, on `dma_phi`.
    const fn step_dma(&mut self, clock_out: ClockOutputs, inputs: PhaseInputs) {
        let dma_phi = Self::dma_phi(clock_out);
        let write = if self.cpu_wr & (inputs.addr == 0xFF46) {
            Some(inputs.data)
        } else {
            None
        };
        if dma_phi & !self.dma_phi {
            self.dma.rise(write.is_some());
        } else if !dma_phi & self.dma_phi {
            self.dma.fall();
        } else {
            // No edge of the DMA's clock.
        }
        self.dma.settle(write);
        self.dma_phi = dma_phi;
    }

    /// The line timing: the chip clock's edge (held while LCDC resets the
    /// LCD), the CPU's write of LYC or STAT, the LCD's reset, with render's
    /// mode 0 as the edge found it. Whether the STAT line rose, its level,
    /// and whether VBlank rose.
    fn step_lcd(
        &mut self,
        at_edge: Lcdc,
        lcdc: Lcdc,
        mode0_at_edge: bool,
        inputs: PhaseInputs,
    ) -> (bool, bool, bool) {
        let mode1 = self.lcd.mode1;
        let mut rose = false;
        if self.lcd.clk != self.clocks.clk {
            if at_edge.ppu_reset {
                self.lcd.clk = self.clocks.clk;
            } else {
                rose = self.lcd.edge(mode0_at_edge);
            }
        }
        let mut level = self.lcd.int_stat(mode0_at_edge);
        if self.cpu_wr & matches!(inputs.addr, 0xFF41 | 0xFF45) {
            // As the write's latch opens, it takes the bus as it was first.
            let opens = !self.wr_before;
            for pass in 0..2 {
                if opens | (pass == 1) {
                    let data = if pass == 0 {
                        self.data_before
                    } else {
                        inputs.data
                    };
                    self.lcd.write(inputs.addr, data);
                    let now = self.lcd.int_stat(mode0_at_edge);
                    rose |= now & !level;
                    level = now;
                }
            }
        }
        if lcdc.ppu_reset {
            self.lcd.reset();
        }
        // @hw die: the chip's int_vbl <= mode1
        (rose, level, self.lcd.mode1 & !mode1)
    }

    /// The OAM scan, on the line timing's clock_out (`lcd` as the edge found
    /// them), with what OAM's latches held; then the latches. Mode 2 for
    /// OAM and STAT, and the save strobes.
    const fn step_scan(&mut self, lcd: NetLcd, at_edge: Lcdc, lcdc: Lcdc) -> (bool, u16) {
        if at_edge.ppu_reset {
            self.scan.reset();
        } else {
            let scan_clocks = ScanClocks {
                rising: self.clocks.clk,
                wuvu_before: lcd.wuvu,
                wuvu_after: self.lcd.wuvu,
                wosu_before: lcd.wosu,
                vclk: lcd.vclk,
                ly: lcd.ly,
            };
            self.scan
                .edge(scan_clocks, self.render.oam.second_even, at_edge.tall);
        }
        if lcdc.ppu_reset {
            self.scan.reset();
        }
        let save = self.scan.save(
            self.lcd.wosu,
            self.render.oam.second_even,
            self.lcd.ly,
            lcdc.tall,
        );
        let save = match save {
            Some(slot) => 1 << slot,
            None => 0,
        };
        (self.mode2(), save)
    }

    /// Mode 2, for OAM and STAT: the scan runs, and OAM DMA does not hide
    /// OAM from it.
    // @hw die: ACYL BOGE
    const fn mode2(&self) -> bool {
        OamInputs {
            scanning: self.scan.scanning,
            run: self.dma.run,
            dma_phi: false,
            wuvu: false,
            wosu: false,
            fetch_read: false,
            fetch_hold: false,
        }
        .mode2()
    }

    /// The bus side, in the phase: the external bus, HRAM, the boot ROM's
    /// mapping, the joypad and the serial port, with the board on the pins;
    /// the address pins' latches also take what the race of the core's
    /// request against the chip's decodings lets through.
    fn step_bus(
        &mut self,
        phase: Phase,
        clock_out: ClockOutputs,
        inputs: PhaseInputs,
        memories: &mut ChipMemories,
    ) -> BusOutputs {
        let bus_inputs = BusInputs {
            addr: inputs.addr,
            read: inputs.read,
            write: inputs.write_in_phase,
            mreq: inputs.mreq_in_phase,
            data: inputs.data,
            clk_t4: clock_out.clk_t4,
            exec_phase: clock_out.exec_phase,
            cpu_wr_sync: clock_out.cpu_wr_sync,
            cpu_ext_cs_en: clock_out.cpu_ext_cs_en,
            clk_1mhz: clock_out.clk_1mhz,
            starts_a: matches!(phase, Phase::A),
            reset2: clock_out.reset2,
            reset: clock_out.reset,
            dma_addr_ext: self.dma.run & !self.dma_from_vram(),
            dma_addr: u16::from_be_bytes([self.dma.source, self.dma.count]),
            div: self.timer.count,
            p10_13: inputs.p10_13,
            sin: inputs.sin,
            sck: inputs.sck,
            latch_glitch: LatchRace {
                addr_before: self.core_out.addr,
                addr: inputs.addr,
                high_byte: inputs.high_byte,
                ctl_mreq_before: self.core_out.ctl_mreq,
                ctl_mreq: inputs.ctl_mreq,
                boot_mapped: !self.bus.unmapped,
                t2_only: inputs.t2 & !inputs.t1,
            }
            .glitch(),
        };
        self.bus
            .phase(&bus_inputs, &mut memories.board, &memories.boot_rom)
    }

    /// What a read of the divider, the timer, IF or IE drives on the bus
    /// (their bits that no flip-flop drives stay precharged, 1). IF's read
    /// latch holds the flags as the read found them in E. IE is the core's,
    /// which pulls its own data bus down where IE is 0.
    // @hw die: TAWU TAKU TEMU TUSE UPUG SEPU SAWA TATU (FF04); TEDA TUBY
    // SORA, SOKU..PUSO SETE..SAPU RYLA ROTE SUPE (FF05 to FF07); MATY MOPO
    // PAVY NEJY NUTY, POLA, NELA..PEGY (FF0F); IRQ_NAND2_NAND3 (FFFF)
    const fn read_timer_if(&self, addr: u16) -> Option<u8> {
        match addr {
            0xFF04 => Some(self.timer.read(Reg::Div)),
            0xFF05 => Some(self.timer.read(Reg::Tima)),
            0xFF06 => Some(self.timer.read(Reg::Tma)),
            0xFF07 => Some(self.timer.read(Reg::Tac) | 0xF8),
            0xFF0F => Some(self.if_latch | 0xE0),
            0xFFFF => Some(self.interrupts.ie),
            _ => None,
        }
    }

    /// OAM DMA reads VRAM (sources 80 to 9F), else the cartridge bus.
    // @hw die: MUDA LOGO MORY LUMA, MUHO LUFA
    const fn dma_from_vram(&self) -> bool {
        self.dma.source >> 5 == 0b100
    }

    /// Render and the sprite store, on the PPU's clock, with what the line
    /// timing, the scan, OAM, the DMA and LCDC give them after their step;
    /// then OAM's latches, with the sprite fetch's new strobes.
    fn step_render(
        &mut self,
        clock_out: ClockOutputs,
        lcdc: Lcdc,
        save: u16,
        inputs: PhaseInputs,
        bus: BusOutputs,
        vram: &mut [u8; 0x2000],
    ) {
        let line_start = self.scan.line_start() | lcdc.ppu_reset;
        self.render.edge_with_pins(
            RenderInputs {
                clk: self.clocks.clk,
                ppu_reset: lcdc.ppu_reset,
                hard_reset: clock_out.reset2,
                hclk: self.lcd.vena,
                mode1: self.lcd.mode1,
                ly: self.lcd.ly,
                lcdc: self.control.lcdc,
                addr: inputs.addr,
                wr: self.cpu_wr,
                data: inputs.data,
                data_mode3: inputs.data_mode3,
                rd: inputs.read,
                non_vram_mreq: bus.non_vram_mreq,
                write: inputs.write,
                cpu_wr_sync: clock_out.cpu_wr_sync,
                cpu_ext_cs_en: clock_out.cpu_ext_cs_en,
                clk_t4: clock_out.clk_t4,
                vram_to_oam: self.dma.run & self.dma_from_vram(),
                dma_addr: u16::from_be_bytes([self.dma.source, self.dma.count]),
                line_start,
                stop_scan: self.scan.stop_scan() & !line_start,
                ceno: self.scan.ceno,
                save,
                // The Y comparator's difference, Y - LY - 1: its low four bits.
                scan_line: self.render.oam.second_even.wrapping_add(!self.lcd.ly) & 0x0F,
                wuvu: self.lcd.wuvu,
                wosu: self.lcd.wosu,
                dma_phi: self.dma_phi,
                dma_addr_ext: self.dma.run & !self.dma_from_vram(),
                d: bus.pins.d,
                dma_run: self.dma.run,
                dma_index: self.dma.count,
                scanning: self.scan.scanning,
                scan_index: self.scan.index,
            },
            vram,
            &mut self.render_pins,
        );
    }

    /// What drives the data bus in a read of `addr`, but VRAM and OAM: the
    /// block that decodes the address, with render's mode 3 (`mode3`) for
    /// STAT. Render's registers do not change in a read, so this holds
    /// before render steps.
    const fn drivers(&self, mode2: bool, mode3: bool, addr: u16, bus: BusOutputs) -> Option<u8> {
        let mut shown = self.control.read(addr);
        shown = first_of(shown, self.lcd.read(addr, mode2, mode3));
        shown = first_of(shown, self.render.read(addr));
        if addr == 0xFF46 {
            shown = first_of(shown, Some(self.dma.source));
        }
        shown = first_of(shown, bus.data_out);
        shown = first_of(shown, self.read_timer_if(addr));
        first_of(shown, self.apu.read())
    }

    /// A read: the AND of what drives the bus from E to H, which the core
    /// takes in H, and what the blocks drive in the phase.
    const fn step_read(
        &mut self,
        phase: Phase,
        mode2: bool,
        inputs: PhaseInputs,
        bus: BusOutputs,
    ) -> (Option<u8>, Option<u8>) {
        let addr = inputs.addr;
        let mut shown = self.drivers(mode2, self.render.mode3, addr, bus);
        shown = first_of(shown, self.render.read_vram());
        shown = first_of(shown, self.render.oam.read(&self.render.oam_inputs()));
        // Nothing else drives the precharged bus: VRAM and OAM while the
        // PPU or OAM DMA holds them, the addresses nothing decodes.
        if (addr >= 0xFE00) | ((addr >= 0x8000) & (addr <= 0x9FFF)) {
            shown = first_of(shown, Some(0xFF));
        }
        // The bus is precharged high; what drives it pulls bits low.
        self.read_bus = if inputs.read {
            match phase {
                Phase::E => shown,
                Phase::F | Phase::G | Phase::H => match (self.read_bus, shown) {
                    (Some(held), Some(byte)) => Some(held & byte),
                    (held, None) => held,
                    (None, byte) => byte,
                },
                Phase::A | Phase::B | Phase::C | Phase::D => None,
            }
        } else {
            None
        };
        let driven = if inputs.read { shown } else { None };
        if inputs.read & matches!(phase, Phase::H) {
            (self.read_bus, driven)
        } else {
            (None, driven)
        }
    }
}
