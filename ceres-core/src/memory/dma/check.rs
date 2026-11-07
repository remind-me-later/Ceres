//! The OAM DMA's bisimulation with the netlist, as one function that Kani
//! proves for every argument and Aeneas takes to Lean (`lean/`), as
//! `timing::check` does for the timer.
//!
//! `NetDma` (`ceres_netlist::dma`) transcribes `hdl/rtl/dma.vhd` phase by
//! phase. Ceres's M-cycle boundary is the start of the netlist's E, as for
//! IF: the CPU writes or reads FF46 there, and the DMA then runs the
//! M-cycle's four T-cycles, in one run (as when the CPU accesses memory in
//! every M-cycle). Its steps come in the third T-cycle, at the netlist's A
//! edge, so the byte ceres copies in an M-cycle is the one the netlist
//! writes in E to G of the next: ceres's side carries its last copy
//! (`CeresDma::last_copy`, which ceres itself does not keep) and compares it
//! with the netlist's write.
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

use super::{Dma, DmaPhase, DmaStep};
use ceres_netlist::{Phase, dma::NetDma};

/// A CPU access to FF46 in the M-cycle.
#[derive(Clone, Copy)]
pub(crate) enum Access {
    None,
    Read,
    Write(u8),
}

/// Ceres's DMA, and the byte it copied in the M-cycle before.
pub(crate) struct CeresDma {
    dma: Dma,
    last_copy: Option<(u16, u8)>,
}

impl CeresDma {
    pub(crate) const fn new(dma: Dma, last_copy: Option<(u16, u8)>) -> Self {
        Self { dma, last_copy }
    }
}

/// The netlist's M-cycle, from E to D: what a read of FF46 reads, the OAM
/// byte the DMA writes (as the write ends, in G), and whether the transfer
/// ran before the M-cycle's A edge and runs after it (`run`, which the PPU
/// sees, and which changes only at A).
pub(crate) const fn net_m_cycle(
    net: &mut NetDma,
    access: Access,
) -> (Option<u8>, Option<(u16, u8)>, bool, bool) {
    let write = match access {
        Access::Write(val) => Some(val),
        Access::None | Access::Read => None,
    };
    let run_before = net.run;
    net.phase(Phase::E, write);
    net.phase(Phase::F, write);
    net.phase(Phase::G, write);
    let oam = net.oam_write();
    net.phase(Phase::H, None);
    let read = match access {
        Access::Read => Some(net.source),
        Access::None | Access::Write(_) => None,
    };
    net.phase(Phase::A, None);
    net.phase(Phase::B, None);
    net.phase(Phase::C, None);
    net.phase(Phase::D, None);
    (read, oam, run_before, net.run)
}

/// What ceres's M-cycle gives: what a read reads, the byte the netlist
/// writes in this M-cycle (the one ceres copied in the last), whether the
/// transfer runs (`Dma::runs`) after the access and after the M-cycle,
/// whether it blocks the CPU's OAM accesses after it, and whether a second
/// step was due (which `step_agrees` rules out).
#[expect(clippy::struct_excessive_bools, reason = "what the M-cycle shows")]
struct CeresOut {
    read: Option<u8>,
    written: Option<(u16, u8)>,
    runs_after_access: bool,
    runs: bool,
    blocks_oam: bool,
    twice: bool,
}

/// Ceres's M-cycle: the access, then a run of four T-cycles, as
/// `Gb::run_dma` makes it. The run takes a step while four T-cycles are
/// left; from the states `related` matches that is one step, so the steps
/// are written out to the second, which may only find none due.
fn ceres_m_cycle(ceres: &mut CeresDma, access: Access) -> CeresOut {
    let read = match access {
        Access::None => None,
        Access::Read => Some(ceres.dma.read()),
        Access::Write(val) => {
            // The byte copied last, read again from the new source.
            ceres.last_copy = ceres.dma.write(val);
            None
        }
    };
    let written = ceres.last_copy;
    let runs_after_access = ceres.dma.runs();
    let mut copy = None;
    let mut twice = false;
    if ceres.dma.is_active() {
        ceres.dma.set_cycles(4);
        ceres.dma.begin_run();
        match ceres.dma.due_step() {
            None => {}
            Some(DmaStep::Copy { src, dest }) => {
                copy = Some((src, dest));
                twice = ceres.dma.due_step().is_some();
            }
            Some(DmaStep::Pause { rewrite }) => {
                copy = rewrite;
                ceres.dma.end_pause();
            }
        }
    }
    ceres.last_copy = copy;
    CeresOut {
        read,
        written,
        runs_after_access,
        runs: ceres.dma.runs(),
        blocks_oam: ceres.dma.blocks_oam(),
        twice,
    }
}

/// Two OAM writes alike (Aeneas does not model `Option`'s equality).
pub(crate) const fn same_write(a: Option<(u16, u8)>, b: Option<(u16, u8)>) -> bool {
    match (a, b) {
        (Some((a_src, a_dest)), Some((b_src, b_dest))) => (a_src == b_src) & (a_dest == b_dest),
        (None, None) => true,
        _ => false,
    }
}

/// Two reads alike.
const fn same_read(a: Option<u8>, b: Option<u8>) -> bool {
    match (a, b) {
        (Some(x), Some(y)) => x == y,
        (None, None) => true,
        _ => false,
    }
}

/// The states match, at the start of the netlist's E.
pub(crate) fn related(net: NetDma, ceres: &CeresDma) -> bool {
    let dma = &ceres.dma;
    let base = u16::from(dma.reg) << 8;
    let regs = (dma.reg == net.source) & (dma.cycles == 0);
    let state = match dma.phase {
        // Nothing asked for, started or running.
        DmaPhase::Idle => {
            !net.requested
                & !net.start_1
                & !net.start_2
                & !net.enabled
                & !net.run
                & same_write(ceres.last_copy, None)
        }
        // A run always passes the start-up delay.
        DmaPhase::StartUp => false,
        DmaPhase::Copy { next: 0 } => {
            if dma.restarting {
                // The M-cycle after a restart: the start is on its way, and
                // the netlist, still running, writes byte 0, which ceres
                // copied.
                net.requested
                    & net.start_1
                    & !net.start_2
                    & net.enabled
                    & net.run
                    & same_write(ceres.last_copy, Some((base, 0)))
                    & (dma.current_src == base)
                    & (dma.cycles_modulo == 2)
            } else {
                // The M-cycle after the write to FF46: the start is on its
                // way.
                net.requested
                    & net.start_1
                    & !net.start_2
                    & !net.enabled
                    & !net.run
                    & same_write(ceres.last_copy, None)
                    & (dma.current_src == base)
                    & (dma.cycles_modulo == 2)
            }
        }
        // Byte `n` copied; the netlist writes it in the coming E to G.
        DmaPhase::Copy { next } => copying(net, ceres, base, next, next < 160),
        DmaPhase::Last => copying(net, ceres, base, 160, true),
    };
    regs & state
}

/// The states match while byte `next_byte - 1` is the last copied.
fn copying(net: NetDma, ceres: &CeresDma, base: u16, next_byte: u8, in_range: bool) -> bool {
    let dma = &ceres.dma;
    let n = next_byte - 1;
    in_range
        & !net.requested
        & !net.start_1
        & (net.start_2 == (n == 0))
        & net.enabled
        & net.run
        & !net.done
        & (net.count == n)
        & same_write(ceres.last_copy, Some((base | u16::from(n), n)))
        & (dma.current_src == base + u16::from(next_byte))
        & (dma.cycles_modulo == 2)
}

/// The step: from a related pair, an M-cycle with `access` gives the same
/// read and the same OAM write on both sides, ceres's transfer runs as the
/// netlist's after the access and after the M-cycle, blocks the CPU's OAM
/// accesses when the netlist's runs, and the pair is related again. True
/// for every argument (Kani's `netlist_dma_bisimulation`).
pub(crate) fn step_agrees(mut net: NetDma, mut ceres: CeresDma, access: Access) -> bool {
    if !related(net, &ceres) {
        return true;
    }
    #[cfg(kani)]
    let starting = ceres.dma.phase == (DmaPhase::Copy { next: 0 });
    let (net_read, net_write, run_before, run_after) = net_m_cycle(&mut net, access);
    let out = ceres_m_cycle(&mut ceres, access);
    #[cfg(kani)]
    {
        let write = matches!(access, Access::Write(_));
        kani::cover!(write, "a transfer starts");
        kani::cover!(write && run_before && !starting, "a restart");
        kani::cover!(write && starting && !run_before, "a write after a write");
        kani::cover!(write && starting && run_before, "a write after a restart");
        kani::cover!(net_write.is_some_and(|(_, n)| n == 0), "byte 0");
        kani::cover!(net_write.is_some_and(|(_, n)| n == 159), "byte 159");
        kani::cover!(
            ceres.dma.phase == DmaPhase::Idle && net_write.is_some(),
            "the transfer ends"
        );
    }
    // What the PPU sees of the transfer after the access, before ceres's
    // step at A: the netlist's run changes only there.
    // @fact oam-scan-during-dma-restart
    same_read(net_read, out.read)
        & same_write(net_write, out.written)
        & (out.runs_after_access == run_before)
        & (out.runs == run_after)
        // @fact dma-oam-write-after-start
        & (out.blocks_oam == run_after)
        & !out.twice
        & related(net, &ceres)
}
