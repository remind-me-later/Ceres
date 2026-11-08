//! Kani proofs that ceres's background pipeline ends mode 3 where the
//! netlist's does, on the DMG, while the window and the objects are idle
//! (`cargo kani -p ceres-core --harness netlist_background`).
//!
//! `NetPipeline` (`ceres_netlist::render_edge`) is render's background
//! pipeline and sprite fetch edge by edge; ceres-netlist's
//! `tests/render_edge.rs` checks it against the delta-cycle transcription,
//! `NetRender`, at every edge where the window is idle. Ceres's mode 3 loop runs a dot's
//! work in its first half dot, the netlist's pipeline takes the dot's two
//! edges; after both, the netlist's pipeline is a function of ceres's
//! `Background` (`check::pipeline_for`): the fetcher's count, the first
//! fetch's flip-flops and the fine scroll follow from the FIFO's length,
//! the pixel counter (6 ahead) and the discard from the position. SCX does
//! not enter it: ceres's lead-in compares SCX's low bits with its position
//! two dots before the netlist's fine scroll compares them with its count,
//! and spends the two dots after a match at positions -7 and -6, which the
//! netlist spends discarding.
//!
//! The CPU can write SCX between the two compares: ceres's mixer takes the
//! write a dot before the netlist's latch (`mapping-scx-write`), so each of
//! the netlist's compares uses the SCX ceres's used. The relation keeps the
//! netlist's two pending compares and that they come out as ceres's did; a
//! write that makes the lead-in miss every position sends it back to -16
//! (`rerun`), with the netlist's first fetch long done.
//!
//! `netlist_background_start` proves ceres's first dot of the loop
//! (`Mode3Start`, `h = 163`) from the netlist's pipeline as it is at the end
//! of the dot before (`check::before_start`, `mapping-mode3-start`), with
//! any SCX. `netlist_background_dot` proves a dot from every related pair,
//! with any SCX in the mixer and any write the CPU's bus can carry
//! (`check::writes`): both stay related and mode 3 goes on, until ceres has
//! drawn the line (its position at 160); then ceres's `HBlankStart` runs in
//! the next dot's first half dot, and the netlist's mode 3 stops at the
//! dot's second edge, a half dot later (`mode3-end-background`, the part of
//! `mapping-mode3-end` it proves).
//!
//! Ceres's side is its mode 3 loop (`mode3::mode3`), run on the display's
//! state alone (`check::Mode3Loop`): the reads, the object's row and the
//! pixels' mixing do nothing there, and the window's hooks are as an idle
//! window leaves them. `check::ceres_dot` wakes it as `step_state_machine`
//! does, and takes `mode3_done`'s sleep into `HBlankStart`.
//!
//! What it does not cover yet: the window and the objects (both off); line
//! 0 after the LCD is turned on, whose loop starts elsewhere; how a write
//! reaches ceres's mixer (`cpu_bus::write`'s hold of SCX's low bits,
//! measured); that an idle window's hooks do nothing; and the dispatch
//! around the loop (`ceres_dot`'s, copied from `step_state_machine` and
//! `mode3_done`, as `timing::check` copies the timing's).

use {
    super::{
        Background,
        check::{Mode3Loop, start_agrees, step_agrees},
    },
    crate::ppu::display::{Display, fetcher::FetcherStep, fifo::Fifo, state::State},
    ceres_netlist::render_edge::{EdgeInputs, NetPipeline},
};

fn any_inputs() -> EdgeInputs {
    EdgeInputs {
        clk: kani::any(),
        ppu_reset: kani::any(),
        hard_reset: kani::any(),
        lcdc: kani::any(),
        line_start: kani::any(),
        stop_scan: kani::any(),
        ceno: kani::any(),
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
        xs: kani::any(),
        fetched: kani::any(),
        sobu: kani::any(),
        suda: kani::any(),
        sprite_step: kani::any(),
        busy: kani::any(),
        tobu: kani::any(),
        vonu: kani::any(),
        seba: kani::any(),
        tyfo: kani::any(),
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

/// Any of mode 3's sleeps, or the one after it, a few dots long.
fn any_sleep() -> (State, i32) {
    let state = match kani::any::<u8>() % 8 {
        0 => State::Mode3Pixel,
        1 => State::Mode3ObjectWait,
        2 => State::Mode3ObjectFetch,
        3 => State::Mode3ObjectAttributes,
        4 => State::Mode3ObjectLow,
        5 => State::Mode3ObjectHigh,
        6 => State::HBlankStart,
        _ => State::Mode3,
    };
    let wait: i32 = kani::any();
    kani::assume((0..=3).contains(&wait));
    (state, wait)
}

/// Ceres's loop with any background, sleep and mixer's SCX, the window
/// idle and nothing else on the line.
fn any_loop() -> Mode3Loop {
    let mut d = Display::default();
    d.bg = any_background();
    (d.state, d.wait) = any_sleep();
    Mode3Loop {
        d,
        mixer_scx: kani::any(),
        objects_enabled: kani::any(),
    }
}

/// A dot from every related pair.
// @assumes transcription-render-edge
// @assumes mapping-mode3-start
// @assumes mapping-scx-write
// @discharges mode3-end-background
#[kani::proof]
#[kani::unwind(12)]
fn netlist_background_dot() {
    let mut l = any_loop();
    let mut net = any_pipeline();
    let last_scx = l.mixer_scx;
    let bg = &l.d.bg;
    let p = super::check::signed(bg.position);
    let related = super::check::related(&l, &net);
    kani::cover!(related & (p == -15), "unmatched lead-in");
    kani::cover!(related & (p == -16), "lead-in restarted");
    kani::cover!(
        related & (p <= -9) & (net.scx & 7 != last_scx & 7),
        "SCX written in the lead-in"
    );
    kani::cover!(related & (p == -6), "netlist still discards");
    kani::cover!(related & (p == 160), "line drawn");
    kani::cover!(related & (bg.fifo.len() == 1) & (p > 0), "push next dot");
    assert!(step_agrees(
        &mut l,
        &mut net,
        kani::any(),
        any_inputs(),
        any_inputs()
    ));
}

/// Ceres's first dot of the loop, from the netlist's state before it.
// @assumes transcription-render-edge
// @assumes mapping-mode3-start
// @assumes mapping-scx-write
#[kani::proof]
#[kani::unwind(12)]
fn netlist_background_start() {
    let mut l = any_loop();
    let mut net = any_pipeline();
    assert!(start_agrees(
        &mut l,
        &mut net,
        kani::any(),
        any_inputs(),
        any_inputs()
    ));
}
