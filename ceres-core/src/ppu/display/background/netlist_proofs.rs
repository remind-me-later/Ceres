//! Kani proofs that ceres's mode 3 ends where the netlist's does, on the
//! DMG, while the window is idle, with any objects at X 8 or more
//! (`cargo kani -p ceres-core --harness netlist_mode3`).
//!
//! `NetPipeline` (`ceres_netlist::render_edge`) is render's background
//! pipeline and sprite fetch edge by edge; ceres-netlist's
//! `tests/render_edge.rs` checks it against the delta-cycle transcription,
//! `NetRender`, at every edge where the window is idle. Ceres's mode 3 loop
//! runs a dot's work in its first half dot, the netlist's pipeline takes
//! the dot's two edges; after both, the netlist's pipeline is a function of
//! ceres's loop and of a ghost (`check::expected`, `check::Ghost`): the
//! background's fetcher, first fetch and fine scroll follow from the FIFO's
//! length and the position (`check::pipeline_at`), SCX does not enter them
//! (ceres's lead-in compares SCX two dots before the netlist's fine scroll,
//! with the SCX the netlist's latch then holds: `mapping-scx-write`), and
//! an object shifts them: ceres stops for it as its position plus 8 reaches
//! its X, the netlist two dots later, and the netlist goes on two dots after
//! ceres's push. The ghost keeps that history, updated from ceres's dot
//! alone: the pixels the netlist drew past ceres's, whether they stood
//! still, its fetcher's age in its tile, and the dots since ceres's push;
//! the netlist's sprite fetch is locked to ceres's reads, and its store
//! holds, X by X, the objects ceres has left (`check::objects_agree`).
//!
//! `netlist_mode3_start` proves ceres's first dot of the loop (`Mode3Start`,
//! `h = 163`) from the netlist's pipeline as it is at the end of the dot
//! before (`check::before_start`, `mapping-mode3-start`), with any SCX and
//! any objects the scan found (the scan's proofs: the store holds them).
//! The `netlist_mode3_dot_*` harnesses prove a dot from every related pair,
//! one per sleep of ceres's loop (and the fetch's start by the dots since
//! the last push), as one did not fit in 9 GB: with any SCX in the mixer
//! and any write the CPU's bus can carry (`check::writes`), both stay
//! related and mode 3 goes on, until ceres has drawn the line (its position
//! at 160); then ceres's `HBlankStart` runs in the next dot's first half
//! dot, and the netlist's mode 3 stops at the dot's second edge, a half dot
//! later (`mode3-end-background`, the part of `mapping-mode3-end` it
//! proves).
//!
//! Ceres's side is its mode 3 loop (`mode3::mode3`), run on the display's
//! state alone (`check::Mode3Loop`): the reads, the object's row and the
//! pixels' mixing do nothing there, and the window's hooks are as an idle
//! window leaves them. `check::ceres_dot` wakes it as `step_state_machine`
//! does, and takes `mode3_done`'s sleep into `HBlankStart`.
//!
//! What it does not cover yet: the window; objects at X 0 to 7 (they match
//! in the lead-in), an object's mode 0 pulse at X 167, and writes of LCDC's
//! object bit (the aborted fetch); line 0 after the LCD is turned on, whose
//! loop starts elsewhere; how a write reaches ceres's mixer
//! (`cpu_bus::write`'s hold of SCX's low bits, measured); that an idle
//! window's hooks do nothing; and the dispatch around the loop
//! (`ceres_dot`'s, copied from `step_state_machine` and `mode3_done`, as
//! `timing::check` copies the timing's).

use {
    super::{
        Background,
        check::{Ghost, Mode3Loop, start_agrees, step_agrees},
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

/// Ceres's loop with any background, sleep, objects and mixer's SCX, the
/// window idle.
fn any_loop() -> Mode3Loop {
    let mut d = Display::default();
    d.bg = any_background();
    d.wait = kani::any();
    kani::assume((0..=3).contains(&d.wait));
    d.objs.count = kani::any();
    kani::assume(d.objs.count <= 10);
    let mut i = 0;
    while i < 10 {
        d.objs.entries[i].x = kani::any();
        i += 1;
    }
    Mode3Loop {
        d,
        mixer_scx: kani::any(),
        objects_enabled: kani::any(),
    }
}

/// A dot from every related pair whose loop sleeps in `state`, `since_push`
/// dots after ceres's last object push if given.
fn dot_from(state: State, since_push: Option<u8>) {
    let mut l = any_loop();
    l.d.state = state;
    let mut net = any_pipeline();
    let g = any_ghost();
    if let Some(since) = since_push {
        kani::assume(g.since_push == since);
    }
    let last_scx = l.mixer_scx;
    let bg = &l.d.bg;
    let p = super::check::signed(bg.position);
    let related = super::check::related(&l, &net, g);
    kani::cover!(
        related & l.objects_enabled & matches!(l.d.state, State::Mode3ObjectWait),
        "object waits for the tile"
    );
    kani::cover!(related & (g.since_push == 0), "object pushed");
    kani::cover!(
        related & (g.since_push == 2) & (g.age == 7) & (bg.fifo.len() < 7),
        "netlist's fetcher ahead"
    );
    kani::cover!(related & (l.d.objs.count >= 2), "two objects left");
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
        g,
        kani::any(),
        any_inputs(),
        any_inputs()
    ));
}

/// A dot from every related pair where the loop sleeps after a pixel.
// @assumes transcription-render-edge
// @assumes mapping-mode3-start
// @assumes mapping-scx-write
// @discharges mode3-end-background
#[kani::proof]
#[kani::unwind(12)]
fn netlist_mode3_dot_pixel() {
    dot_from(State::Mode3Pixel, None);
}

/// A dot from every related pair where an object's fetch waits for the tile.
// @assumes transcription-render-edge
// @assumes mapping-mode3-start
// @assumes mapping-scx-write
// @discharges mode3-end-background
#[kani::proof]
#[kani::unwind(12)]
fn netlist_mode3_dot_wait() {
    dot_from(State::Mode3ObjectWait, None);
}

/// A dot from every related pair where an object's fetch starts as ceres pushes the last.
// @assumes transcription-render-edge
// @assumes mapping-mode3-start
// @assumes mapping-scx-write
// @discharges mode3-end-background
#[kani::proof]
#[kani::unwind(12)]
fn netlist_mode3_dot_fetch_pushed() {
    dot_from(State::Mode3ObjectFetch, Some(0));
}

/// A dot from every related pair where an object's fetch starts a dot after ceres pushed the last.
// @assumes transcription-render-edge
// @assumes mapping-mode3-start
// @assumes mapping-scx-write
// @discharges mode3-end-background
#[kani::proof]
#[kani::unwind(12)]
fn netlist_mode3_dot_fetch_after_push() {
    dot_from(State::Mode3ObjectFetch, Some(1));
}

/// A dot from every related pair where an object's fetch starts.
// @assumes transcription-render-edge
// @assumes mapping-mode3-start
// @assumes mapping-scx-write
// @discharges mode3-end-background
#[kani::proof]
#[kani::unwind(12)]
fn netlist_mode3_dot_fetch() {
    dot_from(State::Mode3ObjectFetch, Some(2));
}

/// A dot from every related pair where an object's attributes are read.
// @assumes transcription-render-edge
// @assumes mapping-mode3-start
// @assumes mapping-scx-write
// @discharges mode3-end-background
#[kani::proof]
#[kani::unwind(12)]
fn netlist_mode3_dot_attributes() {
    dot_from(State::Mode3ObjectAttributes, None);
}

/// A dot from every related pair where an object's low byte is read.
// @assumes transcription-render-edge
// @assumes mapping-mode3-start
// @assumes mapping-scx-write
// @discharges mode3-end-background
#[kani::proof]
#[kani::unwind(12)]
fn netlist_mode3_dot_low() {
    dot_from(State::Mode3ObjectLow, None);
}

/// A dot from every related pair where an object's high byte is read.
// @assumes transcription-render-edge
// @assumes mapping-mode3-start
// @assumes mapping-scx-write
// @discharges mode3-end-background
#[kani::proof]
#[kani::unwind(12)]
fn netlist_mode3_dot_high() {
    dot_from(State::Mode3ObjectHigh, None);
}

/// A dot from every related pair where the line is drawn.
// @assumes transcription-render-edge
// @assumes mapping-mode3-start
// @assumes mapping-scx-write
// @discharges mode3-end-background
#[kani::proof]
#[kani::unwind(12)]
fn netlist_mode3_dot_drawn() {
    dot_from(State::HBlankStart, None);
}

/// Any ghost.
fn any_ghost() -> Ghost {
    let g = Ghost {
        rerun: kani::any(),
        since_push: kani::any(),
        extra: kani::any(),
        frozen: kani::any(),
        age: kani::any(),
    };
    kani::assume((g.since_push <= 2) & (g.extra <= 2) & (g.age <= 7));
    g
}

/// Ceres's first dot of the loop, from the netlist's state before it.
// @assumes transcription-render-edge
// @assumes mapping-mode3-start
// @assumes mapping-scx-write
#[kani::proof]
#[kani::unwind(12)]
fn netlist_mode3_start() {
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
