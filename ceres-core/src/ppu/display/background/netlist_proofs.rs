//! Kani proofs that ceres's mode 3 ends where the netlist's does, on the
//! DMG, while the window is idle, with any objects at X 8 or more, and that
//! an armed window does not start before its WX
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
//! the last push), as one did not fit in 9 GB. They build the netlist's
//! side, `check::expected` of what it carries over, rather than assume it
//! (`netlist_mode3_expected_carries`: every related pair's netlist is that),
//! and its inputs quiet, so that Kani folds what the relation pins. With any
//! SCX in the mixer
//! and any write the CPU's bus can carry (`check::writes`), both stay
//! related and mode 3 goes on, until ceres has drawn the line (its position
//! at 160); then ceres's `HBlankStart` runs in the next dot's first half
//! dot, and the netlist's mode 3 stops at the dot's second edge, a half dot
//! later (`mode3-end-background`, the part of `mapping-mode3-end` it
//! proves).
//!
//! The `netlist_mode3_armed_*` harnesses prove the same dots with the window
//! armed (`mode3-window-armed`): LCDC's bit 5 set, WY matched in the frame on
//! both sides (ceres's `wy_triggered`, the netlist's REJO), WX held between
//! 7 and 165, and ceres's window check short of it. Both stay related, the
//! netlist's window waiting (no WX match, not in it), until ceres's check
//! reaches WX: the window starts in the next dot.
//!
//! The `netlist_mode3_window_*` harnesses prove the window's start and the
//! window after it, with the objects off (`mode3-window-start`): from the
//! armed window with ceres's check at WX, through the nine dots in which
//! ceres fetches the window's first tile and the netlist holds its pixels
//! and restarts its fetcher and fine scroll (`check::window_starting`), and
//! inside the window to the line's end, both stay related, and the
//! netlist's mode 3 stops a dot after ceres has drawn the line.
//!
//! Ceres's side is its mode 3 loop (`mode3::mode3`), run on the display's
//! state alone (`check::Mode3Loop`): the reads, the object's row and the
//! pixels' mixing do nothing there, the window's check runs
//! (`Display::mode3_window`), and the loop itself steps the window's column
//! and ends its first tile's fetch; its pixel insertion does nothing (WX is
//! held, and the insertion needs a second match). `check::ceres_dot` wakes it as `step_state_machine`
//! does, and takes `mode3_done`'s sleep into `HBlankStart`.
//!
//! What it does not cover yet: the window's start with the objects on, the
//! pixels the window draws, how it is armed at a line's start (WY's match,
//! and a line's start with the window armed), WX outside 7 to 165, and
//! writes of WX and LCDC's window bit (the fetcher's read clears ceres's
//! start when the bit is clear, `read_tile`); objects at X 0 to 7 (they match
//! in the lead-in), an object's mode 0 pulse at X 167, and writes of LCDC's
//! object bit (the aborted fetch); line 0 after the LCD is turned on, whose
//! loop starts elsewhere; how a write reaches ceres's mixer
//! (`cpu_bus::write`'s hold of SCX's low bits, measured); that the window's
//! other hooks do nothing before it starts; and the dispatch around the loop
//! (`ceres_dot`'s, copied from `step_state_machine` and `mode3_done`, as
//! `timing::check` copies the timing's).

use {
    super::{
        Background,
        check::{
            Ghost, INSIDE, Mode3Loop, expected, related_but_pipeline, start_agrees, step_agrees,
        },
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
        ly: kani::any(),
        hclk: kani::any(),
        mode1: kani::any(),
        line_start: kani::any(),
        stop_scan: kani::any(),
        ceno: kani::any(),
        cpu_wr_sync: kani::any(),
        addr: kani::any(),
        data: kani::any(),
    }
}

/// LCDC's object and window enables.
const LCDC_OBJ_EN: u8 = 2;
const LCDC_WIN_EN: u8 = 0x20;

/// Inputs as `check::quiet` takes them in mode 3 for `l`, with the clock at
/// `clk`: built rather than assumed, so that Kani folds what they pin. LCDC's
/// other bits, LY, hclk, VBlank (with the window off) and the CPU's bus are
/// free.
fn quiet_inputs(l: &Mode3Loop, clk: bool) -> EdgeInputs {
    let other: u8 = kani::any();
    let mut lcdc = other & !(LCDC_OBJ_EN | LCDC_WIN_EN);
    if l.objects_enabled {
        lcdc |= LCDC_OBJ_EN;
    }
    if l.window_enabled {
        lcdc |= LCDC_WIN_EN;
    }
    EdgeInputs {
        clk,
        ppu_reset: false,
        hard_reset: false,
        lcdc,
        ly: kani::any(),
        hclk: kani::any(),
        mode1: !l.window_enabled & kani::any::<bool>(),
        line_start: false,
        stop_scan: false,
        ceno: false,
        cpu_wr_sync: kani::any(),
        addr: kani::any(),
        data: kani::any(),
    }
}

/// What `check::expected` carries over from the netlist, for `l`: the
/// latches, the sprite store, and the inputs after a dot (the clock low);
/// the window's flip-flops as `check::window_agrees` has them (WY's match
/// and the window's line free where it allows).
fn any_carried(l: &Mode3Loop) -> NetPipeline {
    let armed = l.window_enabled;
    NetPipeline {
        wx: if armed { l.wx } else { kani::any() },
        sary: armed & kani::any::<bool>(),
        wy_latch: armed,
        pyco: false,
        nunu: false,
        nopa: false,
        in_window: false,
        win_x: 0,
        rydy: false,
        sovy: false,
        inputs: quiet_inputs(l, false),
        ..any_pipeline()
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
        wy: kani::any(),
        wx: kani::any(),
        sary: kani::any(),
        wy_latch: kani::any(),
        pyco: kani::any(),
        nunu: kani::any(),
        nopa: kani::any(),
        in_window: kani::any(),
        win_x: kani::any(),
        win_y: kani::any(),
        rydy: kani::any(),
        sovy: kani::any(),
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
    let count: u8 = kani::any();
    kani::assume(count <= 10);
    d.objs.count = usize::from(count);
    let mut i = 0;
    while i < 10 {
        d.objs.entries[i].x = kani::any();
        i += 1;
    }
    Mode3Loop {
        d,
        mixer_scx: kani::any(),
        objects_enabled: kani::any(),
        window_enabled: kani::any(),
        wx: kani::any(),
    }
}

/// A dot from every related pair whose loop sleeps in `state`, `since_push`
/// dots after ceres's last object push if given, with the window off, or
/// armed and its check short of WX (at it, the window starts).
fn dot_from(state: State, since_push: Option<u8>, armed: bool) {
    let mut l = any_loop();
    l.d.state = state;
    // The window's flags as `window_agrees` has them: not started, WY
    // matched if armed.
    l.window_enabled = armed;
    l.d.window.wy_triggered = armed;
    if armed {
        kani::assume(super::check::signed(l.d.bg.position) + 7 < i16::from(l.wx));
    }
    // The sleep's dots left as `between_dots` has them.
    l.d.wait = if matches!(state, State::Mode3ObjectAttributes | State::Mode3ObjectLow) {
        1 + i32::from(kani::any::<bool>())
    } else {
        1
    };
    let mut g = any_ghost();
    if let Some(since) = since_push {
        g.since_push = since;
    }
    // The window not started: the ghost counts no dot of it.
    g.window = 0;
    g.window_fine = 0;
    // Every related pair's netlist is `expected` of what it carries over
    // (`netlist_mode3_expected_carries`).
    let mut net = expected(&l, &any_carried(&l), g);
    let last_scx = l.mixer_scx;
    let bg = &l.d.bg;
    let p = super::check::signed(bg.position);
    let related = related_but_pipeline(&l, &net, g);
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
    kani::cover!(
        related & armed & (p + 8 == i16::from(l.wx)),
        "WX in the next dot"
    );
    if related {
        let rising = quiet_inputs(&l, true);
        let falling = quiet_inputs(&l, false);
        assert!(step_agrees(
            &mut l,
            &mut net,
            g,
            kani::any(),
            rising,
            falling
        ));
    }
}

/// What `expected` gives keeps what it carries over from the netlist: it
/// is its own `expected`. A pair is related if and only if its netlist is
/// `expected` of itself and `related_but_pipeline` holds, so the dot
/// harnesses, which take the netlist as `expected` gives it, cover every
/// related pair.
// @assumes transcription-render-edge
// @discharges mode3-end-background
// @discharges mode3-window-armed
#[kani::proof]
#[kani::unwind(12)]
fn netlist_mode3_expected_carries() {
    let l = any_loop();
    let g = any_ghost();
    let net = expected(&l, &any_pipeline(), g);
    assert!(net == expected(&l, &net, g));
}

/// A dot from every related pair where the loop sleeps after a pixel.
// @assumes transcription-render-edge
// @assumes mapping-mode3-start
// @assumes mapping-scx-write
// @discharges mode3-end-background
#[kani::proof]
#[kani::unwind(12)]
fn netlist_mode3_dot_pixel() {
    dot_from(State::Mode3Pixel, None, false);
}

/// A dot from every related pair where an object's fetch waits for the tile.
// @assumes transcription-render-edge
// @assumes mapping-mode3-start
// @assumes mapping-scx-write
// @discharges mode3-end-background
#[kani::proof]
#[kani::unwind(12)]
fn netlist_mode3_dot_wait() {
    dot_from(State::Mode3ObjectWait, None, false);
}

/// A dot from every related pair where an object's fetch starts as ceres pushes the last.
// @assumes transcription-render-edge
// @assumes mapping-mode3-start
// @assumes mapping-scx-write
// @discharges mode3-end-background
#[kani::proof]
#[kani::unwind(12)]
fn netlist_mode3_dot_fetch_pushed() {
    dot_from(State::Mode3ObjectFetch, Some(0), false);
}

/// A dot from every related pair where an object's fetch starts a dot after ceres pushed the last.
// @assumes transcription-render-edge
// @assumes mapping-mode3-start
// @assumes mapping-scx-write
// @discharges mode3-end-background
#[kani::proof]
#[kani::unwind(12)]
fn netlist_mode3_dot_fetch_after_push() {
    dot_from(State::Mode3ObjectFetch, Some(1), false);
}

/// A dot from every related pair where an object's fetch starts.
// @assumes transcription-render-edge
// @assumes mapping-mode3-start
// @assumes mapping-scx-write
// @discharges mode3-end-background
#[kani::proof]
#[kani::unwind(12)]
fn netlist_mode3_dot_fetch() {
    dot_from(State::Mode3ObjectFetch, Some(2), false);
}

/// A dot from every related pair where an object's attributes are read.
// @assumes transcription-render-edge
// @assumes mapping-mode3-start
// @assumes mapping-scx-write
// @discharges mode3-end-background
#[kani::proof]
#[kani::unwind(12)]
fn netlist_mode3_dot_attributes() {
    dot_from(State::Mode3ObjectAttributes, None, false);
}

/// A dot from every related pair where an object's low byte is read.
// @assumes transcription-render-edge
// @assumes mapping-mode3-start
// @assumes mapping-scx-write
// @discharges mode3-end-background
#[kani::proof]
#[kani::unwind(12)]
fn netlist_mode3_dot_low() {
    dot_from(State::Mode3ObjectLow, None, false);
}

/// A dot from every related pair where an object's high byte is read.
// @assumes transcription-render-edge
// @assumes mapping-mode3-start
// @assumes mapping-scx-write
// @discharges mode3-end-background
#[kani::proof]
#[kani::unwind(12)]
fn netlist_mode3_dot_high() {
    dot_from(State::Mode3ObjectHigh, None, false);
}

/// A dot from every related pair where the line is drawn.
// @assumes transcription-render-edge
// @assumes mapping-mode3-start
// @assumes mapping-scx-write
// @discharges mode3-end-background
#[kani::proof]
#[kani::unwind(12)]
fn netlist_mode3_dot_drawn() {
    dot_from(State::HBlankStart, None, false);
}

/// A dot from every related pair with the window armed where the loop sleeps after a pixel.
// @assumes transcription-render-edge
// @assumes mapping-scx-write
// @discharges mode3-window-armed
#[kani::proof]
#[kani::unwind(12)]
fn netlist_mode3_armed_pixel() {
    dot_from(State::Mode3Pixel, None, true);
}

/// A dot from every related pair with the window armed where an object's fetch waits for the tile.
// @assumes transcription-render-edge
// @assumes mapping-scx-write
// @discharges mode3-window-armed
#[kani::proof]
#[kani::unwind(12)]
fn netlist_mode3_armed_wait() {
    dot_from(State::Mode3ObjectWait, None, true);
}

/// A dot from every related pair with the window armed where an object's fetch starts as ceres pushes the last.
// @assumes transcription-render-edge
// @assumes mapping-scx-write
// @discharges mode3-window-armed
#[kani::proof]
#[kani::unwind(12)]
fn netlist_mode3_armed_fetch_pushed() {
    dot_from(State::Mode3ObjectFetch, Some(0), true);
}

/// A dot from every related pair with the window armed where an object's fetch starts a dot after ceres pushed the last.
// @assumes transcription-render-edge
// @assumes mapping-scx-write
// @discharges mode3-window-armed
#[kani::proof]
#[kani::unwind(12)]
fn netlist_mode3_armed_fetch_after_push() {
    dot_from(State::Mode3ObjectFetch, Some(1), true);
}

/// A dot from every related pair with the window armed where an object's fetch starts.
// @assumes transcription-render-edge
// @assumes mapping-scx-write
// @discharges mode3-window-armed
#[kani::proof]
#[kani::unwind(12)]
fn netlist_mode3_armed_fetch() {
    dot_from(State::Mode3ObjectFetch, Some(2), true);
}

/// A dot from every related pair with the window armed where an object's attributes are read.
// @assumes transcription-render-edge
// @assumes mapping-scx-write
// @discharges mode3-window-armed
#[kani::proof]
#[kani::unwind(12)]
fn netlist_mode3_armed_attributes() {
    dot_from(State::Mode3ObjectAttributes, None, true);
}

/// A dot from every related pair with the window armed where an object's low byte is read.
// @assumes transcription-render-edge
// @assumes mapping-scx-write
// @discharges mode3-window-armed
#[kani::proof]
#[kani::unwind(12)]
fn netlist_mode3_armed_low() {
    dot_from(State::Mode3ObjectLow, None, true);
}

/// A dot from every related pair with the window armed where an object's high byte is read.
// @assumes transcription-render-edge
// @assumes mapping-scx-write
// @discharges mode3-window-armed
#[kani::proof]
#[kani::unwind(12)]
fn netlist_mode3_armed_high() {
    dot_from(State::Mode3ObjectHigh, None, true);
}

/// A dot from every related pair with the window on and the objects off,
/// `window` dots after ceres's window check fired if given (0: armed, with
/// the check at WX; `INSIDE`: ten or more), any of the first nine if not;
/// the loop sleeps after a pixel, or has drawn the line.
fn window_dot(window: Option<u8>) {
    let mut l = any_loop();
    l.d.state = if kani::any() {
        State::Mode3Pixel
    } else {
        State::HBlankStart
    };
    l.d.wait = 1;
    l.window_enabled = true;
    l.objects_enabled = false;
    l.d.window.wy_triggered = true;
    l.d.window.wx_triggered = kani::any();
    l.d.window.being_fetched = kani::any();
    l.d.window.tile_x = kani::any();
    let mut g = any_ghost();
    if let Some(dots) = window {
        g.window = dots;
    } else {
        kani::assume((g.window >= 1) & (g.window < INSIDE));
    }
    let p = super::check::signed(l.d.bg.position);
    if g.window == 0 {
        g.window_fine = 0;
        kani::assume(p + 7 == i16::from(l.wx));
    }
    let mut net = expected(&l, &any_carried(&l), g);
    let related = related_but_pipeline(&l, &net, g);
    let drawn = matches!(l.d.state, State::HBlankStart);
    kani::cover!(related & (g.window == 0), "the check reaches WX");
    kani::cover!(related & (g.window == 1), "the window's first dot");
    kani::cover!(related & (g.window == 6), "the window's row pushed");
    kani::cover!(related & (g.window == 9), "the start's last dot");
    kani::cover!(related & (g.window == 8) & drawn, "line drawn in the start");
    kani::cover!(related & (g.window == INSIDE) & !drawn, "in the window");
    kani::cover!(
        related & (g.window == INSIDE) & (l.d.window.tile_x == 0),
        "the window's column wrapped"
    );
    kani::cover!(
        related & (g.window == INSIDE) & drawn,
        "line drawn in the window"
    );
    if related {
        let rising = quiet_inputs(&l, true);
        let falling = quiet_inputs(&l, false);
        assert!(step_agrees(
            &mut l,
            &mut net,
            g,
            kani::any(),
            rising,
            falling
        ));
    }
}

/// A dot from every related pair with the window armed, the objects off and
/// ceres's check at WX: the window starts.
// @assumes transcription-render-edge
// @assumes mapping-scx-write
// @discharges mode3-window-start
#[kani::proof]
#[kani::unwind(12)]
fn netlist_mode3_window_reached() {
    window_dot(Some(0));
}

/// A dot from every related pair in the nine dots of the window's start.
// @assumes transcription-render-edge
// @assumes mapping-scx-write
// @discharges mode3-window-start
#[kani::proof]
#[kani::unwind(12)]
fn netlist_mode3_window_starting() {
    window_dot(None);
}

/// A dot from every related pair in the window, past its start.
// @assumes transcription-render-edge
// @assumes mapping-scx-write
// @discharges mode3-window-start
#[kani::proof]
#[kani::unwind(12)]
fn netlist_mode3_window_inside() {
    window_dot(Some(INSIDE));
}

/// Any ghost.
fn any_ghost() -> Ghost {
    let g = Ghost {
        rerun: kani::any(),
        since_push: kani::any(),
        extra: kani::any(),
        frozen: kani::any(),
        age: kani::any(),
        window: kani::any(),
        window_fine: kani::any(),
    };
    kani::assume(
        (g.since_push <= 2)
            & (g.extra <= 2)
            & (g.age <= 7)
            & (g.window <= INSIDE)
            & (g.window_fine <= 7),
    );
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
