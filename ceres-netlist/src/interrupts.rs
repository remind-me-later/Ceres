//! The netlist's IF, phase by phase.
//!
//! Transcribed from `hdl/rtl/interrupts.vhd`, and the latch of the requests
//! and the acknowledge of `hdl/rtl/sm83_interrupts.vhd`, for ceres-core's
//! proofs against ceres (its `interrupts/netlist_proofs.rs`) and for the
//! replay of the model's traces (`tests/replay.rs`), which checks the
//! transcription.
//!
//! Each flag is set by a rise of its source, forced to the data bus while
//! the CPU writes IF (E to G), and cleared by the acknowledge (G and H of
//! the interrupt entry's last M-cycle); clearing wins. A read of IF, and the
//! entry's choice of interrupt, take the flags as they were at the start of
//! E.

use crate::Phase;

/// IF's five flags (the other bits read as 1).
const INT_MASK: u8 = 0x1F;

/// The timer's bit of IF.
pub const TIMER: u8 = 0x04;

/// The phases of an M-cycle as ceres cuts it: from E to the next D.
pub const PHASES: [Phase; 8] = [
    Phase::E,
    Phase::F,
    Phase::G,
    Phase::H,
    Phase::A,
    Phase::B,
    Phase::C,
    Phase::D,
];

/// Where phase A is in `PHASES`, which lists the phases in their order: a
/// source that rises at the A edge rises there.
pub const A: usize = 4;
const _: () = assert!(matches!(PHASES[A], Phase::A));

/// The ceres T-cycle (0 to 3) that a netlist phase stands for.
#[must_use]
pub const fn t_cycle(phase: Phase) -> usize {
    match phase {
        Phase::E | Phase::F | Phase::G => 0,
        Phase::H => 1,
        Phase::A | Phase::B => 2,
        Phase::C | Phase::D => 3,
    }
}

#[derive(Clone, Copy)]
pub enum Access {
    None,
    ReadIf,
    WriteIf(u8),
    /// The interrupt entry's last M-cycle: its push goes to RAM.
    Entry,
}

/// The lowest interrupt that is requested and enabled, as a bit.
#[must_use]
pub const fn lowest(requested: u8) -> u8 {
    requested.isolate_lowest_one()
}

/// The netlist's IF, and IE.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct NetIf {
    pub flags: u8,
    pub ie: u8,
}

impl NetIf {
    /// The flags after one phase in which the sources `rise`, the CPU writes
    /// IF with `write`, and the core acknowledges `ack`.
    pub const fn phase(&mut self, rise: u8, write: Option<u8>, ack: u8) {
        let mut flags = self.flags;
        // A rise sets the flag, unless something forces it.
        flags |= rise & INT_MASK;
        if let Some(val) = write {
            flags = val & INT_MASK;
        }
        flags &= !ack;
        self.flags = flags;
    }

    /// An M-cycle from E to D, with the CPU's access and the sources that
    /// rise in each phase. Returns what a read reads and the interrupt the
    /// entry takes.
    pub fn m_cycle(&mut self, access: Access, rises: [u8; 8]) -> (Option<u8>, Option<u8>) {
        // The read's latch, and the requests' latch, close at the start of E.
        let read = matches!(access, Access::ReadIf).then_some(self.flags | !INT_MASK);
        let selected = lowest(self.ie & self.flags);
        let taken = matches!(access, Access::Entry).then_some(selected);
        for (phase, rise) in PHASES.iter().zip(rises) {
            let write = match access {
                Access::WriteIf(val) if matches!(phase, Phase::E | Phase::F | Phase::G) => {
                    Some(val)
                }
                _ => None,
            };
            let ack = match access {
                Access::Entry if matches!(phase, Phase::G | Phase::H) => selected,
                _ => 0,
            };
            self.phase(rise, write, ack);
        }
        (read, taken)
    }
}
