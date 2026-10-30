//! The chip's blocks, wired together as `hdl/rtl/dmg_cpu_b.vhd` wires them,
//! edge by edge of the crystal.
//!
//! So far the clocks and resets, the divider and the timer, IF and OAM DMA:
//! the clocks run the phases the others step on, the divider's last bit
//! ends the clocks' power-on wait, the timer's interrupt rises into IF, and
//! the DMA runs on `dma_phi`, which is `data_phase` inverted. What the
//! blocks not transcribed yet give them (the CPU core's access and its
//! acknowledges, the other interrupt sources) comes in with each edge.
//!
//! A step is the edge that starts a phase, and what happens in that phase:
//! the flip-flops clocked by the edge take what was before it, the latches
//! follow, and the CPU's access in the phase reaches the registers.

use crate::{
    clocks::{ClockInputs, ClockOutputs, NetClocks, Phase},
    dma::NetDma,
    interrupts::{NetIf, TIMER},
    timer::{Access, NetTimer, Phase as TimerPhase, Reg},
};

/// What the chip's untranscribed blocks and pins give it at an edge.
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
    /// The CPU's access in the phase the edge starts: the address, whether
    /// it reads or writes, and the data bus.
    pub addr: u16,
    pub rd: bool,
    pub wr: bool,
    pub data: u8,
    /// The interrupt sources not transcribed yet that rise in the phase, as
    /// IF's bits (the timer's is the chip's own), and the CPU core's
    /// acknowledges in it.
    pub rises: u8,
    pub acks: u8,
}

/// The chip's transcribed blocks, between edges of the crystal.
#[derive(Clone, Copy)]
pub struct NetChip {
    pub clocks: NetClocks,
    pub timer: NetTimer,
    pub interrupts: NetIf,
    pub dma: NetDma,
    /// `dma_phi` as the last edge left it.
    pub dma_phi: bool,
}

impl NetChip {
    /// An edge of the crystal and the phase it starts.
    pub fn edge(&mut self, inputs: ChipInputs) {
        // The clocks, with the divider's last bit as it was at the edge.
        let clock_inputs = self.clock_inputs(inputs);
        self.clocks.step(clock_inputs);
        let phase = self.clocks.phase();
        let clocks = self.clocks.outputs(clock_inputs);

        // The divider and the timer: the A edge, then the access in E to G.
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
        let timer_rose = self.timer.int_timer && !int_timer;

        // IF: the sources' rises, the CPU's write and the acknowledges.
        let rises = (inputs.rises & !TIMER) | if timer_rose { TIMER } else { 0 };
        let write_if = (inputs.wr && inputs.addr == 0xFF0F).then_some(inputs.data);
        self.interrupts.phase(rises, write_if, inputs.acks);

        // OAM DMA, on `dma_phi`.
        let dma_phi = Self::dma_phi(clocks);
        let write_dma = (inputs.wr && inputs.addr == 0xFF46).then_some(inputs.data);
        if dma_phi && !self.dma_phi {
            self.dma.rise(write_dma.is_some());
        } else if !dma_phi && self.dma_phi {
            self.dma.fall();
        } else {
            // No edge of the DMA's clock.
        }
        self.dma.settle(write_dma);
        self.dma_phi = dma_phi;
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
