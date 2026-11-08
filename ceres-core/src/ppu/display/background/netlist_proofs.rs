//! Kani proofs that ceres's background pipeline ends mode 3 where the
//! netlist's does, on the DMG, while the window and the objects are idle
//! (`cargo kani -p ceres-core --harness netlist_background`).
//!
//! `NetPipeline` (`ceres_netlist::render_edge`) is render's background
//! pipeline edge by edge; ceres-netlist's `tests/render_edge.rs` checks it
//! against the delta-cycle transcription, `NetRender`, at every edge where
//! the window and the sprites are idle. Ceres's mode 3 loop runs a dot's
//! work in its first half dot, the netlist's pipeline takes the dot's two
//! edges; after both, the netlist's pipeline is a function of ceres's
//! `Background` (`check::pipeline_for`): the fetcher's count, the first
//! fetch's flip-flops and the fine scroll follow from the FIFO's length,
//! the pixel counter (6 ahead) and the discard from the position. SCX does
//! not enter it: ceres's lead-in matches SCX's low bits two dots before the
//! netlist's fine scroll does, and spends those dots at positions -7 and
//! -6, which the netlist spends discarding.
//!
//! `netlist_background_start` proves ceres's first dot of the loop
//! (`Mode3Start`, `h = 163`) from the netlist's pipeline as it is at the end
//! of the dot before (`check::before_start`, `mapping-mode3-start`), with
//! any SCX. `netlist_background_dot` proves a dot from every related pair,
//! with any SCX: both stay related and mode 3 goes on, until ceres has drawn
//! the line (its position at 160); then ceres's `HBlankStart` runs in the
//! next dot's first half dot, and the netlist's mode 3 stops at the dot's
//! second edge, a half dot later (`mode3-end-background`, the part of
//! `mapping-mode3-end` it proves).
//!
//! What it does not cover yet: the window and the objects; SCX written
//! during mode 3 (the CPU does not address SCX); line 0 after the LCD is
//! turned on, whose loop starts elsewhere; and the order of the loop's
//! steps, which `check::ceres_dot` follows (`Step::OutputPixel`: the FIFO's
//! pixel, then the fetcher) as `timing::check` follows the timing's, with
//! the line drawn ending in `HBlankStart` a dot later (`mode3_done`).

use {
    super::{
        Background,
        check::{start_agrees, step_agrees},
    },
    crate::ppu::display::{fetcher::FetcherStep, fifo::Fifo},
    ceres_netlist::render_edge::{EdgeInputs, NetPipeline},
};

fn any_inputs() -> EdgeInputs {
    EdgeInputs {
        clk: kani::any(),
        ppu_reset: kani::any(),
        hard_reset: kani::any(),
        line_start: kani::any(),
        stop_scan: kani::any(),
        cpu_wr_sync: kani::any(),
        addr: kani::any(),
        data: kani::any(),
    }
}

fn any_pipeline() -> NetPipeline {
    NetPipeline {
        mode3: kani::any(),
        voga: kani::any(),
        px: kani::any(),
        fetch_count: kani::any(),
        lyzu: kani::any(),
        nyka: kani::any(),
        pory: kani::any(),
        pygo: kani::any(),
        started: kani::any(),
        fine: kani::any(),
        puxa: kani::any(),
        nyze: kani::any(),
        discard: kani::any(),
        ryfa: kani::any(),
        rene: kani::any(),
        scx: kani::any(),
        inputs: any_inputs(),
    }
}

fn any_step() -> FetcherStep {
    match kani::any::<u8>() % 7 {
        0 => FetcherStep::GetTileT1,
        1 => FetcherStep::GetTileT2,
        2 => FetcherStep::DataLowT1,
        3 => FetcherStep::DataLowT2,
        4 => FetcherStep::DataHighT1,
        5 => FetcherStep::DataHighT2,
        _ => FetcherStep::Push,
    }
}

/// A background with any position, step and FIFO length (its pixels any).
fn any_background() -> Background {
    let mut fifo = Fifo::default();
    fifo.push_bg_row(kani::any(), kani::any(), 0, false, false);
    let popped: u8 = kani::any();
    kani::assume(popped <= 8);
    let mut k = 0;
    while k < popped {
        fifo.pop();
        k += 1;
    }
    let mut bg = Background {
        position: kani::any(),
        fractional: kani::any(),
        insert_blank: kani::any(),
        fifo,
        ..Background::default()
    };
    bg.fetcher.step = any_step();
    bg.fetcher.data = [kani::any(), kani::any()];
    bg
}

/// A dot from every related pair.
// @assumes transcription-render-edge
// @assumes mapping-mode3-start
// @discharges mode3-end-background
#[kani::proof]
#[kani::unwind(10)]
fn netlist_background_dot() {
    let mut bg = any_background();
    let mut net = any_pipeline();
    let scx: u8 = kani::any();
    let p = super::check::signed(bg.position);
    kani::cover!(
        super::check::related(&bg, &net, scx) && p == -15,
        "unmatched lead-in"
    );
    kani::cover!(
        super::check::related(&bg, &net, scx) && p == -6,
        "netlist still discards"
    );
    kani::cover!(
        super::check::related(&bg, &net, scx) && p == 160,
        "line drawn"
    );
    kani::cover!(
        super::check::related(&bg, &net, scx) && bg.fifo.len() == 1 && p > 0,
        "push next dot"
    );
    assert!(step_agrees(
        &mut bg,
        &mut net,
        scx,
        any_inputs(),
        any_inputs()
    ));
}

/// Ceres's first dot of the loop, from the netlist's state before it.
// @assumes transcription-render-edge
// @assumes mapping-mode3-start
#[kani::proof]
#[kani::unwind(10)]
fn netlist_background_start() {
    let mut bg = any_background();
    let mut net = any_pipeline();
    let scx: u8 = kani::any();
    assert!(start_agrees(
        &mut bg,
        &mut net,
        scx,
        any_inputs(),
        any_inputs()
    ));
}
