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
//! third (`timing::check::same_interrupt`). So is the STAT line's: the
//! netlist's rises by phase, ceres's by T-cycle (`same_stat`), as the line
//! timing's step gives them (`ppu::display::timing::check`). So Lean can
//! wire the timer's step and the line timing's into this one.
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

use super::{INT_LCD_B, INT_MASK, Interrupts};
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

/// The STAT line's rises alike, as IF tells them apart: the netlist's by
/// phase, from E, and ceres's by T-cycle, the first T-cycle standing for E
/// to G, the second for H and the last two for A to D, which IF does not
/// tell apart (no write or acknowledge comes there).
pub(crate) const fn same_stat(net: [bool; 8], ceres: [bool; 4]) -> bool {
    ((net[0] | net[1] | net[2]) == ceres[0])
        & (net[3] == ceres[1])
        & ((net[4] | net[5] | net[6] | net[7]) == (ceres[2] | ceres[3]))
}

/// Ceres's requests by T-cycle with the STAT line's rises added.
pub(crate) const fn with_stat(requests: [u8; 4], stat: [bool; 4]) -> [u8; 4] {
    let mut requests = requests;
    let mut t = 0;
    while t < 4 {
        if stat[t] {
            requests[t] |= INT_LCD_B;
        }
        t += 1;
    }
    requests
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

/// The netlist's M-cycle, from E, with the other sources' rises, the
/// timer's and the STAT line's: what a read reads, and the interrupt an
/// entry takes.
pub(crate) fn net_step(
    net: &mut NetIf,
    access: Access,
    rises: [u8; 8],
    timer: bool,
    stat: [bool; 8],
) -> (Option<u8>, Option<u8>) {
    let mut rises = rises;
    if timer {
        rises[A] |= TIMER;
    }
    let mut i = 0;
    while i < 8 {
        if stat[i] {
            rises[i] |= INT_LCD_B;
        }
        i += 1;
    }
    net.m_cycle(access, rises)
}

/// Ceres's M-cycle: the boundary, then four T-cycles, the timer's request
/// and the STAT line's in their own.
pub(crate) fn ceres_step(
    ints: &mut Interrupts,
    access: Access,
    rises: [u8; 8],
    timer: Option<u8>,
    stat: [bool; 4],
) -> (Option<u8>, Option<u8>) {
    let mut requests = with_stat(requests(rises), stat);
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
/// the sources, and the timer's interrupt and the STAT line's rises alike
/// on both sides gives the same read and the same interrupt taken, and a
/// related pair. True for every argument (Kani's
/// `netlist_if_bisimulation`).
#[expect(clippy::too_many_arguments, reason = "every input of the M-cycle")]
pub(crate) fn step_agrees(
    mut net: NetIf,
    mut ints: Interrupts,
    access: Access,
    rises: [u8; 8],
    net_timer: bool,
    ceres_timer: Option<u8>,
    net_stat: [bool; 8],
    ceres_stat: [bool; 4],
) -> bool {
    if !related(net, &ints)
        | !same_interrupt(net_timer, ceres_timer)
        | !same_stat(net_stat, ceres_stat)
    {
        return true;
    }
    let (net_read, net_taken) = net_step(&mut net, access, rises, net_timer, net_stat);
    let (ceres_read, ceres_taken) = ceres_step(&mut ints, access, rises, ceres_timer, ceres_stat);
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
        kani::cover!(
            net_stat[5] && ceres_stat[3] && !ceres_stat[2],
            "the STAT line rises in another of the last two T-cycles"
        );
    }
    same_read(net_read, ceres_read) & same_read(net_taken, ceres_taken) & related(net, &ints)
}
