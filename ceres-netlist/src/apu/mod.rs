//! The netlist's APU, delta cycle by delta cycle.
//!
//! Its control (`hdl/rtl/apu_control.vhd`) and its channels (`apu_ch1.vhd`
//! to `apu_ch4.vhd`), for the replay of the model's traces (`tests/replay.rs`),
//! which checks the transcription, and for `NetChip`.
//!
//! The APU runs on clocks it makes: dividers of the chip's clock, each
//! clocking the next, the frame sequencer from the divider's bit 10, and
//! the channels' counters, whose bits clock their next stage. Which of them
//! ticks at an edge, and with what, depends on the order in which the
//! simulator settles them, so the transcription settles the APU as the
//! simulator does, as `crate::render` does render: each concurrent
//! statement of the VHDL is a wire that follows its inputs a delta cycle
//! later, each process reads the signals as they are in the current delta
//! cycle, and the flip-flops' outputs change `tco` (1 ns) later. An edge of
//! the chip's clock runs deltas until nothing changes, then the next
//! nanosecond's changes, until none are left. What the APU takes from the
//! other blocks reaches it at its time: the clocks decoded from the phase
//! with the edge (the write strobe among them, so that a write's latches
//! first take the bus as the phase before left it), the resets and the
//! divider 1 ns after it, the CPU's read strobe at 3 ns and its address and
//! data bus at 4 ns (the core puts its strobes out before its address, so
//! that a read ends before the address moves).
//!
//! The mixer, which the model computes in volts, is left out: the
//! transcription ends at the four channels' outputs.

mod control;
mod noise;
mod square;
mod sweep;
mod wave;

pub use {
    control::{ControlWires, NetApuControl},
    noise::{NetNoise, NoiseWires},
    square::{NetSquare, SquareWires},
    sweep::{NetSweep, SweepWires},
    wave::{NetWave, WaveWires},
};

/// A flip-flop: its output, and the change scheduled `tco` (1 ns) after
/// its clock's edge.
///
/// As VHDL's inertial delay does, a later change replaces
/// a pending one, and a zero-delay assignment (a reset, a load) cancels it.
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub struct Ff<T> {
    pub q: T,
    pub next: T,
    pub due: bool,
}

impl<T: Copy> Ff<T> {
    /// A flip-flop that shows `q`, with nothing pending.
    #[must_use]
    pub const fn new(q: T) -> Self {
        Self {
            q,
            next: q,
            due: false,
        }
    }

    /// The change at its clock's edge, `tco` from now.
    pub const fn after_tco(&mut self, value: T) {
        self.next = value;
        self.due = true;
    }

    /// A zero-delay assignment: a reset or a load.
    pub const fn set(&mut self, value: T) {
        self.q = value;
        self.due = false;
    }

    /// The next nanosecond: the change pending takes effect.
    pub const fn advance(&mut self) {
        if self.due {
            self.q = self.next;
            self.due = false;
        }
    }
}

/// What the APU takes in after an edge of the chip's clock.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "the clocks' and strobes' levels"
)]
pub struct ApuInputs {
    /// The chip's clock and the clocks decoded from the phase, the write
    /// strobe among them, at the edge.
    pub clk: bool,
    pub data_phase: bool,
    pub clk_t4: bool,
    pub pch_phase: bool,
    /// The resets, the divider's bit 10 and the test mode T1 alone, 1 ns
    /// after the edge.
    pub reset2: bool,
    pub div10: bool,
    pub t1_only: bool,
    /// The CPU's read strobe, 3 ns after the edge, its address and data
    /// bus, 4 ns after it.
    pub addr: u16,
    pub data: u8,
    pub cpu_rd: bool,
    pub cpu_wr: bool,
}

/// When the resets, the divider and the test pins reach the APU.
const INPUTS_AT: u32 = 1;
/// When the CPU's read strobe does: before its address, which the core's
/// gates put out later (`hdl/rtl/sm83.vhd`), so that a read ends before
/// the address moves.
const READ_AT: u32 = 3;
/// When the CPU's address and data bus do.
const DATA_AT: u32 = 4;

/// The deltas an edge may take before it is taken to oscillate.
const MAX_DELTAS: usize = 1000;

const fn rose(before: bool, now: bool) -> bool {
    !before & now
}

const fn fell(before: bool, now: bool) -> bool {
    before & !now
}

/// The netlist's APU between edges of the chip's clock.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct NetApu {
    pub inputs: ApuInputs,
    pub control: NetApuControl,
    pub ch1: NetSquare,
    pub sweep: NetSweep,
    pub ch2: NetSquare,
    pub ch3: NetWave,
    pub ch4: NetNoise,
}

impl NetApu {
    /// An edge of the chip's clock: `inputs` are what the APU takes in
    /// after it, which reach it at their times.
    ///
    /// # Panics
    ///
    /// If the APU does not settle (a loop of its wires oscillates).
    pub fn edge(&mut self, inputs: ApuInputs) {
        let mut prev = *self;
        // The clocks decoded from the phase change with the clock.
        self.inputs.clk = inputs.clk;
        self.inputs.data_phase = inputs.data_phase;
        self.inputs.clk_t4 = inputs.clk_t4;
        self.inputs.pch_phase = inputs.pch_phase;
        // The write strobe too (`cpu_wr_sync`): a write's latches open on the
        // bus as the phase before left it, before the CPU drives it.
        self.inputs.cpu_wr = inputs.cpu_wr;
        let mut now = 0;
        let mut deltas = 0;
        loop {
            // The deltas of this nanosecond, until nothing changes.
            loop {
                let next = self.delta(&prev);
                prev = *self;
                if next == *self {
                    break;
                }
                *self = next;
                deltas += 1;
                assert!(deltas < MAX_DELTAS, "the APU oscillates");
            }
            // The next nanosecond: the inputs' next arrival, or the
            // flip-flops' changes, due a nanosecond after they were made.
            let due = self.control.any_due()
                | self.ch1.any_due()
                | self.sweep.any_due()
                | self.ch2.any_due()
                | self.ch3.any_due()
                | self.ch4.any_due();
            let arrives = if now < INPUTS_AT {
                INPUTS_AT
            } else if now < READ_AT {
                READ_AT
            } else if now < DATA_AT {
                DATA_AT
            } else {
                u32::MAX
            };
            if !due & (arrives == u32::MAX) {
                break;
            }
            now = if due { now + 1 } else { arrives };
            prev = *self;
            if now == INPUTS_AT {
                self.inputs.reset2 = inputs.reset2;
                self.inputs.div10 = inputs.div10;
                self.inputs.t1_only = inputs.t1_only;
            }
            if now == READ_AT {
                self.inputs.cpu_rd = inputs.cpu_rd;
            }
            if now == DATA_AT {
                self.inputs.addr = inputs.addr;
                self.inputs.data = inputs.data;
            }
            if due {
                self.control.advance();
                self.ch1.advance();
                self.sweep.advance();
                self.ch2.advance();
                self.ch3.advance();
                self.ch4.advance();
            }
        }
    }

    /// One delta cycle: the wires a delta behind what they follow, the
    /// processes on the signals as they are.
    fn delta(&self, prev: &Self) -> Self {
        let mut next = *self;
        let (cw, pcw) = (&self.control.wires, &prev.control.wires);
        self.control
            .delta(&prev.control, &self.inputs, &prev.inputs, &mut next.control);
        self.ch1.delta(
            &prev.ch1,
            &self.inputs,
            cw,
            pcw,
            self.ch1_clocks(cw.ch1_1mhz, pcw.ch1_1mhz),
            &mut next.ch1,
        );
        self.sweep.delta(
            &prev.sweep,
            &self.inputs,
            cw,
            pcw,
            &self.ch1,
            &self.ch1.wires,
            &mut next.sweep,
        );
        let clocks = Self::ch2_clocks(cw.ch2_1mhz, pcw.ch2_1mhz);
        self.ch2.delta(
            &prev.ch2,
            &self.inputs,
            &self.control.wires,
            &prev.control.wires,
            clocks,
            &mut next.ch2,
        );
        self.ch3.delta(
            &prev.ch3,
            &self.inputs,
            &prev.inputs,
            cw,
            pcw,
            &mut next.ch3,
        );
        self.ch4
            .delta(&prev.ch4, &self.inputs, cw, pcw, &mut next.ch4);
        next
    }

    /// Channel 1's clock, registers, swept frequency and overflow.
    const fn ch1_clocks(&self, channel: bool, channel_before: bool) -> square::Clocks {
        square::Clocks {
            channel,
            channel_before,
            nrx1: 0xFF11,
            swept: true,
            acc: self.sweep.acc(),
            overflow: self.sweep.wires.overflow,
        }
    }

    /// Channel 2's clock and registers.
    const fn ch2_clocks(channel: bool, channel_before: bool) -> square::Clocks {
        square::Clocks {
            channel,
            channel_before,
            nrx1: 0xFF16,
            swept: false,
            acc: 0,
            overflow: false,
        }
    }

    /// The wires as the state gives them, until they settle (no process
    /// runs): for a state taken from a trace.
    ///
    /// # Panics
    ///
    /// If the wires do not settle.
    pub fn settle_wires(&mut self) {
        let mut deltas = 0;
        loop {
            let mut next = *self;
            let cw = &self.control.wires;
            next.control.wires = self.control.next_wires(&self.inputs);
            next.ch1.wires =
                self.ch1
                    .next_wires(&self.inputs, cw, self.ch1_clocks(cw.ch1_1mhz, cw.ch1_1mhz));
            next.sweep.wires = self.sweep.next_wires(&self.inputs, cw, &self.ch1);
            next.ch2.wires =
                self.ch2
                    .next_wires(&self.inputs, cw, Self::ch2_clocks(cw.ch2_1mhz, cw.ch2_1mhz));
            next.ch3.wires = self.ch3.next_wires(&self.inputs, cw);
            next.ch4.wires = self.ch4.next_wires(&self.inputs, cw);
            if next == *self {
                break;
            }
            *self = next;
            deltas += 1;
            assert!(deltas < MAX_DELTAS, "the APU's wires oscillate");
        }
    }
}
