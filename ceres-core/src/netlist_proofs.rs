//! Kani proofs that join the blocks' proofs against the netlist
//! (`cargo kani -p ceres-core --harness netlist_timer_and_if`), and that
//! ceres's M-cycle of the two is the blocks' steps in sequence
//! (`--harness ceres_timer_and_if_in_sequence`).
//!
//! Each block's proof leaves what comes from the other blocks free, and
//! cuts the M-cycle at the start of E, where ceres's begins: the timer's
//! (`timing::check`) and IF's (`interrupts::check`), with every source's
//! rise free and each phase standing for one of ceres's T-cycles.
//!
//! Joined, the timer's interrupt is no longer free. On the netlist the timer
//! raises it at the A edge and IF sees it rise in phase A; on ceres the
//! timer requests it in IF in a T-cycle of its own, as the machine does
//! (`check::ceres_m_cycle_joint`). `netlist_timer_and_if` proves the two
//! blocks together: from any pair of states that the two proofs relate, an
//! M-cycle with any access to the timer or to IF, and any rises of the
//! other sources, gives the same reads and the same interrupt taken, and
//! leaves the pair related. Lean (`lean/`) proves the same from the blocks'
//! steps, wired together, and `ceres_timer_and_if_in_sequence`.

use crate::{
    check::{JointHost, ceres_m_cycle_joint, in_sequence},
    interrupts::netlist_proofs::{self as if_proof, A, NetIf, TIMER, any_ints},
    timing::{
        check::{known_difference, related},
        netlist_proofs::{self as timer_proof, any_clock},
    },
};

/// From any related pair, an M-cycle with any access to the timer or to IF
/// gives the same reads and interrupt taken, and leaves the pair related.
// @assumes transcription-timer
// @assumes transcription-if
// @assumes mapping-m-cycle-at-e
// @assumes mapping-timer-a-edge
// @assumes timer-known-differences
// @assumes timer-enable-race-outcome
#[kani::proof]
#[kani::unwind(9)]
fn netlist_timer_and_if() {
    let mut net = timer_proof::any_net_timer();
    let mut clock = any_clock();
    kani::assume(related(&net, &clock));
    let mut net_if = NetIf {
        flags: kani::any::<u8>() & crate::interrupts::INT_MASK,
        ie: kani::any(),
    };
    let mut ints = crate::interrupts::Interrupts::default();
    ints.write_if(net_if.flags);
    ints.write_ie(net_if.ie);
    let mut host = JointHost::new(kani::any(), ints);

    // One access, to one block or the other.
    let on_timer: bool = kani::any();
    let timer_access = if on_timer {
        timer_proof::any_access()
    } else {
        timer_proof::Access::None
    };
    let if_access = if on_timer {
        if_proof::Access::None
    } else {
        if_proof::any_access()
    };
    kani::assume(!known_difference(&net, timer_access));
    // The other sources rise when they like; the timer's comes from the timer.
    let mut rises: [u8; 8] = kani::any();
    for rise in &mut rises {
        *rise &= !TIMER;
    }

    // The netlist: the timer's interrupt rises at the A edge.
    let (net_timer_read, net_irq) = net.m_cycle(timer_access);
    let mut net_rises = rises;
    if net_irq {
        net_rises[A] |= TIMER;
    }
    let (net_if_read, net_taken) = net_if.m_cycle(if_access, net_rises);

    let (ceres_timer_read, ceres_if_read, ceres_taken) =
        ceres_m_cycle_joint(&mut clock, &mut host, timer_access, if_access, rises);

    assert!(
        net_timer_read == ceres_timer_read,
        "the timer's read differs"
    );
    assert!(net_if_read == ceres_if_read, "IF's read differs");
    assert!(net_taken == ceres_taken, "the interrupt taken differs");
    assert!(
        net_if.flags == host.ints().read_if() & crate::interrupts::INT_MASK,
        "IF parts"
    );
    assert!(related(&net, &clock), "the timers part");
    kani::cover!(net_irq, "the timer's interrupt");
    kani::cover!(
        net_irq && matches!(if_access, if_proof::Access::WriteIf(val) if val & TIMER == 0),
        "the timer's interrupt survives a write of IF that clears it"
    );
    kani::cover!(
        net_irq && matches!(if_access, if_proof::Access::Entry) && net_taken == Some(TIMER),
        "the timer's interrupt rises again as the entry takes it"
    );
}

/// Ceres's M-cycle of the timer and IF, T-cycle by T-cycle, is the timer's
/// step and then IF's, from every state the timer's relation allows and
/// with every input.
// @discharges blocks-in-sequence
#[kani::proof]
#[kani::unwind(9)]
fn ceres_timer_and_if_in_sequence() {
    assert!(in_sequence(
        timer_proof::any_net_timer(),
        any_clock(),
        any_clock(),
        any_ints(),
        any_ints(),
        timer_proof::any_access(),
        kani::any(),
        if_proof::any_access(),
        kani::any()
    ));
}
