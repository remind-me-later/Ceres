//! The timer's bisimulation with the netlist, as one function that Kani
//! proves for every argument and Aeneas takes to Lean (`lean/`).
//!
//! `step_agrees` states the step: from any pair of states that `related`
//! matches, an M-cycle with any access to FF04-FF07 gives the same read and
//! the same interrupt on both sides, and leaves the pair related, unless it
//! is a `known_difference`. Kani proves it with every field of both states
//! free (`netlist_proofs.rs`); Lean takes that as a hypothesis and proves by
//! coinduction that related states agree on every M-cycle from then on.
//!
//! The netlist's timer, `NetTimer` (`ceres_netlist::timer`), transcribes
//! `hdl/rtl/divider.vhd` and `hdl/rtl/timer.vhd` phase by phase. Ceres makes
//! a CPU access at the start of its M-cycle and then runs its four
//! T-cycles; the netlist's A edge comes in the third or the fourth (the
//! divider counts in the fourth, the interrupt in the third), so at an
//! M-cycle boundary ceres's T-cycle counter is four times the netlist's
//! M-cycle counter.
//!
//! Written as Aeneas takes it, as ceres-netlist is (see its crate
//! documentation): booleans combine with `&` and `|`, and `Option`s are
//! taken apart with `match`.
#![cfg_attr(
    not(kani),
    expect(
        dead_code,
        reason = "outside Kani, only Charon reads the steps (`lean/translate.sh`)"
    )
)]
#![expect(
    clippy::needless_bitwise_bool,
    reason = "Aeneas (Lean) takes this module: a lazy && or || is a branch, and its symbolic execution forks on every one"
)]

use super::timer::{Clock, TimerHost};
use ceres_netlist::timer::{Access, NetTimer, Reg};

/// A DMG in single speed that counts the timer's interrupt requests.
pub(crate) struct CheckHost {
    serial_div_mask: u16,
    timer_interrupts: u32,
}

impl TimerHost for CheckHost {
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

/// Ceres's access, at the start of its M-cycle: what a read reads.
pub(crate) fn ceres_access<H: TimerHost>(
    clock: &mut Clock,
    host: &mut H,
    access: Access,
) -> Option<u8> {
    match access {
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
    }
}

/// Ceres's M-cycle: the access, then four T-cycles. What a read reads, the
/// T-cycle (0 to 3) in which the interrupt came if one did, and whether a
/// second came. The T-cycles are written out: Aeneas finds no fixed point
/// for a loop that carries the `Option`.
pub(crate) fn ceres_m_cycle(
    clock: &mut Clock,
    host: &mut CheckHost,
    access: Access,
) -> (Option<u8>, Option<u8>, bool) {
    let read = ceres_access(clock, host, access);
    let (first, twice) = ceres_t_cycle(clock, host, 0, None, false);
    let (first, twice) = ceres_t_cycle(clock, host, 1, first, twice);
    let (first, twice) = ceres_t_cycle(clock, host, 2, first, twice);
    let (first, twice) = ceres_t_cycle(clock, host, 3, first, twice);
    (read, first, twice)
}

/// T-cycle `t`: the T-cycle of the first interrupt so far, and whether a
/// second came.
fn ceres_t_cycle(
    clock: &mut Clock,
    host: &mut CheckHost,
    t: u8,
    first: Option<u8>,
    twice: bool,
) -> (Option<u8>, bool) {
    let before = host.timer_interrupts;
    clock.run(host, 1);
    if host.timer_interrupts == before {
        (first, twice)
    } else {
        match first {
            Some(_) => (first, true),
            None => (Some(t), twice),
        }
    }
}

/// The netlist's interrupt, at the A edge, and ceres's, which must come in
/// its third T-cycle (2), where IF's mapping puts A (`interrupts::check`).
pub(crate) const fn same_interrupt(net: bool, ceres: Option<u8>) -> bool {
    match ceres {
        Some(t) => net & (t == 2),
        None => !net,
    }
}

/// Two reads alike (Aeneas does not model `Option`'s equality).
pub(crate) const fn same_read(a: Option<u8>, b: Option<u8>) -> bool {
    match (a, b) {
        (Some(x), Some(y)) => x == y,
        (None, None) => true,
        _ => false,
    }
}

/// The states match: what ceres keeps for the netlist's flip-flops.
pub(crate) const fn related(net: &NetTimer, clock: &Clock) -> bool {
    let same_regs = (clock.div == net.count.wrapping_mul(4))
        & (clock.tma == net.tma)
        & (clock.tac & 7 == net.tac)
        & (clock.div_cycles == 0)
        & !clock.stopped
        & (clock.apu_delay == 0)
        & clock.apu_late_bit.is_none()
        & (net.timer_clock == NetTimer::clock_of(net.tac, net.tac, net.count))
        & !net.write_tma
        & !net.write_tac
        // The DMG that gbmicrotest was checked on: enabling the timer while
        // its select changes does not count.
        // @fact timer-enable-race
        & !net.enable_race_counts;
    let overflowed = net.bit7_before & (net.tima & 0x80 == 0);
    let timer = if net.int_timer {
        // The M-cycle after an overflow: TIMA loads TMA.
        (clock.tima_reload_pending == 5)
            & (clock.tima_irq_countdown == 0)
            & (net.tima == net.tma)
            & !net.bit7_before
            & (clock.tima == net.tma)
    } else if overflowed {
        // TIMA overflowed at this A edge, or a write's glitch did.
        (clock.tima_reload_pending == 4)
            & (clock.tima_irq_countdown == 3)
            & (net.tima == 0)
            & (clock.tima == net.tma)
    } else {
        (clock.tima_reload_pending == 0)
            & (clock.tima_irq_countdown == 0)
            & (clock.tima == net.tima)
    };
    same_regs & timer
}

/// Where ceres parts from the netlist: each is a fact of `doc/facts.toml`.
pub(crate) const fn known_difference(net: &NetTimer, access: Access) -> bool {
    // 1. @fact tima-reload-tick
    //    A tick between an overflow and the end of the reload is lost: TIMA
    //    loads TMA through the M-cycle after the overflow, from the A edge
    //    that starts it (the load comes before the divider's tick). Ceres
    //    holds TMA from the overflow on, and adds the tick to it.
    let now = NetTimer::clock_of(net.tac, net.tac, net.count);
    let (tac, count, glitch) = match access {
        Access::Write(Reg::Tac, val) => {
            let new = val & 7;
            let between = NetTimer::clock_of(new, net.tac, net.count);
            let after = NetTimer::clock_of(new, new, net.count);
            (new, net.count, now & !(between & after))
        }
        Access::Write(Reg::Div, _) => (net.tac, 0, now),
        _ => (net.tac, net.count, false),
    };
    let tma = match access {
        Access::Write(Reg::Tma, val) => val,
        _ => net.tma,
    };
    let overflowed = !net.int_timer & net.bit7_before & (net.tima & 0x80 == 0);
    let falls_at_edge =
        NetTimer::clock_of(tac, tac, count) & !NetTimer::clock_of(tac, tac, count.wrapping_add(1));
    let lost_tick = (net.int_timer & glitch) | (overflowed & (glitch | falls_at_edge));
    // 2. @fact tima-reload-unseen-overflow
    //    The load still holds bit7_before at 0 at the A edge that ends that
    //    M-cycle, so a tick at that edge, or a glitch in the next M-cycle,
    //    that takes a TIMA of FF (a TMA of FF) to 00 overflows unseen; ceres
    //    reloads and interrupts.
    let unseen_overflow = (!net.int_timer & !net.bit7_before & (net.tima == 0xFF) & glitch)
        | (net.int_timer & (tma == 0xFF) & falls_at_edge);
    lost_tick | unseen_overflow
}

/// The step: from a related pair, an M-cycle with `access` gives the same
/// read and the same interrupt (in ceres's third T-cycle, and no
/// second), and a related pair; unless it is a known difference. True for
/// every argument (Kani's `netlist_timer_bisimulation`).
pub(crate) fn step_agrees(
    mut net: NetTimer,
    mut clock: Clock,
    access: Access,
    serial_div_mask: u16,
) -> bool {
    if !related(&net, &clock) | known_difference(&net, access) {
        return true;
    }
    let mut host = CheckHost {
        serial_div_mask,
        timer_interrupts: 0,
    };
    #[cfg(kani)]
    let (reloading, tma) = (net.int_timer, net.tma);
    let (net_read, net_irq) = net.m_cycle(access);
    let (ceres_read, ceres_irq, twice) = ceres_m_cycle(&mut clock, &mut host, access);
    #[cfg(kani)]
    {
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
            matches!(access, Access::Write(Reg::Tima, val) if val != tma) && reloading,
            "TIMA write ignored"
        );
        kani::cover!(
            matches!(access, Access::Read(Reg::Tima)) && net_read == Some(0),
            "reads 0"
        );
    }
    same_read(net_read, ceres_read)
        & same_interrupt(net_irq, ceres_irq)
        & !twice
        & related(&net, &clock)
}
