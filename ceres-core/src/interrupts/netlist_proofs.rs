//! Kani proofs that ceres's IF is the netlist's
//! (`cargo kani -p ceres-core --harness netlist_if`).
//!
//! `NetIf` (`netlist.rs`) transcribes `hdl/rtl/interrupts.vhd`, and the
//! latch of the requests and the acknowledge of
//! `hdl/rtl/sm83_interrupts.vhd`, phase by phase.
//!
//! Ceres's M-cycle boundary is the start of the netlist's E: a read there
//! sees the four T-cycles before it, an IF write lands after the first
//! T-cycle (`ConflictType::WriteCpu`), and the entry acknowledges after the
//! second (`bus.defer(2)` in `dispatch_interrupt`). So the T-cycle in which
//! a source rises in ceres stands for these phases of the netlist:
//!
//! | T-cycle | phases            |
//! | ------- | ----------------- |
//! | 1       | E, F, G           |
//! | 2       | H                 |
//! | 3, 4    | A to D (the next) |
//!
//! and the proof is a bisimulation under that mapping: from equal IF and IE,
//! an M-cycle with any access and any rises of the sources gives the same
//! read, the same interrupt taken, and equal IF after.

pub(crate) use super::netlist::{A, Access, NetIf, TIMER, lowest};
use super::{
    INT_MASK, Interrupts,
    netlist::{PHASES, t_cycle},
};

pub(crate) fn any_access() -> Access {
    match kani::any::<u8>() & 3 {
        0 => Access::None,
        1 => Access::ReadIf,
        2 => Access::WriteIf(kani::any()),
        _ => Access::Entry,
    }
}

/// The requests of each of ceres's T-cycles, from the rises of each phase.
pub(crate) fn requests(rises: &[u8; 8]) -> [u8; 4] {
    let mut requests = [0; 4];
    for (phase, &rise) in PHASES.iter().zip(rises) {
        requests[t_cycle(*phase)] |= rise & INT_MASK;
    }
    requests
}

/// Ceres at the boundary of its M-cycle: what a read of IF reads, and the
/// interrupts an entry chooses from.
pub(crate) fn ceres_boundary(ints: &Interrupts, access: Access) -> (Option<u8>, u8) {
    let read = matches!(access, Access::ReadIf).then(|| ints.read_if());
    let queue = ints.read_ie() & INT_MASK & ints.read_if() & INT_MASK;
    (read, queue)
}

/// Ceres's T-cycle `t` (0 to 3): its requests, then the IF write after the
/// first and the acknowledge after the second.
pub(crate) fn ceres_t_cycle(
    ints: &mut Interrupts,
    access: Access,
    t: usize,
    request: u8,
    queue: u8,
) {
    ints.ifr |= request;
    match access {
        Access::WriteIf(val) if t == 0 => ints.write_if(val),
        Access::Entry if t == 1 && queue != 0 => ints.acknowledge_interrupt(lowest(queue)),
        _ => {}
    }
}

/// Ceres's M-cycle: the access at the boundary, the requests of each
/// T-cycle, the IF write after the first and the acknowledge after the
/// second.
fn ceres_m_cycle(
    ints: &mut Interrupts,
    access: Access,
    rises: &[u8; 8],
) -> (Option<u8>, Option<u8>) {
    let (read, queue) = ceres_boundary(ints, access);
    let taken = matches!(access, Access::Entry).then_some(lowest(queue));
    for (t, &request) in requests(rises).iter().enumerate() {
        ceres_t_cycle(ints, access, t, request, queue);
    }
    (read, taken)
}

/// From equal IF and IE, an M-cycle with any access and any rises of the
/// sources gives the same read, the same interrupt taken, and equal IF.
// @assumes transcription-if
// @assumes mapping-m-cycle-at-e
// @discharges if-sees-rises-by-t-cycle
#[kani::proof]
#[kani::unwind(9)]
fn netlist_if_bisimulation() {
    let flags = kani::any::<u8>() & INT_MASK;
    let ie: u8 = kani::any();
    let mut net = NetIf { flags, ie };
    let mut ints = Interrupts::default();
    ints.write_if(flags);
    ints.write_ie(ie);
    let access = any_access();
    let rises: [u8; 8] = kani::any();
    let (net_read, net_taken) = net.m_cycle(access, rises);
    let (ceres_read, ceres_taken) = ceres_m_cycle(&mut ints, access, &rises);
    assert!(net_read == ceres_read, "the read differs");
    assert!(net_taken == ceres_taken, "the interrupt taken differs");
    assert!(net.flags == ints.ifr, "IF parts");
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
}
