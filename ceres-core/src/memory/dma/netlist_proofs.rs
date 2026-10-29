//! Kani proofs that ceres's OAM DMA is the netlist's, on the DMG
//! (`cargo kani -p ceres-core --harness netlist_dma`).
//!
//! `NetDma` (`memory/netlist.rs`) transcribes `hdl/rtl/dma.vhd` phase by
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

use super::{Dma, DmaPhase, DmaStep};
use crate::memory::netlist::{NetDma, Phase};

#[derive(Clone, Copy)]
enum Access {
    None,
    Read,
    Write(u8),
}

fn any_access() -> Access {
    match kani::any::<u8>() % 3 {
        0 => Access::None,
        1 => Access::Read,
        _ => Access::Write(kani::any()),
    }
}

/// Ceres's DMA, and the byte it copied in the M-cycle before.
struct Ceres {
    dma: Dma,
    last_copy: Option<(u16, u8)>,
}

fn any_ceres() -> Ceres {
    let phase = match kani::any::<u8>() % 4 {
        0 => DmaPhase::Idle,
        1 => DmaPhase::StartUp,
        2 => DmaPhase::Copy { next: kani::any() },
        _ => DmaPhase::Last,
    };
    Ceres {
        dma: Dma {
            cycles: kani::any(),
            cycles_modulo: kani::any(),
            phase,
            current_src: kani::any(),
            reg: kani::any(),
            restarting: kani::any(),
            ran_when_written: kani::any(),
        },
        last_copy: kani::any::<bool>().then(|| (kani::any(), kani::any())),
    }
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

/// The netlist's M-cycle, from E to D: what a read of FF46 reads, and the
/// OAM byte the DMA writes (as the write ends, in G).
fn net_m_cycle(net: &mut NetDma, access: Access) -> (Option<u8>, Option<(u16, u8)>) {
    let write = match access {
        Access::Write(val) => Some(val),
        Access::None | Access::Read => None,
    };
    for phase in [Phase::E, Phase::F, Phase::G] {
        net.phase(phase, write);
    }
    let oam = net.oam_write();
    net.phase(Phase::H, None);
    let read = matches!(access, Access::Read).then_some(net.source);
    for phase in [Phase::A, Phase::B, Phase::C, Phase::D] {
        net.phase(phase, None);
    }
    (read, oam)
}

/// Ceres's M-cycle: the access, then a run of four T-cycles, as
/// `Gb::run_dma` makes it. Returns what a read reads, the byte the netlist
/// writes in this M-cycle (the one ceres copied in the last), and whether
/// the transfer runs (`Dma::runs`) after the access.
fn ceres_m_cycle(ceres: &mut Ceres, access: Access) -> (Option<u8>, Option<(u16, u8)>, bool) {
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
    let runs = ceres.dma.runs();
    let mut copy = None;
    if ceres.dma.is_active() {
        ceres.dma.set_cycles(4);
        ceres.dma.begin_run();
        while let Some(step) = ceres.dma.due_step() {
            assert!(copy.is_none(), "two copies in an M-cycle");
            match step {
                DmaStep::Copy { src, dest } => copy = Some((src, dest)),
                DmaStep::Pause { rewrite } => {
                    copy = rewrite;
                    ceres.dma.end_pause();
                    break;
                }
            }
        }
    }
    ceres.last_copy = copy;
    (read, written, runs)
}

/// The states match, at the start of the netlist's E.
fn related(net: &NetDma, ceres: &Ceres) -> bool {
    let dma = &ceres.dma;
    let base = u16::from(dma.reg) << 8;
    let regs = dma.reg == net.source && dma.cycles == 0;
    let state = match dma.phase {
        // Nothing asked for, started or running.
        DmaPhase::Idle => {
            !net.requested
                && !net.start_1
                && !net.start_2
                && !net.enabled
                && !net.run
                && ceres.last_copy.is_none()
        }
        // A run always passes the start-up delay.
        DmaPhase::StartUp => false,
        // The M-cycle after a restart: the start is on its way, and the
        // netlist, still running, writes byte 0, which ceres copied.
        DmaPhase::Copy { next: 0 } if dma.restarting => {
            net.requested
                && net.start_1
                && !net.start_2
                && net.enabled
                && net.run
                && ceres.last_copy == Some((base, 0))
                && dma.current_src == base
                && dma.cycles_modulo == 2
        }
        // The M-cycle after the write to FF46: the start is on its way.
        DmaPhase::Copy { next: 0 } => {
            net.requested
                && net.start_1
                && !net.start_2
                && !net.enabled
                && !net.run
                && ceres.last_copy.is_none()
                && dma.current_src == base
                && dma.cycles_modulo == 2
        }
        // Byte `n` copied; the netlist writes it in the coming E to G.
        DmaPhase::Copy { .. } | DmaPhase::Last => {
            let next = match dma.phase {
                DmaPhase::Copy { next } => next,
                _ => 160,
            };
            let n = next - 1;
            (matches!(dma.phase, DmaPhase::Last) || next < 160)
                && !net.requested
                && !net.start_1
                && net.start_2 == (n == 0)
                && net.enabled
                && net.run
                && !net.done
                && net.count == n
                && ceres.last_copy == Some((base | u16::from(n), n))
                && dma.current_src == base + u16::from(next)
                && dma.cycles_modulo == 2
        }
    };
    regs && state
}

/// From any related pair, an M-cycle with any access to FF46 gives the same
/// read, OAM write and blocking of the CPU's OAM accesses, and leaves the
/// pair related.
// @assumes transcription-dma
// @assumes mapping-m-cycle-at-e
// @assumes mapping-dma-step-at-a
// @assumes dma-one-run-per-m-cycle
// @discharges dma-runs-is-netlist-run
#[kani::proof]
#[kani::unwind(5)]
fn netlist_dma_bisimulation() {
    let mut net = any_net_dma();
    let mut ceres = any_ceres();
    kani::assume(related(&net, &ceres));
    let access = any_access();
    let write = matches!(access, Access::Write(_));
    let starting = ceres.dma.phase == (DmaPhase::Copy { next: 0 });
    let restart = write && net.run && !starting;
    let write_after_write = write && starting && !net.run;
    let write_after_restart = write && starting && net.run;
    // What the PPU sees of the transfer after the access, before ceres's
    // step at A: the netlist's run changes only there.
    // @fact oam-scan-during-dma-restart
    let run_before = net.run;
    let (net_read, net_write) = net_m_cycle(&mut net, access);
    let (ceres_read, ceres_write, runs_after_access) = ceres_m_cycle(&mut ceres, access);
    assert!(
        runs_after_access == run_before,
        "the PPU sees another run after the access"
    );
    assert!(net_read == ceres_read, "the read differs");
    assert!(net_write == ceres_write, "the OAM write differs");
    // @fact dma-oam-write-after-start
    assert!(
        ceres.dma.blocks_oam() == net.run,
        "the CPU's OAM accesses are blocked otherwise"
    );
    assert!(ceres.dma.runs() == net.run, "the PPU sees another run");
    assert!(related(&net, &ceres), "the states part");
    kani::cover!(matches!(access, Access::Write(_)), "a transfer starts");
    kani::cover!(restart, "a restart");
    kani::cover!(write_after_write, "a write after a write");
    kani::cover!(write_after_restart, "a write after a restart");
    kani::cover!(net_write.is_some_and(|(_, n)| n == 0), "byte 0");
    kani::cover!(net_write.is_some_and(|(_, n)| n == 159), "byte 159");
    kani::cover!(
        ceres.dma.phase == DmaPhase::Idle && net_write.is_some(),
        "the transfer ends"
    );
}
