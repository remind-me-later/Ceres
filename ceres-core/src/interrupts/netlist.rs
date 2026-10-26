//! The netlist's IF, transcribed from `hdl/rtl/interrupts.vhd`, and the
//! latch of the requests and the acknowledge of
//! `hdl/rtl/sm83_interrupts.vhd`, phase by phase, for the proofs against
//! ceres (`netlist_proofs.rs`) and for the replay of the model's traces
//! (`crate::netlist_replay`), which checks the transcription.
//!
//! Each flag is set by a rise of its source, forced to the data bus while
//! the CPU writes IF (E to G), and cleared by the acknowledge (G and H of
//! the interrupt entry's last M-cycle); clearing wins. A read of IF, and the
//! entry's choice of interrupt, take the flags as they were at the start of
//! E.

// Some of it only the proofs use, which other builds leave out.
#![cfg_attr(not(kani), allow(dead_code))]
#![expect(
    clippy::field_scoped_visibility_modifiers,
    reason = "the proofs and the replay set and compare the flags one by one"
)]

use super::{INT_MASK, INT_TIMER_B};

/// The timer's bit of IF.
pub(crate) const TIMER: u8 = INT_TIMER_B;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Phase {
    E,
    F,
    G,
    H,
    A,
    B,
    C,
    D,
}

pub(crate) const PHASES: [Phase; 8] = [
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
pub(crate) const A: usize = Phase::A as usize;

/// The ceres T-cycle (0 to 3) that a netlist phase stands for.
pub(crate) const fn t_cycle(phase: Phase) -> usize {
    match phase {
        Phase::E | Phase::F | Phase::G => 0,
        Phase::H => 1,
        Phase::A | Phase::B => 2,
        Phase::C | Phase::D => 3,
    }
}

#[derive(Clone, Copy)]
pub(crate) enum Access {
    None,
    ReadIf,
    WriteIf(u8),
    /// The interrupt entry's last M-cycle: its push goes to RAM.
    Entry,
}

/// The lowest interrupt that is requested and enabled, as a bit.
pub(crate) const fn lowest(requested: u8) -> u8 {
    requested.isolate_lowest_one()
}

/// The netlist's IF, and IE.
pub(crate) struct NetIf {
    pub(crate) flags: u8,
    pub(crate) ie: u8,
}

impl NetIf {
    /// The flags after one phase in which the sources `rise`, the CPU writes
    /// IF with `write`, and the core acknowledges `ack`.
    pub(crate) const fn phase(&mut self, rise: u8, write: Option<u8>, ack: u8) {
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
    pub(crate) fn m_cycle(&mut self, access: Access, rises: [u8; 8]) -> (Option<u8>, Option<u8>) {
        // The read's latch, and the requests' latch, close at the start of E.
        let read = matches!(access, Access::ReadIf).then_some(self.flags | !INT_MASK);
        let selected = lowest(self.ie & self.flags);
        let taken = matches!(access, Access::Entry).then_some(selected);
        for (phase, rise) in PHASES.iter().zip(rises) {
            let write = match access {
                Access::WriteIf(val) if *phase <= Phase::G => Some(val),
                _ => None,
            };
            let ack = match access {
                Access::Entry if (Phase::G..=Phase::H).contains(phase) => selected,
                _ => 0,
            };
            self.phase(rise, write, ack);
        }
        (read, taken)
    }
}
