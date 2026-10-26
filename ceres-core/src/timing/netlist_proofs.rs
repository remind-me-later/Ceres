//! Kani proofs that ceres's timer is the netlist's, on the DMG
//! (`cargo kani -p ceres-core --harness netlist`).
//!
//! `NetTimer` transcribes `hdl/rtl/divider.vhd` and `hdl/rtl/timer.vhd`
//! phase by phase: eight phases, A to H, make an M-cycle, the counter and
//! the overflow flip-flops change at the start of A, the CPU writes in E to
//! G and TIMA takes the data bus in E and F, and a read takes what the
//! timer drives at the end of H.
//!
//! Ceres makes a CPU access at the start of its M-cycle and then runs its
//! four T-cycles; the last of them is the netlist's A edge. So at an M-cycle
//! boundary ceres's T-cycle counter is four times the netlist's M-cycle
//! counter, and the proof is a bisimulation: from any pair of states that
//! `related` matches, an M-cycle with any access to FF04-FF07 gives the same
//! read and the same interrupt on both sides, and leaves them related.

use super::timer::{Clock, TAC_CLOCK, TAC_ENABLE_B, TimerHost};

/// A DMG in single speed that counts the timer interrupts.
struct DmgHost {
    serial_div_mask: u16,
    timer_interrupts: u32,
}

impl TimerHost for DmgHost {
    fn is_cgb_hardware(&self) -> bool {
        false
    }

    fn is_cgb_mode(&self) -> bool {
        false
    }

    fn double_speed(&self) -> bool {
        false
    }

    fn request_timer_interrupt(&mut self) {
        self.timer_interrupts += 1;
    }

    fn serial_div_mask(&self) -> u16 {
        self.serial_div_mask
    }

    fn serial_clock_edge(&mut self) {}

    fn apu_frame_sequencer_step(&mut self, _: &Clock) {}

    fn apu_envelopes_armed(&mut self) {}

    fn apu_delayed_envelope_tick(&mut self, _: &Clock) {}

    fn apu_m_cycle(&mut self, _: &Clock) {}
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Reg {
    Div,
    Tima,
    Tma,
    Tac,
}

#[derive(Clone, Copy)]
enum Access {
    None,
    Read(Reg),
    Write(Reg, u8),
}

fn any_reg() -> Reg {
    match kani::any::<u8>() & 3 {
        0 => Reg::Div,
        1 => Reg::Tima,
        2 => Reg::Tma,
        _ => Reg::Tac,
    }
}

fn any_access() -> Access {
    match kani::any::<u8>() % 3 {
        0 => Access::None,
        1 => Access::Read(any_reg()),
        _ => Access::Write(any_reg(), kani::any()),
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Phase {
    B,
    C,
    D,
    E,
    F,
    G,
    H,
}

/// The netlist's divider and timer, between phases.
#[derive(Clone, Copy)]
struct NetTimer {
    /// The divider: M-cycles.
    count: u16,
    tima: u8,
    tma: u8,
    /// TAC's three flip-flops.
    tac: u8,
    /// NYDU, MOBA: TIMA overflowed, reload it and request the interrupt.
    int_timer: bool,
    /// TIMA's bit 7 at the last A edge.
    bit7_before: bool,
    /// The timer's clock as it stands.
    timer_clock: bool,
    /// The write strobes as they stood in the phase before.
    write_tma: bool,
    write_tac: bool,
    /// The data bus during the write, which TMA and TAC take as it ends.
    latched_data: u8,
    /// Whether enabling the timer while its select changes counts, as in
    /// dmg-sim (`doc/dmg-hardware-tests.md`, entry 4). The DMG that
    /// gbmicrotest was checked on does not count.
    enable_race_counts: bool,
}

impl NetTimer {
    const fn clock_of(tac: u8, select: u8, count: u16) -> bool {
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
    fn settle(&mut self, write_tima: bool, t4: bool, data: u8, clock: bool) {
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
    fn phase(&mut self, phase: Phase, access: Access) {
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
        let mut clock = Self::clock_of(self.tac, self.tac, self.count);
        if self.write_tac && !write_tac {
            let new = self.latched_data & 7;
            // The enable arrives a moment before the select.
            let between = Self::clock_of(new, self.tac, self.count);
            if self.enable_race_counts || self.timer_clock {
                self.settle(write_tima, t4, data, between);
            }
            self.tac = new;
            clock = Self::clock_of(self.tac, self.tac, self.count);
        }
        self.write_tac = write_tac;
        if reg.is_some() {
            self.latched_data = data;
        }

        self.settle(write_tima, t4, data, clock);
    }

    /// The A edge: the counter counts and the overflow flip-flops take TIMA's
    /// bit 7, then TIMA loads or counts (a delta cycle later).
    fn edge_a(&mut self) {
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

    fn read(&self, reg: Reg) -> u8 {
        match reg {
            Reg::Div => (self.count >> 6) as u8,
            Reg::Tima => self.tima,
            Reg::Tma => self.tma,
            Reg::Tac => self.tac,
        }
    }

    /// An M-cycle: phases B to H with the access, then the next A edge.
    /// Returns what a read reads and whether the interrupt was requested.
    fn m_cycle(&mut self, access: Access) -> (Option<u8>, bool) {
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
}

/// Ceres's M-cycle: the access, then four T-cycles.
fn ceres_m_cycle(clock: &mut Clock, host: &mut DmgHost, access: Access) -> (Option<u8>, bool) {
    let read = match access {
        Access::None => None,
        Access::Read(Reg::Div) => Some(clock.read_div()),
        Access::Read(Reg::Tima) => Some(clock.tima()),
        Access::Read(Reg::Tma) => Some(clock.tma()),
        Access::Read(Reg::Tac) => Some(clock.read_tac() & 7),
        Access::Write(Reg::Div, _) => {
            clock.write_div(host);
            None
        }
        Access::Write(Reg::Tima, val) => {
            clock.write_tima(val);
            None
        }
        Access::Write(Reg::Tma, val) => {
            clock.write_tma(val);
            None
        }
        Access::Write(Reg::Tac, val) => {
            clock.write_tac(host, val);
            None
        }
    };
    let before = host.timer_interrupts;
    for _ in 0..4 {
        clock.run(host, 1);
    }
    (read, host.timer_interrupts != before)
}

/// The states match: what ceres keeps for the netlist's flip-flops.
fn related(net: &NetTimer, clock: &Clock) -> bool {
    let same_regs = clock.div == net.count.wrapping_mul(4)
        && clock.tma == net.tma
        && clock.tac & 7 == net.tac
        && clock.div_cycles == 0
        && !clock.stopped
        && clock.apu_delay == 0
        && clock.apu_late_bit.is_none()
        && net.timer_clock == NetTimer::clock_of(net.tac, net.tac, net.count)
        && !net.write_tma
        && !net.write_tac;
    let overflowed = net.bit7_before && net.tima & 0x80 == 0;
    let timer = if net.int_timer {
        // The M-cycle after an overflow: TIMA loads TMA.
        clock.tima_reload_pending == 5
            && clock.tima_irq_countdown == 0
            && net.tima == net.tma
            && !net.bit7_before
            && clock.tima == net.tma
    } else if overflowed {
        // TIMA overflowed at this A edge, or a write's glitch did.
        clock.tima_reload_pending == 4
            && clock.tima_irq_countdown == 3
            && net.tima == 0
            && clock.tima == net.tma
    } else {
        clock.tima_reload_pending == 0 && clock.tima_irq_countdown == 0 && clock.tima == net.tima
    };
    same_regs && timer
}

fn any_net_timer() -> NetTimer {
    let count: u16 = kani::any();
    let tac = kani::any::<u8>() & 7;
    NetTimer {
        count,
        tima: kani::any(),
        tma: kani::any(),
        tac,
        int_timer: kani::any(),
        bit7_before: kani::any(),
        timer_clock: NetTimer::clock_of(tac, tac, count),
        write_tma: false,
        write_tac: false,
        latched_data: 0,
        enable_race_counts: false,
    }
}

/// The ceres timer that `related` matches with `net`.
fn ceres_for(net: &NetTimer) -> Clock {
    let (tima, pending, countdown) = if net.int_timer {
        (net.tma, 5, 0)
    } else if net.bit7_before && net.tima & 0x80 == 0 {
        (net.tma, 4, 3)
    } else {
        (net.tima, 0, 0)
    };
    Clock {
        div: net.count.wrapping_mul(4),
        tac: net.tac,
        tima,
        tma: net.tma,
        tima_reload_pending: pending,
        tima_irq_countdown: countdown,
        ..Clock::default()
    }
}

/// Where ceres parts from the netlist.
fn known_difference(net: &NetTimer, access: Access) -> bool {
    // 1. A tick between an overflow and the end of the reload is lost: TIMA
    //    loads TMA through the M-cycle after the overflow, from the A edge
    //    that starts it (the load comes before the divider's tick). Ceres
    //    holds TMA from the overflow on, and adds the tick to it.
    let now = NetTimer::clock_of(net.tac, net.tac, net.count);
    let (tac, count, glitch) = match access {
        Access::Write(Reg::Tac, val) => {
            let new = val & 7;
            let between = NetTimer::clock_of(new, net.tac, net.count);
            let after = NetTimer::clock_of(new, new, net.count);
            (new, net.count, now && !(between && after))
        }
        Access::Write(Reg::Div, _) => (net.tac, 0, now),
        _ => (net.tac, net.count, false),
    };
    let tma = match access {
        Access::Write(Reg::Tma, val) => val,
        _ => net.tma,
    };
    let overflowed = !net.int_timer && net.bit7_before && net.tima & 0x80 == 0;
    let falls_at_edge =
        NetTimer::clock_of(tac, tac, count) && !NetTimer::clock_of(tac, tac, count.wrapping_add(1));
    let lost_tick = (net.int_timer && glitch) || (overflowed && (glitch || falls_at_edge));
    // 2. The load still holds bit7_before at 0 at the A edge that ends that
    //    M-cycle, so a tick at that edge, or a glitch in the next M-cycle,
    //    that takes a TIMA of FF (a TMA of FF) to 00 overflows unseen; ceres
    //    reloads and interrupts.
    let unseen_overflow = (!net.int_timer && !net.bit7_before && net.tima == 0xFF && glitch)
        || (net.int_timer && tma == 0xFF && falls_at_edge);
    lost_tick || unseen_overflow
}

/// From any related pair, an M-cycle with any access gives the same read and
/// interrupt, and leaves the pair related.
#[kani::proof]
#[kani::unwind(9)]
fn netlist_timer_bisimulation() {
    let mut net = any_net_timer();
    let mut clock = ceres_for(&net);
    kani::assume(related(&net, &clock));
    let access = any_access();
    kani::assume(!known_difference(&net, access));
    let mut host = DmgHost {
        serial_div_mask: kani::any(),
        timer_interrupts: 0,
    };
    let reloading = net.int_timer;
    let (net_read, net_irq) = net.m_cycle(access);
    let (ceres_read, ceres_irq) = ceres_m_cycle(&mut clock, &mut host, access);
    assert!(net_read == ceres_read, "the read differs");
    assert!(net_irq == ceres_irq, "the interrupt differs");
    assert!(related(&net, &clock), "the states part");
    kani::cover!(net_irq, "interrupt");
    kani::cover!(net.int_timer, "ends reloading");
    kani::cover!(net.bit7_before && net.tima == 0, "ends overflowed");
    kani::cover!(
        matches!(access, Access::Write(Reg::Tac, _)) && net_irq,
        "TAC write then interrupt"
    );
    kani::cover!(
        matches!(access, Access::Write(Reg::Div, _)) && net_irq,
        "DIV write then interrupt"
    );
    kani::cover!(
        matches!(access, Access::Write(Reg::Tima, val) if val != net.tma) && reloading,
        "TIMA write ignored"
    );
    kani::cover!(
        matches!(access, Access::Read(Reg::Tima)) && net_read == Some(0),
        "reads 0"
    );
}
