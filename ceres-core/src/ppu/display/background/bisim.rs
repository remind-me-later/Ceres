//! Kani proofs that ceres's mode 3 ends where the netlist's does, on the
//! DMG, while the window is idle, with any objects, and that
//! an armed window does not start before its WX
//! (`cargo kani -p ceres-core --harness netlist_mode3`).
//!
//! `NetPipeline` (`ceres_netlist::render_edge`) is render's background
//! pipeline and sprite fetch edge by edge; ceres-netlist's
//! `tests/render_edge.rs` checks it against the delta-cycle transcription,
//! `NetRender`, at every edge where the window is idle. Ceres's mode 3 loop
//! runs a dot's work in its first half dot, the netlist's pipeline takes
//! the dot's two edges; after both, the netlist's pipeline is a function of
//! ceres's loop and of a ghost (`relation::expected`, `relation::Ghost`): the
//! background's fetcher, first fetch and fine scroll follow from the FIFO's
//! length and the position (`relation::pipeline_at`), SCX does not enter them
//! (ceres's lead-in compares SCX two dots before the netlist's fine scroll,
//! with the SCX the netlist's latch then holds: `mapping-scx-write`), and
//! an object shifts them: ceres stops for it as its position plus 8 reaches
//! its X, the netlist two dots later, and the netlist goes on two dots after
//! ceres's push. The ghost keeps that history, updated from ceres's dot
//! alone: the pixels the netlist drew past ceres's, whether they stood
//! still, its fetcher's age in its tile, and the dots since ceres's push;
//! the netlist's sprite fetch is locked to ceres's reads, and its store
//! holds, X by X, the objects ceres has left (`relation::objects_agree`).
//!
//! `netlist_mode3_start` proves ceres's first dot of the loop (`Mode3Start`,
//! `h = 163`) from the netlist's pipeline as it is at the end of the dot
//! before (`relation::before_start`, `mapping-mode3-start`), with any SCX and
//! any objects the scan found (the scan's proofs: the store holds them).
//! The `netlist_mode3_dot_*` harnesses prove a dot from every related pair,
//! one per sleep of ceres's loop (and the fetch's start by the dots since
//! the last push), as one did not fit in 9 GB. They build the netlist's
//! side, `relation::expected` of what it carries over, rather than assume it
//! (`netlist_mode3_expected_carries`: every related pair's netlist is that),
//! and its inputs quiet, so that Kani folds what the relation pins. With any
//! SCX in the mixer
//! and any write the CPU's bus can carry (`relation::writes`), both stay
//! related and mode 3 goes on, until ceres has drawn the line (its position
//! at 160); then ceres's `HBlankStart` runs in the next dot's first half
//! dot, and the netlist's mode 3 stops at the dot's second edge, a half dot
//! later (`mode3-end-background`, the part of `mapping-mode3-end` it
//! proves).
//!
//! The `netlist_mode3_armed_*` harnesses prove the same dots with the window
//! armed (`mode3-window-armed`): LCDC's bit 5 set, WY matched in the frame on
//! both sides (ceres's `wy_triggered`, the netlist's REJO), WX held between
//! 0 and 165, and ceres's window check short of it. Both stay related, the
//! netlist's window waiting (no WX match, not in it), until ceres's check
//! reaches WX: the window starts in the next dot.
//!
//! The `netlist_mode3_window_*` and `netlist_mode3_inside_*` harnesses
//! prove the window's start and the window after it (`mode3-window-start`):
//! from the armed window with ceres's check at WX, through the nine dots in
//! which
//! ceres fetches the window's first tile and the netlist holds its pixels
//! and restarts its fetcher and fine scroll (`relation::window_starting`), and
//! inside the window to the line's end, both stay related, and the
//! netlist's mode 3 stops a dot after ceres has drawn the line.
//! At WX 0 the check fires in the line's first dot: with SCX's low bits not
//! 0 it waits a dot (`relation::window_delay_ok`), and SCX's discard runs in
//! the window (`relation::in_lead`), but for a lead-in that misses SCX
//! altogether, which the steps leave out (`wx0-lead-in-matches`).
//! Inside the window the objects are fetched as outside it, the ghost
//! keeping their history (`relation::background`), one harness per sleep
//! of an object's fetch; the window's column follows ceres's from its read
//! of a tile's last byte to the netlist's restart for the tile, which an
//! object's fetch holds back.
//! An object fetched just before the start holds the netlist in its first
//! two dots (`relation::start_history_ok`: its push, its fetcher's age);
//! from the start's seventh dot, ceres drawing again, an object stops it as
//! outside the window, the netlist two dots later (the ghost's history
//! again), and an object at WX + 1 stops ceres at the start's position
//! from its first dots, the window's first pixel not drawn
//! (`netlist_mode3_window_starting_wait`).
//!
//! Ceres's side is its mode 3 loop (`mode3::mode3`), run on the display's
//! pace (`relation::Mode3Loop`): the window's check runs (`Pace::mode3_window`),
//! and the loop itself steps the window's column and ends its first tile's
//! fetch; it runs the window's pixel insertion and the fetcher's window
//! glitches too, which a held WX and LCDC's bit never set off. The hooks
//! the loop calls (`mode3::Mode3Ppu`) cannot change the pace: ceres's are
//! `PpuCore`'s, which does not hold it, and get it to read
//! (`mode3-hooks-keep-pace`). What they change and the loop reads back is
//! the tile the fetcher holds, which it pushes onto the FIFO: here the
//! fetcher's read and the object's push leave any tile in it, so the proofs
//! hold for whatever ceres's reads find; the object's reads and the pixels'
//! mixing do nothing. `relation::ceres_dot` wakes the loop as
//! `step_state_machine` does, and takes `mode3_done`'s sleep into
//! `HBlankStart`.
//!
//! What it does not cover yet: the pixels the window draws, how it is armed
//! (ceres's WY match against the netlist's latch: a line that starts armed
//! is covered), WX 166, and writes of WX and LCDC's window bit (the loop
//! stops ceres's window as a tile's fetch starts with the bit clear,
//! `Pace::window_before_fetch`); objects at X 0 to 7 with the window
//! starting in the lead-in, at WX 0 to 6 (`lead-in-objects`: the cosim
//! walks them), an object's mode 0 pulse at X 167, and writes of LCDC's
//! object bit (the aborted fetch); line 0 after the LCD is turned on,
//! whose loop starts elsewhere; how a write
//! reaches ceres's mixer (`cpu_bus::write`'s hold of SCX's low bits,
//! measured); and the dispatch around the loop
//! (`ceres_dot`'s, copied from `step_state_machine` and `mode3_done`, as
//! `timing::relation` copies the timing's).

use {
    super::{
        Background,
        relation::{
            DRAWS_AGAIN, Ghost, INSIDE, Mode3Loop, expected, related_but_pipeline, start_agrees,
            step_agrees,
        },
    },
    crate::ppu::display::{Pace, fetcher::FetcherStep, fifo::Fifo, state::State},
    ceres_netlist::render_edge::{EdgeInputs, NetFetcher, NetPipeline, NetSpriteFetch},
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

/// Inputs as `relation::quiet` takes them in mode 3 for `l`, with the clock at
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

/// What `relation::expected` carries over from the netlist, for `l`: the
/// latches, the sprite store, and the inputs after a dot (the clock low);
/// the window's flip-flops as `relation::window_agrees` has them (WY's match
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
        fetcher: NetFetcher {
            count: kani::any(),
            lyzu: kani::any(),
            nyka: kani::any(),
            pory: kani::any(),
            pygo: kani::any(),
            started: kani::any(),
        },
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
        sprite: NetSpriteFetch {
            sobu: kani::any(),
            suda: kani::any(),
            step: kani::any(),
            busy: kani::any(),
            tobu: kani::any(),
            vonu: kani::any(),
            seba: kani::any(),
            tyfo: kani::any(),
        },
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
    bg.step = any_step();
    bg
}

/// Ceres's loop with any background, sleep, objects, fetched tile and
/// mixer's SCX, the window idle.
fn any_loop() -> Mode3Loop {
    let mut pace = Pace::default();
    pace.bg = any_background();
    pace.wait = kani::any();
    kani::assume((0..=3).contains(&pace.wait));
    let count: u8 = kani::any();
    kani::assume(count <= 10);
    let found: u8 = kani::any();
    kani::assume((count <= found) & (found <= 10));
    pace.objs.count = usize::from(count);
    pace.objs.found = usize::from(found);
    // Objects at X 0 to 7 are fetched in the lead-in: `objects_ok` leaves
    // them out where the window starts there.
    let mut i = 0;
    while i < 10 {
        pace.objs.entries[i].x = kani::any();
        i += 1;
    }
    Mode3Loop {
        pace,
        fetcher: kani::any(),
        mixer_scx: kani::any(),
        objects_enabled: kani::any(),
        window_enabled: kani::any(),
        wx: kani::any(),
    }
}

/// Where the window is in `dot_from`'s pairs.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Window {
    Off,
    /// Armed, ceres's check short of WX (at it, the window starts).
    Armed,
    /// Ten dots or more after ceres's check fired.
    Inside,
}

/// A dot from every related pair whose loop sleeps in `state`, `since_push`
/// dots after ceres's last object push if given, with the window where
/// `window` has it.
fn dot_from(state: State, since_push: Option<u8>, window: Window) {
    let (mut l, mut net, g) = dot_pair(state, since_push, window);
    let related = related_but_pipeline(&l, &net, g);
    // Some pair is related, so the proof is not vacuous; the cases the
    // pairs reach are `netlist_mode3_dot_covers`'s.
    kani::cover!(related, "a related pair");
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

/// The pairs `dot_from` takes, related or not: the loop sleeps in `state`,
/// `since_push` dots after ceres's last object push if given, with the
/// window where `window` has it.
fn dot_pair(
    state: State,
    since_push: Option<u8>,
    window: Window,
) -> (Mode3Loop, NetPipeline, Ghost) {
    let mut l = any_loop();
    l.pace.state = state;
    // The window's flags as `window_agrees` has them: WY matched if the
    // window is on, started inside it, its first tile fetched.
    let on = window != Window::Off;
    l.window_enabled = on;
    l.pace.window.wy_triggered = on;
    if window == Window::Armed {
        kani::assume(super::relation::signed(l.pace.bg.position) + 7 < i16::from(l.wx));
    }
    if window == Window::Inside {
        l.pace.window.wx_triggered = true;
        // Its first pixel not drawn for an object at WX + 1
        // (`window_inside_ok`).
        l.pace.window.being_fetched = kani::any();
        l.pace.window.tile_x = kani::any();
    }
    // The sleep's dots left as `between_dots` has them.
    l.pace.wait = if matches!(state, State::Mode3ObjectAttributes | State::Mode3ObjectLow) {
        1 + i32::from(kani::any::<bool>())
    } else {
        1
    };
    let mut g = any_ghost();
    if let Some(since) = since_push {
        g.since_push = since;
    }
    // The ghost counts the window's dots once ceres's check fired.
    if window == Window::Inside {
        g.window = INSIDE;
    } else {
        g.window = 0;
        g.window_fine = 0;
    }
    // Every related pair's netlist is `expected` of what it carries over
    // (`netlist_mode3_expected_carries`).
    let net = expected(&l, &any_carried(&l), g);
    (l, net, g)
}

/// The sleeps and windows of the `dot_from` harnesses, as `dot_from` takes
/// them.
const DOT_PAIRS: [(State, Option<u8>, Window); 24] = [
    (State::Mode3Pixel, None, Window::Off),
    (State::Mode3ObjectWait, None, Window::Off),
    (State::Mode3ObjectFetch, Some(0), Window::Off),
    (State::Mode3ObjectFetch, Some(1), Window::Off),
    (State::Mode3ObjectFetch, Some(2), Window::Off),
    (State::Mode3ObjectAttributes, None, Window::Off),
    (State::Mode3ObjectLow, None, Window::Off),
    (State::Mode3ObjectHigh, None, Window::Off),
    (State::HBlankStart, None, Window::Off),
    (State::Mode3Pixel, None, Window::Armed),
    (State::Mode3ObjectWait, None, Window::Armed),
    (State::Mode3ObjectFetch, Some(0), Window::Armed),
    (State::Mode3ObjectFetch, Some(1), Window::Armed),
    (State::Mode3ObjectFetch, Some(2), Window::Armed),
    (State::Mode3ObjectAttributes, None, Window::Armed),
    (State::Mode3ObjectLow, None, Window::Armed),
    (State::Mode3ObjectHigh, None, Window::Armed),
    (State::Mode3ObjectWait, None, Window::Inside),
    (State::Mode3ObjectFetch, Some(0), Window::Inside),
    (State::Mode3ObjectFetch, Some(1), Window::Inside),
    (State::Mode3ObjectFetch, Some(2), Window::Inside),
    (State::Mode3ObjectAttributes, None, Window::Inside),
    (State::Mode3ObjectLow, None, Window::Inside),
    (State::Mode3ObjectHigh, None, Window::Inside),
];

/// The related pairs of the `dot_from` harnesses reach each case below: a
/// harness of its own, without the dot, as the covers took a third of
/// theirs.
// @assumes transcription-render-edge
#[kani::proof]
#[kani::unwind(12)]
fn netlist_mode3_dot_covers() {
    let i: usize = kani::any();
    kani::assume(i < DOT_PAIRS.len());
    let (state, since_push, window) = DOT_PAIRS[i];
    let (l, net, g) = dot_pair(state, since_push, window);
    let last_scx = l.mixer_scx;
    let bg = &l.pace.bg;
    let p = super::relation::signed(bg.position);
    let related = related_but_pipeline(&l, &net, g);
    kani::cover!(
        related & l.objects_enabled & matches!(l.pace.state, State::Mode3ObjectWait),
        "object waits for the tile"
    );
    kani::cover!(related & (g.since_push == 0), "object pushed");
    kani::cover!(
        related & (g.since_push == 2) & (g.age == 7) & (bg.fifo.len() < 7),
        "netlist's fetcher ahead"
    );
    kani::cover!(related & (l.pace.objs.count >= 2), "two objects left");
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
        related & (window == Window::Armed) & (p + 8 == i16::from(l.wx)),
        "WX in the next dot"
    );
    kani::cover!(
        related & (window == Window::Inside) & (g.since_push == 0),
        "object pushed in the window"
    );
    kani::cover!(
        related & (window == Window::Inside) & l.pace.window.being_fetched,
        "the window's first pixel held for an object"
    );
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
// @assumes mode3-hooks-keep-pace
// @discharges mode3-end-background
#[kani::proof]
#[kani::unwind(12)]
fn netlist_mode3_dot_pixel() {
    dot_from(State::Mode3Pixel, None, Window::Off);
}

/// A dot from every related pair where an object's fetch waits for the tile.
// @assumes transcription-render-edge
// @assumes mapping-mode3-start
// @assumes mapping-scx-write
// @assumes mode3-hooks-keep-pace
// @discharges mode3-end-background
#[kani::proof]
#[kani::unwind(12)]
fn netlist_mode3_dot_wait() {
    dot_from(State::Mode3ObjectWait, None, Window::Off);
}

/// A dot from every related pair where an object's fetch starts as ceres pushes the last.
// @assumes transcription-render-edge
// @assumes mapping-mode3-start
// @assumes mapping-scx-write
// @assumes mode3-hooks-keep-pace
// @discharges mode3-end-background
#[kani::proof]
#[kani::unwind(12)]
fn netlist_mode3_dot_fetch_pushed() {
    dot_from(State::Mode3ObjectFetch, Some(0), Window::Off);
}

/// A dot from every related pair where an object's fetch starts a dot after ceres pushed the last.
// @assumes transcription-render-edge
// @assumes mapping-mode3-start
// @assumes mapping-scx-write
// @assumes mode3-hooks-keep-pace
// @discharges mode3-end-background
#[kani::proof]
#[kani::unwind(12)]
fn netlist_mode3_dot_fetch_after_push() {
    dot_from(State::Mode3ObjectFetch, Some(1), Window::Off);
}

/// A dot from every related pair where an object's fetch starts.
// @assumes transcription-render-edge
// @assumes mapping-mode3-start
// @assumes mapping-scx-write
// @assumes mode3-hooks-keep-pace
// @discharges mode3-end-background
#[kani::proof]
#[kani::unwind(12)]
fn netlist_mode3_dot_fetch() {
    dot_from(State::Mode3ObjectFetch, Some(2), Window::Off);
}

/// A dot from every related pair where an object's attributes are read.
// @assumes transcription-render-edge
// @assumes mapping-mode3-start
// @assumes mapping-scx-write
// @assumes mode3-hooks-keep-pace
// @discharges mode3-end-background
#[kani::proof]
#[kani::unwind(12)]
fn netlist_mode3_dot_attributes() {
    dot_from(State::Mode3ObjectAttributes, None, Window::Off);
}

/// A dot from every related pair where an object's low byte is read.
// @assumes transcription-render-edge
// @assumes mapping-mode3-start
// @assumes mapping-scx-write
// @assumes mode3-hooks-keep-pace
// @discharges mode3-end-background
#[kani::proof]
#[kani::unwind(12)]
fn netlist_mode3_dot_low() {
    dot_from(State::Mode3ObjectLow, None, Window::Off);
}

/// A dot from every related pair where an object's high byte is read.
// @assumes transcription-render-edge
// @assumes mapping-mode3-start
// @assumes mapping-scx-write
// @assumes mode3-hooks-keep-pace
// @discharges mode3-end-background
#[kani::proof]
#[kani::unwind(12)]
fn netlist_mode3_dot_high() {
    dot_from(State::Mode3ObjectHigh, None, Window::Off);
}

/// A dot from every related pair where the line is drawn.
// @assumes transcription-render-edge
// @assumes mapping-mode3-start
// @assumes mapping-scx-write
// @assumes mode3-hooks-keep-pace
// @discharges mode3-end-background
#[kani::proof]
#[kani::unwind(12)]
fn netlist_mode3_dot_drawn() {
    dot_from(State::HBlankStart, None, Window::Off);
}

/// A dot from every related pair with the window armed where the loop sleeps after a pixel.
// @assumes transcription-render-edge
// @assumes mapping-scx-write
// @assumes mode3-hooks-keep-pace
// @assumes lead-in-objects
// @discharges mode3-window-armed
#[kani::proof]
#[kani::unwind(12)]
fn netlist_mode3_armed_pixel() {
    dot_from(State::Mode3Pixel, None, Window::Armed);
}

/// A dot from every related pair with the window armed where an object's fetch waits for the tile.
// @assumes transcription-render-edge
// @assumes mapping-scx-write
// @assumes mode3-hooks-keep-pace
// @assumes lead-in-objects
// @discharges mode3-window-armed
#[kani::proof]
#[kani::unwind(12)]
fn netlist_mode3_armed_wait() {
    dot_from(State::Mode3ObjectWait, None, Window::Armed);
}

/// A dot from every related pair with the window armed where an object's fetch starts as ceres pushes the last.
// @assumes transcription-render-edge
// @assumes mapping-scx-write
// @assumes mode3-hooks-keep-pace
// @assumes lead-in-objects
// @discharges mode3-window-armed
#[kani::proof]
#[kani::unwind(12)]
fn netlist_mode3_armed_fetch_pushed() {
    dot_from(State::Mode3ObjectFetch, Some(0), Window::Armed);
}

/// A dot from every related pair with the window armed where an object's fetch starts a dot after ceres pushed the last.
// @assumes transcription-render-edge
// @assumes mapping-scx-write
// @assumes mode3-hooks-keep-pace
// @assumes lead-in-objects
// @discharges mode3-window-armed
#[kani::proof]
#[kani::unwind(12)]
fn netlist_mode3_armed_fetch_after_push() {
    dot_from(State::Mode3ObjectFetch, Some(1), Window::Armed);
}

/// A dot from every related pair with the window armed where an object's fetch starts.
// @assumes transcription-render-edge
// @assumes mapping-scx-write
// @assumes mode3-hooks-keep-pace
// @assumes lead-in-objects
// @discharges mode3-window-armed
#[kani::proof]
#[kani::unwind(12)]
fn netlist_mode3_armed_fetch() {
    dot_from(State::Mode3ObjectFetch, Some(2), Window::Armed);
}

/// A dot from every related pair with the window armed where an object's attributes are read.
// @assumes transcription-render-edge
// @assumes mapping-scx-write
// @assumes mode3-hooks-keep-pace
// @assumes lead-in-objects
// @discharges mode3-window-armed
#[kani::proof]
#[kani::unwind(12)]
fn netlist_mode3_armed_attributes() {
    dot_from(State::Mode3ObjectAttributes, None, Window::Armed);
}

/// A dot from every related pair with the window armed where an object's low byte is read.
// @assumes transcription-render-edge
// @assumes mapping-scx-write
// @assumes mode3-hooks-keep-pace
// @assumes lead-in-objects
// @discharges mode3-window-armed
#[kani::proof]
#[kani::unwind(12)]
fn netlist_mode3_armed_low() {
    dot_from(State::Mode3ObjectLow, None, Window::Armed);
}

/// A dot from every related pair with the window armed where an object's high byte is read.
// @assumes transcription-render-edge
// @assumes mapping-scx-write
// @assumes mode3-hooks-keep-pace
// @assumes lead-in-objects
// @discharges mode3-window-armed
#[kani::proof]
#[kani::unwind(12)]
fn netlist_mode3_armed_high() {
    dot_from(State::Mode3ObjectHigh, None, Window::Armed);
}

/// A dot from every related pair inside the window where an object's fetch waits for the tile.
// @assumes transcription-render-edge
// @assumes mapping-scx-write
// @assumes mode3-hooks-keep-pace
// @assumes lead-in-objects
// @discharges mode3-window-start
#[kani::proof]
#[kani::unwind(12)]
fn netlist_mode3_inside_wait() {
    dot_from(State::Mode3ObjectWait, None, Window::Inside);
}

/// A dot from every related pair inside the window where an object's fetch starts as ceres pushes the last.
// @assumes transcription-render-edge
// @assumes mapping-scx-write
// @assumes mode3-hooks-keep-pace
// @assumes lead-in-objects
// @discharges mode3-window-start
#[kani::proof]
#[kani::unwind(12)]
fn netlist_mode3_inside_fetch_pushed() {
    dot_from(State::Mode3ObjectFetch, Some(0), Window::Inside);
}

/// A dot from every related pair inside the window where an object's fetch starts a dot after ceres pushed the last.
// @assumes transcription-render-edge
// @assumes mapping-scx-write
// @assumes mode3-hooks-keep-pace
// @assumes lead-in-objects
// @discharges mode3-window-start
#[kani::proof]
#[kani::unwind(12)]
fn netlist_mode3_inside_fetch_after_push() {
    dot_from(State::Mode3ObjectFetch, Some(1), Window::Inside);
}

/// A dot from every related pair inside the window where an object's fetch starts.
// @assumes transcription-render-edge
// @assumes mapping-scx-write
// @assumes mode3-hooks-keep-pace
// @assumes lead-in-objects
// @discharges mode3-window-start
#[kani::proof]
#[kani::unwind(12)]
fn netlist_mode3_inside_fetch() {
    dot_from(State::Mode3ObjectFetch, Some(2), Window::Inside);
}

/// A dot from every related pair inside the window where an object's attributes are read.
// @assumes transcription-render-edge
// @assumes mapping-scx-write
// @assumes mode3-hooks-keep-pace
// @assumes lead-in-objects
// @discharges mode3-window-start
#[kani::proof]
#[kani::unwind(12)]
fn netlist_mode3_inside_attributes() {
    dot_from(State::Mode3ObjectAttributes, None, Window::Inside);
}

/// A dot from every related pair inside the window where an object's low byte is read.
// @assumes transcription-render-edge
// @assumes mapping-scx-write
// @assumes mode3-hooks-keep-pace
// @assumes lead-in-objects
// @discharges mode3-window-start
#[kani::proof]
#[kani::unwind(12)]
fn netlist_mode3_inside_low() {
    dot_from(State::Mode3ObjectLow, None, Window::Inside);
}

/// A dot from every related pair inside the window where an object's high byte is read.
// @assumes transcription-render-edge
// @assumes mapping-scx-write
// @assumes mode3-hooks-keep-pace
// @assumes lead-in-objects
// @discharges mode3-window-start
#[kani::proof]
#[kani::unwind(12)]
fn netlist_mode3_inside_high() {
    dot_from(State::Mode3ObjectHigh, None, Window::Inside);
}

/// A dot from every related pair with the window on, the objects on or
/// off, `window` dots after ceres's window check fired if given (0: armed, with
/// the check at WX; `INSIDE`: ten or more), any of the first nine if not;
/// the loop sleeps after a pixel, or has drawn the line, or (`waits`)
/// waits for the tile to fetch an object.
fn window_dot(window: Option<u8>, waits: bool) {
    let (mut l, mut net, g, _) = window_pair(window, waits);
    let related = related_but_pipeline(&l, &net, g);
    // Some pair is related, so the proof is not vacuous; the cases the
    // pairs reach are `netlist_mode3_window_covers`'s.
    kani::cover!(related, "a related pair");
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

/// The pairs `window_dot` takes, related or not, and whether ceres's loop
/// waits a dot for SCX's discard.
fn window_pair(window: Option<u8>, waits: bool) -> (Mode3Loop, NetPipeline, Ghost, bool) {
    let mut l = any_loop();
    // At WX 0 with SCX's low bits not 0, the check fired in the line's
    // first dot and the loop waits a dot (`relation::window_delay_ok`).
    let delay = (window == Some(0)) & kani::any::<bool>();
    l.pace.state = if waits {
        State::Mode3ObjectWait
    } else if delay {
        State::Mode3WindowDelay
    } else if kani::any() {
        State::Mode3Pixel
    } else {
        State::HBlankStart
    };
    l.pace.wait = 1;
    l.window_enabled = true;
    l.pace.window.wy_triggered = true;
    l.pace.window.wx_triggered = kani::any();
    l.pace.window.being_fetched = kani::any();
    l.pace.window.tile_x = kani::any();
    let mut g = any_ghost();
    if let Some(dots) = window {
        g.window = dots;
    } else {
        kani::assume((g.window >= 1) & (g.window < INSIDE));
    }
    let p = super::relation::signed(l.pace.bg.position);
    if g.window == 0 {
        g.window_fine = 0;
        kani::assume((p + 7 == i16::from(l.wx)) | delay);
    }
    let net = expected(&l, &any_carried(&l), g);
    (l, net, g, delay)
}

/// The windows and sleeps of the `window_dot` harnesses, as `window_dot`
/// takes them.
const WINDOW_PAIRS: [(Option<u8>, bool); 4] = [
    (Some(0), false),
    (None, false),
    (None, true),
    (Some(INSIDE), false),
];

/// The related pairs of the `window_dot` harnesses reach each case below,
/// in a harness of its own as `netlist_mode3_dot_covers`.
// @assumes transcription-render-edge
#[kani::proof]
#[kani::unwind(12)]
fn netlist_mode3_window_covers() {
    let i: usize = kani::any();
    kani::assume(i < WINDOW_PAIRS.len());
    let (window, waits) = WINDOW_PAIRS[i];
    let (l, net, g, delay) = window_pair(window, waits);
    let p = super::relation::signed(l.pace.bg.position);
    let related = related_but_pipeline(&l, &net, g);
    let drawn = matches!(l.pace.state, State::HBlankStart);
    kani::cover!(related & (g.window == 0) & !delay, "the check reaches WX");
    kani::cover!(related & delay, "the window waits for SCX's discard");
    kani::cover!(
        related & (l.wx == 0) & (g.window != 0) & (p < -7),
        "the lead-in in the window"
    );
    kani::cover!(related & g.blank, "a blank pixel drawn");
    kani::cover!(related & (g.window == 1), "the window's first dot");
    kani::cover!(related & (g.window == 6), "the window's row pushed");
    kani::cover!(related & (g.window == 9), "the start's last dot");
    kani::cover!(related & (g.window == 8) & drawn, "line drawn in the start");
    kani::cover!(related & (g.window == INSIDE) & !drawn, "in the window");
    kani::cover!(
        related & (g.window == INSIDE) & (l.pace.window.tile_x == 0),
        "the window's column wrapped"
    );
    kani::cover!(
        related & (g.window == INSIDE) & drawn,
        "line drawn in the window"
    );
    kani::cover!(
        related & waits & (g.window < DRAWS_AGAIN),
        "an object at WX + 1 stops the start"
    );
    kani::cover!(
        related & waits & (g.window == 9) & !l.pace.window.being_fetched,
        "an object stops the start's pixels"
    );
}

/// A dot from every related pair with the window armed and ceres's check at
/// WX: the window starts.
// @assumes transcription-render-edge
// @assumes mapping-scx-write
// @assumes mode3-hooks-keep-pace
// @assumes lead-in-objects
// @discharges mode3-window-start
#[kani::proof]
#[kani::unwind(12)]
fn netlist_mode3_window_reached() {
    window_dot(Some(0), false);
}

/// A dot from every related pair in the nine dots of the window's start.
// @assumes transcription-render-edge
// @assumes mapping-scx-write
// @assumes mode3-hooks-keep-pace
// @assumes lead-in-objects
// @discharges mode3-window-start
#[kani::proof]
#[kani::unwind(12)]
fn netlist_mode3_window_starting() {
    window_dot(None, false);
}

/// A dot from every related pair in the nine dots of the window's start
/// where ceres waits for the tile to fetch an object.
// @assumes transcription-render-edge
// @assumes mapping-scx-write
// @assumes mode3-hooks-keep-pace
// @assumes lead-in-objects
// @discharges mode3-window-start
#[kani::proof]
#[kani::unwind(12)]
fn netlist_mode3_window_starting_wait() {
    window_dot(None, true);
}

/// A dot from every related pair in the window, past its start.
// @assumes transcription-render-edge
// @assumes mapping-scx-write
// @assumes wx0-lead-in-matches
// @assumes mode3-hooks-keep-pace
// @assumes lead-in-objects
// @discharges mode3-window-start
#[kani::proof]
#[kani::unwind(12)]
fn netlist_mode3_window_inside() {
    window_dot(Some(INSIDE), false);
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
        blank: kani::any(),
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
// @assumes mode3-hooks-keep-pace
// @assumes lead-in-objects
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
