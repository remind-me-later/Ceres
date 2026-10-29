//! Kani proofs that join the blocks' proofs against the netlist
//! (`cargo kani -p ceres-core --harness netlist_timer_and_if`).
//!
//! Each block's proof leaves what comes from the other blocks free, and
//! cuts the M-cycle where it suits it: the timer's
//! (`timing/netlist_proofs.rs`) at the start of phase B, after the A edge,
//! IF's (`interrupts/netlist_proofs.rs`) at the start of E, where ceres's
//! M-cycle begins, with every source's rise free and each phase standing
//! for one of ceres's T-cycles.
//!
//! Joined, the timer's interrupt is no longer free. On the netlist the timer
//! raises it at the A edge and IF sees it rise in phase A; on ceres the
//! timer requests it in IF in a T-cycle of its own, as the machine does. The
//! proof is a bisimulation of the two blocks together, cut at E: from any
//! pair of states that the two proofs relate, an M-cycle with any access to
//! the timer or to IF, and any rises of the other sources, gives the same
//! reads and the same interrupt taken, and leaves the pair related. It also
//! shows that the timer does nothing in B to D, so that its cut can move
//! from B to E.

use crate::{
    interrupts::{
        Interrupts,
        netlist_proofs::{self as if_proof, A, NetIf, TIMER, ceres_boundary, lowest, requests},
    },
    timing::netlist_proofs::{
        self as timer_proof, DmgHost, ceres_access, ceres_for, ceres_t_cycle, known_difference,
        related,
    },
};

/// From any related pair, an M-cycle with any access to the timer or to IF
/// gives the same reads and interrupt taken, and leaves the pair related.
// @assumes transcription-timer
// @assumes transcription-if
// @assumes mapping-m-cycle-at-e
// @assumes mapping-timer-a-edge
// @assumes timer-known-differences
#[kani::proof]
#[kani::unwind(9)]
fn netlist_timer_and_if() {
    let mut net = timer_proof::any_net_timer();
    let mut clock = ceres_for(&net);
    kani::assume(related(&net, &clock));
    let mut net_if = NetIf {
        flags: kani::any::<u8>() & crate::interrupts::INT_MASK,
        ie: kani::any(),
    };
    let mut host = DmgHost {
        serial_div_mask: kani::any(),
        timer_interrupts: 0,
        ints: Interrupts::default(),
    };
    host.ints.write_if(net_if.flags);
    host.ints.write_ie(net_if.ie);

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

    // The netlist: B to D change nothing, so the timer's M-cycle, cut at B,
    // is the one cut at E; its interrupt rises at the A edge.
    assert!(
        net.quiet_until_e(timer_access),
        "the timer changes in B to D"
    );
    let (net_timer_read, net_irq) = net.m_cycle(timer_access);
    if net_irq {
        rises[A] |= TIMER;
    }
    let (net_if_read, net_taken) = net_if.m_cycle(if_access, rises);

    // Ceres: the accesses at the boundary, then four T-cycles, in each of
    // which the timer runs, then IF takes the other requests, the write and
    // the acknowledge.
    let ceres_timer_read = ceres_access(&mut clock, &mut host, timer_access);
    let (ceres_if_read, queue) = ceres_boundary(&host.ints, if_access);
    let ceres_taken = matches!(if_access, if_proof::Access::Entry).then_some(lowest(queue));
    let other = requests(&rises);
    for (t, &request) in other.iter().enumerate() {
        ceres_t_cycle(&mut clock, &mut host);
        if_proof::ceres_t_cycle(&mut host.ints, if_access, t, request, queue);
    }

    assert!(
        net_timer_read == ceres_timer_read,
        "the timer's read differs"
    );
    assert!(net_if_read == ceres_if_read, "IF's read differs");
    assert!(net_taken == ceres_taken, "the interrupt taken differs");
    assert!(
        net_if.flags == host.ints.read_if() & crate::interrupts::INT_MASK,
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
