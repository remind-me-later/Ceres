//! Kani proofs that ceres's OAM DMA is the netlist's, on the DMG
//! (`cargo kani -p ceres-core --harness netlist_dma`).
//!
//! `NetDma` (`ceres_netlist::dma`) transcribes `hdl/rtl/dma.vhd` phase by
//! phase. Ceres's M-cycle boundary is the start of the netlist's E, as for
//! IF (`interrupts/netlist_proofs.rs`): the CPU writes or reads FF46 there,
//! and the DMA then runs the M-cycle's four T-cycles, in one run (as when
//! the CPU accesses memory in every M-cycle). Its steps come in the third
//! T-cycle, at the netlist's A edge, so the byte ceres copies in an M-cycle
//! is the one the netlist writes in E to G of the next: the proof carries
//! ceres's last copy and compares it with the netlist's write.
//!
//! The proof is a bisimulation: from any pair of states that `related`
//! matches, an M-cycle with any access to FF46 gives the same read and the
//! same OAM write on both sides, blocks the CPU's accesses to OAM in the
//! same M-cycles, and leaves them related. A write to FF46 while the DMA
//! runs (a restart) is one of the accesses.

use super::{
    Dma, DmaPhase,
    check::{Access, CeresDma, step_agrees},
};
use ceres_netlist::dma::NetDma;

fn any_access() -> Access {
    match kani::any::<u8>() % 3 {
        0 => Access::None,
        1 => Access::Read,
        _ => Access::Write(kani::any()),
    }
}

fn any_ceres() -> CeresDma {
    let phase = match kani::any::<u8>() % 4 {
        0 => DmaPhase::Idle,
        1 => DmaPhase::StartUp,
        2 => DmaPhase::Copy { next: kani::any() },
        _ => DmaPhase::Last,
    };
    CeresDma::new(
        Dma {
            cycles: kani::any(),
            cycles_modulo: kani::any(),
            phase,
            current_src: kani::any(),
            reg: kani::any(),
            restarting: kani::any(),
            ran_when_written: kani::any(),
        },
        kani::any::<bool>().then(|| (kani::any(), kani::any())),
    )
}

fn any_net_dma() -> NetDma {
    NetDma {
        source: kani::any(),
        requested: kani::any(),
        start_1: kani::any(),
        start_2: kani::any(),
        enabled: kani::any(),
        count: kani::any(),
        done: kani::any(),
        run: kani::any(),
    }
}

/// The step, for every argument: from any related pair, an M-cycle with any
/// access to FF46 gives the same read, OAM write and blocking of the CPU's
/// OAM accesses, and leaves the pair related. Lean (`lean/`) takes it as
/// given.
// @assumes transcription-dma
// @assumes mapping-m-cycle-at-e
// @assumes mapping-dma-step-at-a
// @assumes dma-one-run-per-m-cycle
// @discharges dma-runs-is-netlist-run
// @discharges dma-step-agrees
#[kani::proof]
#[kani::unwind(5)]
fn netlist_dma_bisimulation() {
    assert!(step_agrees(any_net_dma(), any_ceres(), any_access()));
}
