//! IF's bisimulation with the netlist, as one function that Kani proves for
//! every argument and Aeneas takes to Lean (`lean/`), as `timing::check`
//! does for the timer.
//!
//! `NetIf` (`ceres_netlist::interrupts`) transcribes `hdl/rtl/interrupts.vhd`
//! and the latch of the requests and the acknowledge of
//! `hdl/rtl/sm83_interrupts.vhd`, phase by phase; `netlist_proofs.rs` gives
//! the mapping of ceres's T-cycles onto the netlist's phases. The timer's
//! interrupt is an input of the step on both sides: the netlist's rises at
//! the A edge, ceres's in the T-cycle in which its timer requests it, the
//! third (`timing::check::same_interrupt`). So Lean can wire the timer's
//! step into this one.
//!
//! Written as Aeneas takes it, as ceres-netlist is (see its crate
//! documentation).
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

use super::{INT_MASK, Interrupts};
use crate::timing::check::{same_interrupt, same_read};
use ceres_netlist::interrupts::{A, Access, NetIf, PHASES, TIMER, lowest, t_cycle};

/// IF and IE match.
pub(crate) const fn related(net: NetIf, ints: &Interrupts) -> bool {
    (net.flags & !INT_MASK == 0) & (ints.ifr == net.flags) & (ints.ie == net.ie)
}

/// Two IFs alike, IE and IF.
pub(crate) const fn same_ints(a: &Interrupts, b: &Interrupts) -> bool {
    (a.ie == b.ie) & (a.ifr == b.ifr)
}

/// The requests, by ceres's T-cycle, that the netlist's rises by phase
/// stand for.
pub(crate) const fn requests(rises: [u8; 8]) -> [u8; 4] {
    let mut requests = [0; 4];
    let mut i = 0;
    while i < 8 {
        let t = t_cycle(PHASES[i]);
        requests[t] |= rises[i] & INT_MASK;
        i += 1;
    }
    requests
}

/// What ceres does at its M-cycle boundary: what a read of IF reads, and
/// the requests that are enabled, which an entry chooses from.
pub(crate) const fn ceres_boundary(ints: &Interrupts, access: Access) -> (Option<u8>, u8) {
    let read = match access {
        Access::ReadIf => Some(ints.read_if()),
        Access::None | Access::WriteIf(_) | Access::Entry => None,
    };
    let queue = ints.read_ie() & INT_MASK & ints.read_if() & INT_MASK;
    (read, queue)
}

/// Ceres's T-cycle `t` (0 to 3): its requests, then an IF write (in the
/// first) or the entry's acknowledge (in the second).
pub(crate) const fn ceres_t_cycle(
    ints: &mut Interrupts,
    access: Access,
    t: usize,
    request: u8,
    queue: u8,
) {
    ints.ifr |= request;
    match access {
        Access::WriteIf(val) => {
            if t == 0 {
                ints.write_if(val);
            }
        }
        Access::Entry => {
            if (t == 1) & (queue != 0) {
                ints.acknowledge_interrupt(lowest(queue));
            }
        }
        Access::None | Access::ReadIf => {}
    }
}

/// The netlist's M-cycle, from E, with the other sources' rises and the
/// timer's: what a read reads, and the interrupt an entry takes.
pub(crate) fn net_step(
    net: &mut NetIf,
    access: Access,
    rises: [u8; 8],
    timer: bool,
) -> (Option<u8>, Option<u8>) {
    let mut rises = rises;
    if timer {
        rises[A] |= TIMER;
    }
    net.m_cycle(access, rises)
}

/// Ceres's M-cycle: the boundary, then four T-cycles, the timer's request
/// in its own.
pub(crate) fn ceres_step(
    ints: &mut Interrupts,
    access: Access,
    rises: [u8; 8],
    timer: Option<u8>,
) -> (Option<u8>, Option<u8>) {
    let mut requests = requests(rises);
    if let Some(t) = timer {
        requests[usize::from(t & 3)] |= TIMER;
    }
    let (read, queue) = ceres_boundary(ints, access);
    let taken = match access {
        Access::Entry => Some(lowest(queue)),
        Access::None | Access::ReadIf | Access::WriteIf(_) => None,
    };
    let mut t = 0;
    while t < 4 {
        ceres_t_cycle(ints, access, t, requests[t], queue);
        t += 1;
    }
    (read, taken)
}

/// The step: from a related pair, an M-cycle with any access, any rises of
/// the sources and the timer's interrupt alike on both sides gives the same
/// read and the same interrupt taken, and a related pair. True for every
/// argument (Kani's `netlist_if_bisimulation`).
pub(crate) fn step_agrees(
    mut net: NetIf,
    mut ints: Interrupts,
    access: Access,
    rises: [u8; 8],
    net_timer: bool,
    ceres_timer: Option<u8>,
) -> bool {
    if !related(net, &ints) | !same_interrupt(net_timer, ceres_timer) {
        return true;
    }
    let (net_read, net_taken) = net_step(&mut net, access, rises, net_timer);
    let (ceres_read, ceres_taken) = ceres_step(&mut ints, access, rises, ceres_timer);
    #[cfg(kani)]
    {
        kani::cover!(
            matches!(access, Access::WriteIf(_)) && rises[3] != 0 && net.flags & rises[3] != 0,
            "a rise in H survives the write"
        );
        kani::cover!(
            matches!(access, Access::Entry)
                && net_taken != Some(0)
                && net.flags & net_taken.unwrap_or(0) != 0,
            "the source rises again after the acknowledge"
        );
        kani::cover!(
            ceres_timer.is_some()
                && matches!(access, Access::Entry)
                && net_taken == Some(TIMER)
                && net.flags & TIMER != 0,
            "the timer's interrupt rises again as the entry takes it"
        );
    }
    same_read(net_read, ceres_read) & same_read(net_taken, ceres_taken) & related(net, &ints)
}
