//! The chip's blocks, wired together as `hdl/rtl/dmg_cpu_b.vhd` wires them,
//! edge by edge of the crystal.
//!
//! The clocks and resets, the divider and the timer, IF, OAM DMA, LCDC,
//! the line timing, the OAM scan, OAM, render, the sprite store, VRAM's
//! bus, and the bus side (the external bus, HRAM, the boot ROM's mapping,
//! the joypad, the serial port): everything but the CPU core and the APU.
//! What comes in with each edge (`ChipInputs`) is the core's (its address,
//! read, write, request, the data bus, its acknowledges, and its request's
//! pulses, which the external bus's latches can catch), the APU's byte in
//! a read of its registers, the clock pins and the world outside (the
//! joypad's lines, SIN, SCK); the memories (VRAM, the board's work RAM and
//! cartridge, the boot ROM) are the edge's to borrow. Every read gets its
//! byte from the blocks that drive the data bus, IE's (the core's) among
//! them, or the precharged FF where nothing does.
//!
//! A step is the edge that starts a phase, and what happens in that phase:
//! the flip-flops clocked by the edge take what was before it, the latches
//! follow, and the CPU's access in the phase reaches the registers.

use crate::{
    Phase,
    bus::{BusInputs, BusOutputs, NetBoard, NetBus},
    clocks::{ClockInputs, ClockOutputs, NetClocks},
    dma::NetDma,
    interrupts::{NetIf, TIMER},
    lcd_control::NetLcdControl,
    lcd_timing::NetLcd,
    oam::OamInputs,
    oam_scan::{Clocks as ScanClocks, NetScan},
    render::{NetRender, RenderInputs},
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

/// What the chip's untranscribed blocks and pins give it at an edge, and in
/// the phase the edge starts.
#[derive(Clone, Copy)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "the pins' and strobes' levels"
)]
pub struct ChipInputs {
    /// The pins and the CPU core's enables as they are after the edge (the
    /// divider's bit, which the clocks also take, comes from the chip).
    pub ck1: bool,
    pub reset_pin_n: bool,
    pub t1: bool,
    pub t2: bool,
    pub osc_ena: bool,
    pub clk_ena: bool,
    pub mreq: bool,
    pub write: bool,
    pub test_reset: bool,
    /// The CPU core's access in the phase: its address, its read, and the
    /// chip's data bus (the core's write strobe and request are `write` and
    /// `mreq`, above); its acknowledges.
    pub addr: u16,
    pub read: bool,
    pub data: u8,
    pub acks: u8,
    /// The core's request and write strobe as they are in the phase (the
    /// clocks take them as they are at the edge, `mreq` and `write`).
    pub mreq_in_phase: bool,
    pub write_in_phase: bool,
    /// The address the external bus's latches took as the core's request
    /// pulsed inside the phase (the core's timing), if they did.
    pub latch_glitch: Option<u16>,
    /// The world outside: the joypad's lines, SIN, and SCK as the other end
    /// of the link drives it.
    pub p10_13: u8,
    pub sin: bool,
    pub sck: bool,
    /// What the APU, which is not transcribed, drives on the data bus in a
    /// read of its registers or wave RAM (FF10 to FF3F).
    pub apu: Option<u8>,
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
    /// The bus side: the external bus, HRAM, the boot ROM's mapping, the
    /// joypad, the serial port.
    pub bus: NetBus,
    /// The chip's write strobe in the phase (`cpu_wr_sync`).
    pub cpu_wr: bool,
    /// The serial port's and the joypad's interrupt lines in the last phase.
    pub int_serial: bool,
    pub int_jp: bool,
    /// The CPU's write strobe and the data bus in the last phase: the
    /// latches a write opens take the bus as it was first.
    pub wr_before: bool,
    pub data_before: u8,
    /// What a read of LCDC, STAT, LY or LYC has taken so far: the core's data bus
    /// is precharged high and only pulled low while the read drives it,
    /// from E until it takes it in H.
    pub read_bus: Option<u8>,
}

/// What a step does that the CPU and the other blocks see.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
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
    /// The byte a read of LCDC, STAT, LY or LYC takes, in H.
    pub read: Option<u8>,
    /// What the bus side gave in the phase: the pins, its interrupts.
    pub bus: BusOutputs,
}

impl NetChip {
    /// An edge of the crystal and the phase it starts.
    pub fn edge(&mut self, inputs: ChipInputs, memories: &mut ChipMemories) -> ChipEvents {
        // LCDC as the edge finds it: its latches open after the edge.
        // @fact lcdc-write-latch-delay
        let lcdc_at_edge = Lcdc {
            ppu_reset: self.control.ppu_reset(self.clocks.reset2()),
            tall: self.control.tall(),
        };
        // The clocks, with the divider's last bit as it was at the edge.
        let clock_inputs = self.clock_inputs(inputs);
        self.clocks.step(clock_inputs);
        let phase = self.clocks.phase();
        let clocks = self.clocks.outputs(clock_inputs);

        self.cpu_wr = clocks.cpu_wr_sync;
        let timer_rose = self.step_timer(phase, inputs);
        self.step_dma(clocks, inputs);
        let bus = self.step_bus(phase, clocks, inputs, memories);
        let write_lcdc = (self.cpu_wr && inputs.addr == 0xFF40).then_some(inputs.data);
        self.control.settle(clocks.reset2, write_lcdc);
        let lcdc = Lcdc {
            ppu_reset: self.control.ppu_reset(clocks.reset2),
            tall: self.control.tall(),
        };
        // Mode 0 as the edge finds it (render's, which changes after the
        // line timing's flip-flops).
        let mode0_at_edge = self.render.mode0();
        let lcd_before = self.lcd;
        let (mut stat_rose, level, vblank_rose) =
            self.step_lcd(lcdc_at_edge, lcdc, mode0_at_edge, inputs);
        let (mode2, save) = self.step_scan(lcd_before, lcdc_at_edge, lcdc);
        self.step_render(clocks, lcdc, save, inputs, bus, &mut memories.vram);
        // Then mode 0, which rises after the edge's flip-flops.
        let mut level = level;
        for now in [
            self.lcd.int_stat(mode0_at_edge),
            self.lcd.int_stat(self.render.mode0()),
        ] {
            stat_rose |= now && !level;
            level = now;
        }

        // IF: the sources' rises, the CPU's write and the acknowledges.
        let rises = if bus.int_serial && !self.int_serial {
            SERIAL
        } else {
            0
        } | if bus.int_jp && !self.int_jp {
            JOYPAD
        } else {
            0
        } | if timer_rose { TIMER } else { 0 }
            | if stat_rose { STAT } else { 0 }
            | if vblank_rose { VBLANK } else { 0 };
        let write_if = (self.cpu_wr && inputs.addr == 0xFF0F).then_some(inputs.data);
        // A read of IF takes the flags as they were when it started.
        let flags_before = self.interrupts.flags;
        self.interrupts.phase(rises, write_if, inputs.acks);
        self.int_serial = bus.int_serial;
        self.int_jp = bus.int_jp;

        let read = self.step_read(phase, mode2, flags_before, inputs, bus);
        self.wr_before = self.cpu_wr;
        self.data_before = inputs.data;
        ChipEvents {
            timer_rose,
            stat_rose,
            vblank_rose,
            mode2,
            save,
            read,
            bus,
        }
    }

    /// The clocks' inputs at an edge: the pins and the CPU core's, and the
    /// divider's last bit.
    #[must_use]
    pub const fn clock_inputs(&self, inputs: ChipInputs) -> ClockInputs {
        ClockInputs {
            ck1: inputs.ck1,
            reset_pin_n: inputs.reset_pin_n,
            t1: inputs.t1,
            t2: inputs.t2,
            osc_ena: inputs.osc_ena,
            clk_ena: inputs.clk_ena,
            mreq: inputs.mreq,
            write: inputs.write,
            test_reset: inputs.test_reset,
            div_16hz: self.timer.count & 0x8000 != 0,
        }
    }

    /// The DMA's clock: high in A to D, and while the CPU is halted.
    // @hw die: the chip's dma_phi <= not data_phase
    #[must_use]
    pub const fn dma_phi(clocks: ClockOutputs) -> bool {
        !clocks.data_phase
    }

    /// The divider and the timer: the A edge, then the access in E to G.
    /// Whether the interrupt rose.
    fn step_timer(&mut self, phase: Phase, inputs: ChipInputs) -> bool {
        let int_timer = self.timer.int_timer;
        let access = match Reg::at(inputs.addr) {
            Some(reg) if self.cpu_wr => Access::Write(reg, inputs.data),
            _ => Access::None,
        };
        self.timer.phase(phase, access);
        self.timer.int_timer && !int_timer
    }

    /// OAM DMA, on `dma_phi`.
    const fn step_dma(&mut self, clocks: ClockOutputs, inputs: ChipInputs) {
        let dma_phi = Self::dma_phi(clocks);
        let write = if self.cpu_wr && inputs.addr == 0xFF46 {
            Some(inputs.data)
        } else {
            None
        };
        if dma_phi && !self.dma_phi {
            self.dma.rise(write.is_some());
        } else if !dma_phi && self.dma_phi {
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
        inputs: ChipInputs,
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
        if self.cpu_wr && matches!(inputs.addr, 0xFF41 | 0xFF45) {
            let opens = !self.wr_before;
            for data in [opens.then_some(self.data_before), Some(inputs.data)]
                .into_iter()
                .flatten()
            {
                self.lcd.write(inputs.addr, data);
                let now = self.lcd.int_stat(mode0_at_edge);
                rose |= now && !level;
                level = now;
            }
        }
        if lcdc.ppu_reset {
            self.lcd.reset();
        }
        // @hw die: the chip's int_vbl <= mode1
        (rose, level, self.lcd.mode1 && !mode1)
    }

    /// The OAM scan, on the line timing's clocks (`lcd` as the edge found
    /// them), with what OAM's latches held; then the latches. Mode 2 for
    /// OAM and STAT, and the save strobes.
    fn step_scan(&mut self, lcd: NetLcd, at_edge: Lcdc, lcdc: Lcdc) -> (bool, u16) {
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
        let save = self
            .scan
            .save(
                self.lcd.wosu,
                self.render.oam.second_even,
                self.lcd.ly,
                lcdc.tall,
            )
            .map_or(0, |slot| 1 << slot);
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
    /// mapping, the joypad and the serial port, with the board on the pins.
    fn step_bus(
        &mut self,
        phase: Phase,
        clocks: ClockOutputs,
        inputs: ChipInputs,
        memories: &mut ChipMemories,
    ) -> BusOutputs {
        let bus_inputs = BusInputs {
            addr: inputs.addr,
            read: inputs.read,
            write: inputs.write_in_phase,
            mreq: inputs.mreq_in_phase,
            data: inputs.data,
            clk_t4: clocks.clk_t4,
            exec_phase: clocks.exec_phase,
            cpu_wr_sync: clocks.cpu_wr_sync,
            cpu_ext_cs_en: clocks.cpu_ext_cs_en,
            clk_1mhz: clocks.clk_1mhz,
            starts_a: matches!(phase, Phase::A),
            reset2: clocks.reset2,
            reset: clocks.reset,
            dma_addr_ext: self.dma.run && !self.dma_from_vram(),
            dma_addr: u16::from_be_bytes([self.dma.source, self.dma.count]),
            div: self.timer.count,
            p10_13: inputs.p10_13,
            sin: inputs.sin,
            sck: inputs.sck,
            latch_glitch: inputs.latch_glitch,
        };
        self.bus
            .phase(&bus_inputs, &mut memories.board, &memories.boot_rom)
    }

    /// What a read of the divider, the timer, IF or IE drives on the bus
    /// (their bits that no flip-flop drives stay precharged, 1). IF's read
    /// latch holds the flags as the read found them in E; they only rise
    /// during a read, so later phases' flags AND to the same byte. IE is the
    /// core's, which pulls its own data bus down where IE is 0.
    // @hw die: TAWU TAKU TEMU TUSE UPUG SEPU SAWA TATU (FF04); TEDA TUBY
    // SORA, SOKU..PUSO SETE..SAPU RYLA ROTE SUPE (FF05 to FF07); MATY MOPO
    // PAVY NEJY NUTY, POLA, NELA..PEGY (FF0F); IRQ_NAND2_NAND3 (FFFF)
    const fn read_timer_if(&self, phase: Phase, flags_before: u8, addr: u16) -> Option<u8> {
        match addr {
            0xFF04 => Some(self.timer.read(Reg::Div)),
            0xFF05 => Some(self.timer.read(Reg::Tima)),
            0xFF06 => Some(self.timer.read(Reg::Tma)),
            0xFF07 => Some(self.timer.read(Reg::Tac) | 0xF8),
            0xFF0F => Some(
                if matches!(phase, Phase::E) {
                    flags_before
                } else {
                    self.interrupts.flags
                } | 0xE0,
            ),
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
        clocks: ClockOutputs,
        lcdc: Lcdc,
        save: u16,
        inputs: ChipInputs,
        bus: BusOutputs,
        vram: &mut [u8; 0x2000],
    ) {
        let line_start = self.scan.line_start() || lcdc.ppu_reset;
        self.render.edge(
            RenderInputs {
                clk: self.clocks.clk,
                ppu_reset: lcdc.ppu_reset,
                hard_reset: clocks.reset2,
                hclk: self.lcd.vena,
                mode1: self.lcd.mode1,
                ly: self.lcd.ly,
                lcdc: self.control.lcdc,
                addr: inputs.addr,
                wr: self.cpu_wr,
                data: inputs.data,
                rd: inputs.read,
                non_vram_mreq: bus.non_vram_mreq,
                write: inputs.write,
                cpu_wr_sync: clocks.cpu_wr_sync,
                cpu_ext_cs_en: clocks.cpu_ext_cs_en,
                clk_t4: clocks.clk_t4,
                vram_to_oam: self.dma.run && self.dma_from_vram(),
                dma_addr: u16::from_be_bytes([self.dma.source, self.dma.count]),
                line_start,
                stop_scan: self.scan.stop_scan() && !line_start,
                ceno: self.scan.ceno,
                save,
                // The Y comparator's difference, Y - LY - 1: its low four bits.
                scan_line: self.render.oam.second_even.wrapping_add(!self.lcd.ly) & 0x0F,
                wuvu: self.lcd.wuvu,
                wosu: self.lcd.wosu,
                dma_phi: self.dma_phi,
                dma_addr_ext: self.dma.run && !self.dma_from_vram(),
                d: bus.pins.d,
                dma_run: self.dma.run,
                dma_index: self.dma.count,
                scanning: self.scan.scanning,
                scan_index: self.scan.index,
            },
            vram,
        );
    }

    /// A read: the AND of what drives the bus from E to H, which the core
    /// takes in H. `flags_before` is IF as the phase found it.
    fn step_read(
        &mut self,
        phase: Phase,
        mode2: bool,
        flags_before: u8,
        inputs: ChipInputs,
        bus: BusOutputs,
    ) -> Option<u8> {
        let shown = self
            .control
            .read(inputs.addr)
            .or_else(|| self.lcd.read(inputs.addr, mode2, self.render.mode3))
            .or_else(|| self.render.read(inputs.addr))
            .or_else(|| self.render.read_vram())
            .or_else(|| self.render.oam.read(&self.render.oam_inputs()))
            .or_else(|| (inputs.addr == 0xFF46).then_some(self.dma.source))
            .or(bus.data_out)
            .or_else(|| self.read_timer_if(phase, flags_before, inputs.addr))
            .or(inputs.apu)
            // Nothing else drives the precharged bus: VRAM and OAM while the
            // PPU or OAM DMA holds them, the addresses nothing decodes.
            .or_else(|| {
                (inputs.addr >= 0xFE00 || (0x8000..=0x9FFF).contains(&inputs.addr)).then_some(0xFF)
            });
        // The bus is precharged high; what drives it pulls bits low.
        let driven = |held: Option<u8>| match (held, shown) {
            (Some(held), Some(byte)) => Some(held & byte),
            (held, None) => held,
            (None, byte) => byte,
        };
        self.read_bus = match (inputs.read, phase) {
            (true, Phase::E) => shown,
            (true, Phase::F | Phase::G | Phase::H) => driven(self.read_bus),
            _ => None,
        };
        (inputs.read && matches!(phase, Phase::H))
            .then_some(self.read_bus)
            .flatten()
    }
}
