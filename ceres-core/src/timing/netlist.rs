//! The netlist's divider and timer, transcribed from `hdl/rtl/divider.vhd`
//! and `hdl/rtl/timer.vhd` phase by phase, for the proofs against ceres
//! (`netlist_proofs.rs`) and for the replay of the model's traces
//! (`crate::netlist_replay`), which checks the transcription.
//!
//! Eight phases, A to H, make an M-cycle: the counter and the overflow
//! flip-flops change at the start of A, the CPU writes in E to G and TIMA
//! takes the data bus in E and F, and a read takes what the timer drives at
//! the end of H.

// Some of it only the proofs use, which other builds leave out.
#![cfg_attr(not(kani), allow(dead_code))]
#![expect(
    clippy::field_scoped_visibility_modifiers,
    reason = "the proofs and the replay set and compare the flip-flops one by one"
)]

use super::timer::{TAC_CLOCK, TAC_ENABLE_B};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Reg {
    Div,
    Tima,
    Tma,
    Tac,
}

#[derive(Clone, Copy)]
pub(crate) enum Access {
    None,
    Read(Reg),
    Write(Reg, u8),
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Phase {
    B,
    C,
    D,
    E,
    F,
    G,
    H,
}

/// The netlist's divider and timer, between phases.
#[derive(Clone, Copy, PartialEq, Eq)]
#[expect(clippy::struct_excessive_bools, reason = "the netlist's flip-flops")]
pub(crate) struct NetTimer {
    /// The divider: M-cycles.
    pub(crate) count: u16,
    pub(crate) tima: u8,
    pub(crate) tma: u8,
    /// TAC's three flip-flops.
    pub(crate) tac: u8,
    /// NYDU, MOBA: TIMA overflowed, reload it and request the interrupt.
    pub(crate) int_timer: bool,
    /// TIMA's bit 7 at the last A edge.
    pub(crate) bit7_before: bool,
    /// The timer's clock as it stands.
    pub(crate) timer_clock: bool,
    /// The write strobes as they stood in the phase before.
    pub(crate) write_tma: bool,
    pub(crate) write_tac: bool,
    /// The data bus during the write, which TMA and TAC take as it ends.
    pub(crate) latched_data: u8,
    /// Whether enabling the timer while its select changes counts, as in
    /// dmg-sim (`doc/hardware-tests.md`). The DMG that
    /// gbmicrotest was checked on does not count.
    // @fact timer-enable-race
    pub(crate) enable_race_counts: bool,
}

impl NetTimer {
    pub(crate) const fn clock_of(tac: u8, select: u8, count: u16) -> bool {
        let bit = match select & TAC_CLOCK {
            0 => 7,
            1 => 1,
            2 => 3,
            _ => 5,
        };
        tac & TAC_ENABLE_B != 0 && count & (1 << bit) != 0
    }

    /// TIMA's load: during the M-cycle after an overflow, and while the CPU
    /// writes it outside G and H.
    const fn load(&self, write_tima: bool, t4: bool) -> bool {
        self.int_timer || (write_tima && !t4)
    }

    /// TIMA follows its load value while it loads, and otherwise counts when
    /// the timer's clock falls.
    const fn settle(&mut self, write_tima: bool, t4: bool, data: u8, clock: bool) {
        let load = self.load(write_tima, t4);
        if load {
            self.tima = if write_tima { data } else { self.tma };
            self.bit7_before = false;
        } else if self.timer_clock && !clock {
            self.tima = self.tima.wrapping_add(1);
        } else {
            // TIMA keeps its value.
        }
        self.timer_clock = clock;
    }

    /// One phase from B to H, with the CPU's access.
    #[expect(clippy::similar_names, reason = "TMA's and TAC's write strobes")]
    pub(crate) fn phase(&mut self, phase: Phase, access: Access) {
        let (reg, data) = match access {
            Access::Write(reg, data) if (Phase::E..=Phase::G).contains(&phase) => (Some(reg), data),
            _ => (None, 0),
        };
        let t4 = phase >= Phase::G;
        let write_tima = reg == Some(Reg::Tima);

        // Writing DIV clears the counter for as long as the write lasts.
        if reg == Some(Reg::Div) {
            self.count = 0;
        }

        // TMA and TAC take the data bus when the write ends.
        let write_tma = reg == Some(Reg::Tma);
        let write_tac = reg == Some(Reg::Tac);
        if self.write_tma && !write_tma {
            self.tma = self.latched_data;
        }
        self.write_tma = write_tma;
        if self.write_tac && !write_tac {
            let new = self.latched_data & 7;
            // The enable arrives a moment before the select.
            let between = Self::clock_of(new, self.tac, self.count);
            if self.enable_race_counts || self.timer_clock {
                self.settle(write_tima, t4, data, between);
            }
            self.tac = new;
        }
        let clock = Self::clock_of(self.tac, self.tac, self.count);
        self.write_tac = write_tac;
        if reg.is_some() {
            self.latched_data = data;
        }

        self.settle(write_tima, t4, data, clock);
    }

    /// The A edge: the counter counts and the overflow flip-flops take TIMA's
    /// bit 7, then TIMA loads or counts (a delta cycle later).
    pub(crate) const fn edge_a(&mut self) {
        let load = self.load(false, false);
        let overflow = self.bit7_before && self.tima & 0x80 == 0;
        if !load {
            self.bit7_before = self.tima & 0x80 != 0;
        }
        self.int_timer = overflow;
        self.count = self.count.wrapping_add(1);
        let clock = Self::clock_of(self.tac, self.tac, self.count);
        self.settle(false, false, 0, clock);
    }

    #[expect(clippy::cast_possible_truncation, reason = "DIV shows bits 6 to 13")]
    pub(crate) const fn read(&self, reg: Reg) -> u8 {
        match reg {
            Reg::Div => (self.count >> 6) as u8,
            Reg::Tima => self.tima,
            Reg::Tma => self.tma,
            Reg::Tac => self.tac,
        }
    }

    /// An M-cycle: phases B to H with the access, then the next A edge.
    /// Returns what a read reads and whether the interrupt was requested.
    pub(crate) fn m_cycle(&mut self, access: Access) -> (Option<u8>, bool) {
        for phase in [
            Phase::B,
            Phase::C,
            Phase::D,
            Phase::E,
            Phase::F,
            Phase::G,
            Phase::H,
        ] {
            self.phase(phase, access);
        }
        let read = match access {
            Access::Read(reg) => Some(self.read(reg)),
            _ => None,
        };
        let before = self.int_timer;
        self.edge_a();
        (read, !before && self.int_timer)
    }

    /// Whether phases B to D leave the timer as it is: then the M-cycle can
    /// as well be cut at the start of E, where ceres's begins, as at the
    /// start of B.
    pub(crate) fn quiet_until_e(&self, access: Access) -> bool {
        let mut net = *self;
        for phase in [Phase::B, Phase::C, Phase::D] {
            net.phase(phase, access);
        }
        net == *self
    }
}
