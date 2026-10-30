//! The chip's blocks, wired together as `hdl/rtl/dmg_cpu_b.vhd` wires them,
//! edge by edge of the crystal.
//!
//! So far the clocks and resets, the divider and the timer, IF, OAM DMA,
//! the line timing, the OAM scan and OAM's latches of the Y: the clocks run
//! the phases the others step on, the divider's last bit ends the clocks'
//! power-on wait, the timer's interrupt, the STAT line and VBlank rise into
//! IF, the DMA runs on `dma_phi` (`data_phase` inverted), the scan runs on
//! the line timing's clocks and compares what OAM's latches hold, and the
//! latches follow the scan and the DMA. What the blocks not transcribed yet
//! give them comes in with each edge (`ChipInputs`): the CPU core's access
//! and acknowledges, the other interrupt sources, render's modes, LCDC's
//! reset and bit 2, the sprite fetch's strobes and OAM's RAM.
//!
//! A step is the edge that starts a phase, and what happens in that phase:
//! the flip-flops clocked by the edge take what was before it, the latches
//! follow, and the CPU's access in the phase reaches the registers.

use crate::{
    clocks::{ClockInputs, ClockOutputs, NetClocks, Phase},
    dma::NetDma,
    interrupts::{NetIf, TIMER},
    lcd_timing::NetLcd,
    oam::{NetOamLatches, OamInputs},
    oam_scan::{Clocks as ScanClocks, NetScan},
    timer::{Access, NetTimer, Phase as TimerPhase, Reg},
};

/// IF's VBlank and STAT bits.
const VBLANK: u8 = 0x01;
const STAT: u8 = 0x02;

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
    /// LCDC's reset of the LCD (bit 7 clear) and render's mode 0, at the
    /// edge and in the phase, and render's mode 3 in the phase.
    pub ppu_reset_at_edge: bool,
    pub ppu_reset: bool,
    pub mode0_at_edge: bool,
    pub mode0: bool,
    pub mode3: bool,
    /// LCDC bit 2 (sprites of 16 lines), at the edge and in the phase.
    pub tall_at_edge: bool,
    pub tall: bool,
    /// The sprite fetch reads OAM, and holds the second latches.
    pub fetch_read: bool,
    pub fetch_hold: bool,
    /// The byte OAM's RAM gives while its first latches are open (the RAM is
    /// not transcribed).
    pub oam_data: u8,
}

/// The chip's transcribed blocks, between edges of the crystal.
#[derive(Clone, Copy)]
pub struct NetChip {
    pub clocks: NetClocks,
    pub timer: NetTimer,
    pub interrupts: NetIf,
    pub dma: NetDma,
    /// `dma_phi` in the last phase.
    pub dma_phi: bool,
    pub lcd: NetLcd,
    pub scan: NetScan,
    pub oam: NetOamLatches,
    /// The CPU's write strobe and the data bus in the last phase: the
    /// latches a write opens take the bus as it was first.
    pub wr_before: bool,
    pub data_before: u8,
    /// What a read of STAT, LY or LYC has taken so far: the core's data bus
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
    /// The byte a read of STAT, LY or LYC takes, in H.
    pub read: Option<u8>,
}

impl NetChip {
    /// An edge of the crystal and the phase it starts.
    pub fn edge(&mut self, inputs: ChipInputs) -> ChipEvents {
        // The clocks, with the divider's last bit as it was at the edge.
        let clock_inputs = self.clock_inputs(inputs);
        self.clocks.step(clock_inputs);
        let phase = self.clocks.phase();
        let clocks = self.clocks.outputs(clock_inputs);

        let timer_rose = self.step_timer(phase, inputs);
        self.step_dma(clocks, inputs);
        let lcd_before = self.lcd;
        let (stat_rose, vblank_rose) = self.step_lcd(inputs);

        // IF: the sources' rises, the CPU's write and the acknowledges.
        let rises = (inputs.rises & !(TIMER | STAT | VBLANK))
            | if timer_rose { TIMER } else { 0 }
            | if stat_rose { STAT } else { 0 }
            | if vblank_rose { VBLANK } else { 0 };
        let write_if = (inputs.wr && inputs.addr == 0xFF0F).then_some(inputs.data);
        self.interrupts.phase(rises, write_if, inputs.acks);

        let (mode2, save) = self.step_scan(lcd_before, inputs);
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
        match timer_phase(phase) {
            None => self.timer.edge_a(),
            Some(timer_phase) => {
                let access = match Reg::at(inputs.addr) {
                    Some(reg) if inputs.wr => Access::Write(reg, inputs.data),
                    _ => Access::None,
                };
                self.timer.phase(timer_phase, access);
            }
        }
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
    /// LCD), the CPU's write of LYC or STAT, the LCD's reset. Whether the
    /// STAT line and VBlank rose.
    fn step_lcd(&mut self, inputs: ChipInputs) -> (bool, bool) {
        let mode1 = self.lcd.mode1;
        let mut rose = false;
        if self.lcd.clk != self.clocks.clk {
            if inputs.ppu_reset_at_edge {
                self.lcd.clk = self.clocks.clk;
            } else {
                rose = self.lcd.edge(inputs.mode0_at_edge);
            }
        }
        let mut level = self.lcd.int_stat(inputs.mode0_at_edge);
        if inputs.wr && matches!(inputs.addr, 0xFF41 | 0xFF45) {
            let opens = !self.wr_before;
            for data in [opens.then_some(self.data_before), Some(inputs.data)]
                .into_iter()
                .flatten()
            {
                self.lcd.write(inputs.addr, data);
                let now = self.lcd.int_stat(inputs.mode0_at_edge);
                rose |= now && !level;
                level = now;
            }
        }
        if inputs.ppu_reset {
            self.lcd.reset();
        }
        // Then mode 0, which rises after the edge's flip-flops.
        for now in [
            self.lcd.int_stat(inputs.mode0_at_edge),
            self.lcd.int_stat(inputs.mode0),
        ] {
            rose |= now && !level;
            level = now;
        }
        // @hw die: the chip's int_vbl <= mode1
        (rose, self.lcd.mode1 && !mode1)
    }

    /// The OAM scan, on the line timing's clocks (`lcd` as the edge found
    /// them), with what OAM's latches held; then the latches. Mode 2 for
    /// OAM and STAT, and the save strobes.
    fn step_scan(&mut self, lcd: NetLcd, inputs: ChipInputs) -> (bool, u16) {
        if inputs.ppu_reset_at_edge {
            self.scan.reset();
        } else {
            let clocks = ScanClocks {
                rising: self.clocks.clk,
                wuvu_before: lcd.wuvu,
                wuvu_after: self.lcd.wuvu,
                wosu_before: lcd.wosu,
                vclk: lcd.vclk,
                ly: lcd.ly,
            };
            self.scan.edge(clocks, self.oam.second, inputs.tall_at_edge);
        }
        if inputs.ppu_reset {
            self.scan.reset();
        }
        let oam_inputs = OamInputs {
            scanning: self.scan.scanning,
            run: self.dma.run,
            dma_phi: self.dma_phi,
            wuvu: self.lcd.wuvu,
            wosu: self.lcd.wosu,
            fetch_read: inputs.fetch_read,
            fetch_hold: inputs.fetch_hold,
        };
        self.oam.settle(oam_inputs, inputs.oam_data);
        let save = self
            .scan
            .save(self.lcd.wosu, self.oam.second, self.lcd.ly, inputs.tall)
            .map_or(0, |slot| 1 << slot);
        (oam_inputs.mode2(), save)
    }

    /// A read of STAT, LY or LYC: the AND of what the register shows from E
    /// to H, which the core takes in H.
    fn step_read(&mut self, phase: Phase, mode2: bool, inputs: ChipInputs) -> Option<u8> {
        let shown = self.lcd.read(inputs.addr, mode2, inputs.mode3);
        self.read_bus = match (inputs.rd, phase) {
            (true, Phase::E) => shown,
            (true, Phase::F | Phase::G | Phase::H) => self.read_bus.zip(shown).map(|(a, b)| a & b),
            _ => None,
        };
        (inputs.rd && matches!(phase, Phase::H))
            .then_some(self.read_bus)
            .flatten()
    }
}

/// The timer's phase for a phase of the clocks: `None` for A, whose edge
/// the timer takes as such.
const fn timer_phase(phase: Phase) -> Option<TimerPhase> {
    match phase {
        Phase::A => None,
        Phase::B => Some(TimerPhase::B),
        Phase::C => Some(TimerPhase::C),
        Phase::D => Some(TimerPhase::D),
        Phase::E => Some(TimerPhase::E),
        Phase::F => Some(TimerPhase::F),
        Phase::G => Some(TimerPhase::G),
        Phase::H => Some(TimerPhase::H),
    }
}
