//! Kani proofs that ceres's IF is the netlist's
//! (`cargo kani -p ceres-core --harness netlist_if`).
//!
//! `NetIf` (`ceres_netlist::interrupts`) transcribes `hdl/rtl/interrupts.vhd`,
//! and the latch of the requests and the acknowledge of
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
//! read, the same interrupt taken, and equal IF after (`check::step_agrees`,
//! which also takes the timer's interrupt).

pub(crate) use super::check::{ceres_boundary, ceres_t_cycle, requests};
use super::{Interrupts, check::step_agrees};
pub(crate) use ceres_netlist::interrupts::{A, Access, NetIf, TIMER, lowest};

pub(crate) fn any_access() -> Access {
    match kani::any::<u8>() & 3 {
        0 => Access::None,
        1 => Access::ReadIf,
        2 => Access::WriteIf(kani::any()),
        _ => Access::Entry,
    }
}

/// The step, for every argument: from equal IF and IE, an M-cycle with any
/// access, any rises of the sources and the timer's interrupt gives the
/// same read, the same interrupt taken, and equal IF. Lean (`lean/`) takes
/// it as given.
// @assumes transcription-if
// @assumes mapping-m-cycle-at-e
// @discharges if-sees-rises-by-t-cycle
// @discharges if-step-agrees
#[kani::proof]
#[kani::unwind(9)]
fn netlist_if_bisimulation() {
    let net = NetIf {
        flags: kani::any(),
        ie: kani::any(),
    };
    let ints = Interrupts {
        ie: kani::any(),
        ifr: kani::any(),
        ..Interrupts::default()
    };
    assert!(step_agrees(
        net,
        ints,
        any_access(),
        kani::any(),
        kani::any(),
        kani::any()
    ));
}
