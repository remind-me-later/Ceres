//! Kani proofs that ceres's timer is the netlist's, on the DMG
//! (`cargo kani -p ceres-core --harness netlist`): `check::step_agrees`,
//! from every pair of states.

use super::{check::step_agrees, timer::Clock};
pub(crate) use ceres_netlist::timer::{Access, NetTimer, Reg};

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

/// Any netlist timer: every flip-flop free, so that a proof from it holds
/// for every state `related` matches, not only the ones it was built from.
pub(crate) fn any_net_timer() -> NetTimer {
    NetTimer {
        count: kani::any(),
        tima: kani::any(),
        tma: kani::any(),
        tac: kani::any(),
        int_timer: kani::any(),
        bit7_before: kani::any(),
        timer_clock: kani::any(),
        write_tma: kani::any(),
        write_tac: kani::any(),
        latched_data: kani::any(),
        enable_race_counts: kani::any(),
    }
}

/// Any ceres timer: every field free, as for `any_net_timer`.
pub(crate) fn any_clock() -> Clock {
    Clock {
        div: kani::any(),
        tac: kani::any(),
        tima: kani::any(),
        tma: kani::any(),
        tima_reload_pending: kani::any(),
        tima_irq_countdown: kani::any(),
        div_cycles: kani::any(),
        div_state: kani::any(),
        stopped: kani::any(),
        during_div_write: kani::any(),
        apu_delay: kani::any(),
        apu_late_bit: kani::any(),
        apu_odd: kani::any(),
    }
}

/// The step, for every argument: from any related pair, an M-cycle with any
/// access gives the same read and interrupt, and leaves the pair related,
/// unless it is a known difference. Lean (`lean/`) takes it as given.
// @assumes transcription-timer
// @assumes mapping-timer-a-edge
// @assumes timer-known-differences
// @assumes timer-enable-race-outcome
// @discharges timer-step-agrees
#[kani::proof]
#[kani::unwind(9)]
fn netlist_timer_bisimulation() {
    assert!(step_agrees(
        any_net_timer(),
        any_clock(),
        any_access(),
        kani::any()
    ));
}
