//! Kani proofs that ceres's timer is the netlist's, on the DMG
//! (`cargo kani -p ceres-core --harness netlist`).
//!
//! `NetTimer` (`netlist.rs`) transcribes `hdl/rtl/divider.vhd` and
//! `hdl/rtl/timer.vhd` phase by phase.
//!
//! Ceres makes a CPU access at the start of its M-cycle and then runs its
//! four T-cycles; the netlist's A edge comes in the third or the fourth
//! (the divider counts in the fourth, the interrupt in the third). So at an
//! M-cycle boundary ceres's T-cycle counter is four times the netlist's
//! M-cycle counter, and the proof is a bisimulation: from any pair of states
//! that `related` matches, an M-cycle with any access to FF04-FF07 gives the
//! same read and the same interrupt on both sides, and leaves them related.

pub(crate) use super::netlist::{Access, NetTimer, Reg};
use super::timer::{Clock, TimerHost};
use crate::interrupts::Interrupts;

/// A DMG in single speed that counts the timer interrupts, and requests them
/// in IF as the machine does.
pub(crate) struct DmgHost {
    pub(crate) serial_div_mask: u16,
    pub(crate) timer_interrupts: u32,
    pub(crate) ints: Interrupts,
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
        self.ints.request_timer();
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

fn any_reg() -> Reg {
    match kani::any::<u8>() & 3 {
        0 => Reg::Div,
        1 => Reg::Tima,
        2 => Reg::Tma,
        _ => Reg::Tac,
    }
}

pub(crate) fn any_access() -> Access {
    match kani::any::<u8>() % 3 {
        0 => Access::None,
        1 => Access::Read(any_reg()),
        _ => Access::Write(any_reg(), kani::any()),
    }
}

/// Ceres's access, at the start of its M-cycle: what a read reads.
pub(crate) fn ceres_access(clock: &mut Clock, host: &mut DmgHost, access: Access) -> Option<u8> {
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

/// Ceres's T-cycle.
pub(crate) fn ceres_t_cycle(clock: &mut Clock, host: &mut DmgHost) {
    clock.run(host, 1);
}

/// Ceres's M-cycle: the access, then four T-cycles.
fn ceres_m_cycle(clock: &mut Clock, host: &mut DmgHost, access: Access) -> (Option<u8>, bool) {
    let read = ceres_access(clock, host, access);
    let before = host.timer_interrupts;
    for t in 0..4 {
        let interrupts = host.timer_interrupts;
        ceres_t_cycle(clock, host);
        // The netlist requests it at the A edge, which IF sees in the third
        // T-cycle (`interrupts/netlist_proofs.rs`; `netlist_proofs.rs` joins
        // the two).
        assert!(
            host.timer_interrupts == interrupts || t >= 2,
            "the interrupt comes too early"
        );
    }
    (read, host.timer_interrupts != before)
}

/// The states match: what ceres keeps for the netlist's flip-flops.
pub(crate) fn related(net: &NetTimer, clock: &Clock) -> bool {
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

pub(crate) fn any_net_timer() -> NetTimer {
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
pub(crate) fn ceres_for(net: &NetTimer) -> Clock {
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

/// Where ceres parts from the netlist: each is a fact of `doc/facts.toml`.
pub(crate) fn known_difference(net: &NetTimer, access: Access) -> bool {
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
    // 2. @fact tima-reload-unseen-overflow
    //    The load still holds bit7_before at 0 at the A edge that ends that
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
        ints: Interrupts::default(),
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
