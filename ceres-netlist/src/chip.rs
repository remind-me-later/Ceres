//! The chip's blocks, wired together as `hdl/rtl/dmg_cpu_b.vhd` wires them,
//! edge by edge of the crystal.
//!
//! So far the clocks and resets, the divider and the timer, IF, OAM DMA,
//! LCDC, the line timing, the OAM scan, OAM's latches of the Y, render and
//! the sprite store: the clocks run the phases the others step on, the
//! divider's last bit ends the clocks' power-on wait, the timer's
//! interrupt, the STAT line and VBlank rise into IF, the DMA runs on
//! `dma_phi` (`data_phase` inverted), LCDC holds the PPU in reset and gives
//! the scan the sprites' height, the scan runs on the line timing's clocks
//! and compares what OAM's latches hold, the latches follow the scan, the
//! DMA and the sprite fetch, render's mode 0 is the STAT line's HBlank
//! source and its mode 3 STAT's, and render and the store run on what the
//! line timing, the scan, OAM's latches, the DMA and LCDC give them. What
//! the blocks not transcribed yet give them comes in with each edge
//! (`ChipInputs`): the CPU core's access and acknowledges, the other
//! interrupt sources, OAM's RAM and its odd byte, and VRAM's data.
//!
//! A step is the edge that starts a phase, and what happens in that phase:
//! the flip-flops clocked by the edge take what was before it, the latches
//! follow, and the CPU's access in the phase reaches the registers.

use crate::{
    Phase,
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
    /// The CPU's access in the phase: the address, whether it reads or
    /// writes, and the data bus.
    pub addr: u16,
    pub rd: bool,
    pub wr: bool,
    pub data: u8,
    /// The interrupt sources not transcribed yet (the serial port, the
    /// joypad) that rise in the phase, as IF's bits, and the CPU core's
    /// acknowledges in it.
    pub rises: u8,
    pub acks: u8,
    /// The data pins (the cartridge bus, which OAM DMA may read: the work
    /// RAM and the cartridge are not transcribed), and the CPU core's memory
    /// request outside VRAM.
    pub d: u8,
    pub non_vram_mreq: bool,
}

/// The chip's transcribed blocks, between edges of the crystal. VRAM, on
/// the board, is the edge's to borrow.
#[derive(Clone, Copy)]
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
}

impl NetChip {
    /// An edge of the crystal and the phase it starts.
    pub fn edge(&mut self, inputs: ChipInputs, vram: &mut [u8; 0x2000]) -> ChipEvents {
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

        let timer_rose = self.step_timer(phase, inputs);
        self.step_dma(clocks, inputs);
        let write_lcdc = (inputs.wr && inputs.addr == 0xFF40).then_some(inputs.data);
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
        self.step_render(clocks, lcdc, save, inputs, vram);
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
        let rises = (inputs.rises & !(TIMER | STAT | VBLANK))
            | if timer_rose { TIMER } else { 0 }
            | if stat_rose { STAT } else { 0 }
            | if vblank_rose { VBLANK } else { 0 };
        let write_if = (inputs.wr && inputs.addr == 0xFF0F).then_some(inputs.data);
        self.interrupts.phase(rises, write_if, inputs.acks);

        let read = self.step_read(phase, mode2, inputs);
        self.wr_before = inputs.wr;
        self.data_before = inputs.data;
        ChipEvents {
            timer_rose,
            stat_rose,
            vblank_rose,
            mode2,
            save,
            read,
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
            Some(reg) if inputs.wr => Access::Write(reg, inputs.data),
            _ => Access::None,
        };
        self.timer.phase(phase, access);
        self.timer.int_timer && !int_timer
    }

    /// OAM DMA, on `dma_phi`.
    const fn step_dma(&mut self, clocks: ClockOutputs, inputs: ChipInputs) {
        let dma_phi = Self::dma_phi(clocks);
        let write = if inputs.wr && inputs.addr == 0xFF46 {
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
        if inputs.wr && matches!(inputs.addr, 0xFF41 | 0xFF45) {
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
                wr: inputs.wr,
                data: inputs.data,
                rd: inputs.rd,
                non_vram_mreq: inputs.non_vram_mreq,
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
                d: inputs.d,
                dma_run: self.dma.run,
                dma_index: self.dma.count,
                scanning: self.scan.scanning,
                scan_index: self.scan.index,
            },
            vram,
        );
    }

    /// A read of the PPU's registers, OAM, VRAM or FF46: the AND of what
    /// drives the bus from E to H, which the core takes in H.
    fn step_read(&mut self, phase: Phase, mode2: bool, inputs: ChipInputs) -> Option<u8> {
        let shown = self
            .control
            .read(inputs.addr)
            .or_else(|| self.lcd.read(inputs.addr, mode2, self.render.mode3))
            .or_else(|| self.render.read(inputs.addr))
            .or_else(|| self.render.read_vram())
            .or_else(|| self.render.oam.read(&self.render.oam_inputs()))
            .or_else(|| (inputs.addr == 0xFF46).then_some(self.dma.source))
            // VRAM and OAM while the PPU or OAM DMA holds them: nothing
            // drives the precharged bus.
            .or_else(|| matches!(inputs.addr, 0x8000..=0x9FFF | 0xFE00..=0xFE9F).then_some(0xFF));
        // The bus is precharged high; what drives it pulls bits low.
        let driven = |bus: Option<u8>| match (bus, shown) {
            (Some(bus), Some(byte)) => Some(bus & byte),
            (bus, None) => bus,
            (None, byte) => byte,
        };
        self.read_bus = match (inputs.rd, phase) {
            (true, Phase::E) => shown,
            (true, Phase::F | Phase::G | Phase::H) => driven(self.read_bus),
            _ => None,
        };
        (inputs.rd && matches!(phase, Phase::H))
            .then_some(self.read_bus)
            .flatten()
    }
}
