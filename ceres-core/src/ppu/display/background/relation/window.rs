//! The window: off, armed, starting or inside, on both sides.

#[cfg_attr(
    not(test),
    expect(clippy::wildcard_imports, reason = "the check's parts share its items")
)]
use super::*;

/// The window's start takes nine dots: from the tenth, the netlist's
/// pipeline is the background's again, in the window.
pub(crate) const INSIDE: u8 = 10;

/// The start's dot from which ceres draws again, the window's row pushed:
/// an object can stop it from there.
pub(crate) const DRAWS_AGAIN: u8 = 7;

/// Where the window is on a line.
#[derive(Clone, Copy)]
pub(crate) enum WindowPhase {
    /// LCDC's bit 5 clear, and no WY match in the frame.
    Off,
    /// On, WY matched in the frame, ceres's check short of WX or at it.
    Armed,
    /// `dot` dots (1 to 9) after ceres's check fired, with the netlist's
    /// fine scroll's count in the first.
    Starting { dot: u8, fine: u8 },
    /// Ten dots or more after it.
    Inside,
}

/// The window's phase, from LCDC's bit and the ghost's count.
pub(crate) const fn window_phase(l: &Mode3Loop, g: Ghost) -> WindowPhase {
    if !l.window_enabled {
        WindowPhase::Off
    } else if g.window == 0 {
        WindowPhase::Armed
    } else if g.window < INSIDE {
        WindowPhase::Starting {
            dot: g.window,
            fine: g.window_fine,
        }
    } else {
        WindowPhase::Inside
    }
}

/// The netlist's pipeline `dot` dots (1 to 9) after ceres's window check
/// fired, its fine scroll at `fine` in the first.
///
/// Dot 1 is one more dot of the background: ceres cleared its FIFO and holds
/// its position, the netlist drew a pixel. In dot 2 the netlist's pixel
/// counter passes WX (PYCO, NUNU; the window's latch, and RYDY, which holds
/// the dot clock) and its fetcher restarts, NYKA and PORY with it; through
/// dot 7 the fetcher counts to 5 with the pixels and the fine scroll held
/// (NOPA and SOVY from dot 3). In dot 8 the window's first tile is fetched
/// (SOVY, not RYDY any more): the fetcher and the fine scroll restart, with
/// no RYFA, and the window's column steps. In dot 9 the pixels go on, PYCO
/// and NUNU fall, and RENE has no RYFA to take.
///
/// An object's push in the dot before the check fired still holds the
/// netlist's pixels in dot 1, and its fetcher may be done (`g.age`); from
/// dot 7 an object stops ceres two dots before the netlist (the ghost's
/// `extra`), whose fetcher counts on while its pixels stand still.
pub(super) const fn window_starting(
    l: &Mode3Loop,
    net: &NetPipeline,
    dot: u8,
    fine: u8,
    g: Ghost,
) -> NetPipeline {
    let pace = &l.pace;
    let rerun = g.rerun;
    let position = signed(pace.bg.position);
    // The netlist's pixels: from dot 7 an object can stop ceres two dots
    // before the netlist (the ghost's `extra`).
    let (net_p, net_len) = shifted(&pace.bg, g.extra);
    // At WX 0 the netlist's window fires in the lead-in's first dot: its
    // fine scroll restarted for the line (0), and ceres's check fired at
    // the lead-in's start (`lead`, SCX's discard to come) or matched at -7
    // (SCX 0).
    let at_line_start = l.wx == 0;
    let lead = in_lead(l, dot);
    let fine = if at_line_start { 0 } else { fine };
    let held = if fine == 7 { 7 } else { fine + 1 };
    let started = NetPipeline {
        pyco: true,
        nunu: true,
        nopa: true,
        in_window: true,
        rydy: true,
        sovy: true,
        win_x: 0,
        ..*net
    };
    // The pixel the netlist draws in dot 1 is in the line's first round (the
    // junk row) when the window starts in the lead-in (WX 1), unless the
    // round is run again (`Ghost::rerun`), as `expected` has it.
    let first = (position + 1 + len_at(fine) as i16 <= 0) & !rerun;
    let px = if lead { 0 } else { l.wx.wrapping_add(1) };
    if dot == 1 {
        if at_line_start {
            // The lead-in's second dot: its first fetch restarted.
            pipeline_at(LEAD_IN, 6, true, net)
        } else {
            first_dot_after(
                l,
                pipeline_at(position + 1, len_at(fine), first, net),
                first,
                g,
            )
        }
    } else if dot == 2 {
        let base = pipeline_at(position + 1, len_at(fine), false, &started);
        // At WX 0 the lead-in's first fetch restarted in dot 1 (`fine`).
        let age = if at_line_start { fine + 1 } else { g.age };
        NetPipeline {
            px,
            fetcher: NetFetcher {
                count: 0,
                // A clock behind the fetcher's count, which restarted: as
                // it was a dot on in its tile.
                lyzu: fetcher_for(age).lyzu,
                nyka: false,
                pory: false,
                pygo: fetcher_for(age).pygo,
                ..base.fetcher
            },
            fine: held,
            ryfa: false,
            rene: (fine == 0) & !at_line_start,
            // The fine scroll is held: it does not match through the start
            // (WX 1 starts the window a pixel before its match), but at WX 0
            // with SCX 0 it matched in dot 1 and holds the match.
            puxa: at_line_start & !lead,
            nyze: at_line_start & !lead,
            discard: if at_line_start { lead } else { base.discard },
            nopa: false,
            sovy: false,
            ..base
        }
    } else if dot <= 7 {
        // In dot 7 ceres may have stopped for an object, the netlist not.
        let base = pipeline_at(net_p + 1, len_at(fine), false, &started);
        NetPipeline {
            px,
            fetcher: NetFetcher {
                count: dot - 2,
                lyzu: dot & 1 == 0,
                nyka: false,
                pory: false,
                pygo: false,
                ..base.fetcher
            },
            fine: held,
            ryfa: false,
            rene: false,
            puxa: at_line_start & !lead,
            nyze: at_line_start & !lead,
            discard: if at_line_start { lead } else { base.discard },
            ..base
        }
    } else {
        first_tile_fetched(
            l,
            pipeline_at(net_p, net_len, false, &started),
            dot,
            g,
            net_p,
        )
    }
}

/// The netlist's pipeline in the window's eighth and ninth dots, `base` as
/// its pixels at `net_p` have it: the window's first tile fetched.
const fn first_tile_fetched(
    l: &Mode3Loop,
    base: NetPipeline,
    dot: u8,
    g: Ghost,
    net_p: i16,
) -> NetPipeline {
    if dot == 8 {
        NetPipeline {
            fetcher: NetFetcher {
                pygo: false,
                ..base.fetcher
            },
            ryfa: false,
            rydy: false,
            win_x: 1,
            ..base
        }
    } else {
        NetPipeline {
            // Its fetcher counts on if an object stopped its pixels.
            fetcher: NetFetcher {
                started: base.fetcher.started,
                ..fetcher_for(g.age)
            },
            ryfa: base.ryfa & !window_ryfa_held(l, net_p),
            rene: false,
            pyco: wx_matched(l, net_p),
            nunu: wx_matched(l, net_p),
            rydy: false,
            sovy: false,
            win_x: 1,
            ..base
        }
    }
}

/// The netlist's pipeline in the window's first dot, `e` as its pixels
/// have it: the last object's history as `background` has it, the
/// fetcher's age in its tile (but in the line's first round, `first`), the
/// pixels standing still for the push, and the sprite fetch at its end.
const fn first_dot_after(l: &Mode3Loop, mut e: NetPipeline, first: bool, g: Ghost) -> NetPipeline {
    if !first {
        e.fetcher = NetFetcher {
            started: e.fetcher.started,
            ..fetcher_for(g.age)
        };
    }
    if g.frozen {
        e.rene = e.ryfa;
    }
    if let Some(phase) = SpritePhase::of(g.since_push, l.pace.state, l.pace.wait) {
        e.sprite = phase.net();
    }
    e
}

/// Ceres's position `dot` dots (1 to 9) after its window check fired, the
/// check matched at WX: held at WX's through the window's first fetch, then
/// a pixel a dot from the seventh.
const fn start_position(wx: u8, dot: u8) -> i16 {
    let drawn = dot.saturating_sub(6);
    wx as i16 - WX_AHEAD + drawn as i16
}

/// Ceres's position `dot` dots after its check fired at the lead-in's start
/// at WX 0, SCX not matched yet: held there through the window's first
/// fetch, then a pixel a dot from the seventh, as at WX's.
const fn lead_position(dot: u8) -> i16 {
    LEAD_IN + dot.saturating_sub(6) as i16
}

/// At WX 0 ceres's check fired at the lead-in's start (SCX's low bits not
/// 0): its position is short of WX's, the lead-in to run in the window.
pub(super) const fn in_lead(l: &Mode3Loop, dot: u8) -> bool {
    (l.wx == 0) & (signed(l.pace.bg.position) < start_position(0, dot))
}

/// The netlist's WX match (PYCO, and NUNU's latch) holds while its pixel
/// counter is at WX + 1 or short of it: past the start's eighth dot only at
/// WX 0, where the counter waits for SCX's discard.
const fn wx_matched(l: &Mode3Loop, position: i16) -> bool {
    position + PX_AHEAD <= l.wx as i16 + 1
}

/// At WX 0 with SCX's low bits at 7, ceres's last pixel before WX's
/// position (-7) is its tile's last, the next tile due there: its window
/// inserts a blank pixel before it (`window_pixel_insertion`), which takes
/// the match at -9, and pushes the tile a dot later, as the netlist loads
/// it. The netlist inserts nothing: its fine scroll counts on to 7 and
/// holds 7 for its match, a pixel on from ceres's FIFO until ceres pops a
/// pixel of the tile (the ghost's `blank`).
const fn counted_len(l: &Mode3Loop, g: Ghost) -> u8 {
    l.pace.bg.fifo.len().saturating_sub(g.blank as u8)
}

/// Where ceres's blank pixel leaves it: at -7 with the tile's last pixel,
/// or at -6 with the next tile pushed.
const fn blank_ok(l: &Mode3Loop, g: Ghost) -> bool {
    let (p, len) = (signed(l.pace.bg.position), l.pace.bg.fifo.len());
    !g.blank
        | (l.wx == 0)
            & ((p == LAST_DISCARDED - 1) & (len == 1) | (p == LAST_DISCARDED) & (len == FULL))
}

/// The netlist's pipeline ten dots or more after the window started: the
/// background's (in the lead-in at ceres's position and FIFO length, but
/// for its blank pixel, `counted_len`; from the first pixel on with the
/// objects' history, `background`), in the window (NOPA and its latch),
/// its column a tile behind ceres's from ceres's read of a tile's last byte
/// to the netlist's restart for it.
pub(super) const fn window_inside(l: &Mode3Loop, net: &NetPipeline, g: Ghost) -> NetPipeline {
    let pace = &l.pace;
    let position = signed(pace.bg.position);
    let inside = if position < 0 {
        pipeline_at(position, counted_len(l, g), false, net)
    } else {
        background(l, net, g)
    };
    // The netlist's pixel counter, ahead of ceres's while an object stops it.
    let net_p = shifted(&pace.bg, g.extra).0;
    let matched = wx_matched(l, net_p);
    let ryfa_held = window_ryfa_held(l, net_p);
    // Ceres steps its column as it reads a tile's last byte, the netlist as
    // its fetcher restarts for the tile (its fine scroll at 0): between,
    // ceres holds the tile, or has pushed it and the netlist's pixels have
    // not reached its third. While an object is fetched the netlist's
    // pixels stand still and ceres's fetcher runs on.
    let read = if position < 0 {
        inside.fine >= 4
    } else {
        matches_step(pace.bg.step, FetcherStep::Push) | (inside.fine >= 6)
    };
    NetPipeline {
        pyco: matched,
        nunu: matched,
        nopa: true,
        in_window: true,
        rydy: false,
        sovy: false,
        win_x: pace.window.tile_x.wrapping_sub(read as u8) & WIN_X_MASK,
        ryfa: inside.ryfa & !ryfa_held,
        // Its first pixel past the window's eighth dot finds no RYFA, as in
        // the start's ninth: RENE takes the RYFA it brought a dot later, as
        // the pixels stand still there.
        rene: inside.rene
            & !ryfa_held
            & !((net_p >= 0) & (net_p == start_position(l.wx, 9)) & !g.frozen),
        ..inside
    }
}

/// Ceres's loop `dot` dots (1 to 9) after its window check fired: the FIFO
/// cleared and the position held (`start_position`) while the fetcher
/// fetches the window's first tile (five dots from its second step), the
/// row pushed in the sixth, and a pixel a dot from the seventh, the first
/// of which ends `being_fetched`. In the lead-in (`in_lead`) the position is
/// held at its start, and from the seventh dot runs the lead-in
/// (`lead_in_ok`).
pub(super) fn window_starting_ok(l: &Mode3Loop, latched: u8, g: Ghost, dot: u8, fine: u8) -> bool {
    let pace = &l.pace;
    let bg = &pace.bg;
    let p = signed(bg.position);
    let len = bg.fifo.len();
    let lead = in_lead(l, dot);
    let held = if lead {
        lead_position(dot)
    } else {
        start_position(l.wx, dot)
    };
    // From dot 7 an object at ceres's position stops it, waiting for the
    // tile: its pixels held since, the netlist's two dots behind
    // (`Ghost::after`), its fetcher running on (`history_ok`).
    let drawing = (dot >= DRAWS_AGAIN) & (p >= 0);
    let stopped = drawing & matches!(pace.state, State::Mode3ObjectWait);
    let shape = if dot <= 5 {
        (len == 0)
            & (bg.step as u8 == dot)
            & (p == held)
            & pace.window.being_fetched
            & (pace.window.tile_x == 0)
    } else if stopped {
        // Stopped at the start's position (the object at WX + 1), ceres has
        // not drawn the window's first pixel.
        (p + i16::from(len) == start_position(l.wx, 6) + i16::from(FULL))
            & (p >= start_position(l.wx, 6))
            & (p < held)
            & (pace.window.being_fetched == (p == start_position(l.wx, 6)))
            & (pace.window.tile_x == 1)
    } else {
        (len == 14 - dot)
            & matches_step(bg.step, step_for(len))
            & if lead & (dot >= 7) {
                // Eight pixels from the seventh dot.
                lead_in_ok(p, len, latched, l.mixer_scx, false)
            } else {
                p == held
            }
            & (pace.window.being_fetched == (dot == 6))
            & (pace.window.tile_x == 1)
    };
    // At WX 0 the netlist's fine scroll compares in dot 2 the SCX ceres's
    // check fired with (`pending_compares`): it matches with SCX 0, and in
    // the lead-in it does not.
    let matched = (l.wx != 0) | (dot != 1) | ((latched.trailing_zeros() >= 3) != lead);
    // With WX at 165 the line is drawn after the eighth dot.
    shape
        & matched
        & (p <= i16::from(PX_WIDTH))
        & (fine <= 7)
        & !bg.insert_blank
        & if stopped {
            // The netlist draws on to two pixels past ceres's, where the
            // start would have it, and stands still from there; its fetcher
            // restarted for the tile in the eighth dot, as without the
            // object.
            let behind = held - p;
            let extra = if behind < 2 { behind } else { 2 };
            (g.since_push == 2)
                & (i16::from(g.extra) == extra)
                & (g.frozen == (behind > 2))
                & (g.age == fine_of(14 - dot))
                & history_ok(pace, g)
        } else {
            start_history_ok(l, g, dot)
        }
        & objects_ok(l, g)
}

/// The ghost through the window's start: in its first two dots the last
/// object's push may still hold the netlist (pushed in the dot before the
/// check fired, its pixels stand still in the first and its fetcher has
/// counted on through the fetch: from its fifth dot, the netlist stopped
/// two dots after ceres; not in the lead-in, at WX 0 to 6, left out), or
/// its fetcher ran on past its fine scroll's count (counting through an
/// earlier object's fetch), or is at the count (`fine`, in the first dot);
/// then it rests.
fn start_history_ok(l: &Mode3Loop, g: Ghost, dot: u8) -> bool {
    let fine = g.window_fine;
    let after_lead_in = l.wx >= super::FIRST_OBJECT_X - 1;
    let held = (g.since_push == 1) & g.frozen & ((g.age >= 5) | (g.age > fine)) & after_lead_in;
    let on = (g.since_push == 2) & !g.frozen;
    // In the line's first round the netlist's fetcher is at its fine
    // scroll's count in the first dot (`window_starting`).
    let first = (signed(l.pace.bg.position) + 1 + i16::from(len_at(fine)) <= 0) & !g.rerun;
    match dot {
        1 => {
            (g.extra == 0) & (held | on & ((g.age == fine) | (g.age > fine) & (fine != 0) & !first))
        }
        2 => (g.extra == 0) & on & ((g.age >= 6) | (g.age > fine)),
        _ => ghost_rests(l, g),
    }
}

/// Ceres's loop ten dots or more after its window check fired: the
/// background's shapes past the window's start, a tile's fetch a FIFO, the
/// window's column within its five bits. At WX 0 the lead-in may run on
/// (`lead_in_ok`), and before its last pixel insert a blank one (`blank_ok`).
pub(super) fn window_inside_ok(l: &Mode3Loop, latched: u8, g: Ghost) -> bool {
    let pace = &l.pace;
    let bg = &pace.bg;
    let p = signed(bg.position);
    let len = bg.fifo.len();
    // No object before the first pixel: the ghost rests in the lead-in.
    // From the first pixel on, the objects' history as outside the window.
    let history = if p < 0 {
        ghost_rests(l, g) & matches_step(bg.step, step_for(len))
    } else {
        history_ok(pace, g)
    };
    let lead_in = if l.wx == 0 {
        // The lead-in's first round pops a pixel a dot from the start's
        // sixth (`lead_position`): four are gone by its tenth.
        let first_left = (p + i16::from(len) > 0) | (len <= FULL - (INSIDE - 6));
        // No tile is pushed at -7: the blank pixel comes before it, so no
        // pop from a full FIFO leaves -6 with seven pixels.
        let at_wx = (p == LAST_DISCARDED - 1) & (len == FULL);
        let popped_at_wx = (p == LAST_DISCARDED) & (len == FULL - 1);
        (p >= lead_position(INSIDE))
            & first_left
            & !at_wx
            & !popped_at_wx
            & blank_ok(l, g)
            & lead_in_ok(p, counted_len(l, g), latched, l.mixer_scx, g.rerun)
    } else {
        // Four pixels drawn past WX's position, or fewer for an object
        // that stopped ceres from the start's seventh dot (drawing).
        (p + 3 >= i16::from(l.wx))
            | l.objects_enabled & (p >= 0) & (p + WX_AHEAD >= i16::from(l.wx))
    };
    let reached = start_position(l.wx, 8).min(i16::from(PX_WIDTH));
    (len >= 1)
        & (len <= FULL)
        & history
        & lead_in
        & (p <= i16::from(PX_WIDTH))
        // The window's first pixel not drawn if and only if an object at
        // WX + 1 stopped ceres at the start's position: a pixel drawn moves
        // it on.
        & (pace.window.being_fetched == first_pixel_held(l))
        // Its pixels stand two past ceres's, at the start's eighth dot's.
        & (!pace.window.being_fetched | (g.extra == 2))
        // But at WX 0 the netlist's pixels reached the start's eighth dot's
        // in it, whatever the objects (from X 8 on, none stops ceres
        // before).
        & ((l.wx == 0) | (p + i16::from(g.extra) >= reached))
        & (pace.window.tile_x <= WIN_X_MASK)
        & !bg.insert_blank
        & (!g.blank | (l.wx == 0))
        & objects_ok(l, g)
}

/// The write strobe does not latch WX in an edge: the bus changes its
/// address a few nanoseconds after the strobe, so the strobe meets the
/// address of the edge before too.
pub(super) const fn holds_wx(before: &EdgeInputs, now: &EdgeInputs) -> bool {
    !now.cpu_wr_sync | ((before.addr != WX) & (now.addr != WX))
}

/// The netlist's window at rest: no WY match in the frame, not in it, its
/// first tile not wanted.
pub(super) const fn net_window_off(net: &NetPipeline) -> bool {
    !net.sary & !net.wy_latch & net_window_waits(net)
}

/// The netlist's window not started on this line: no WX match, not in it,
/// its first tile not wanted, its column at 0.
pub(super) const fn net_window_waits(net: &NetPipeline) -> bool {
    !net.pyco & !net.nunu & !net.nopa & !net.in_window & !net.rydy & !net.sovy & (net.win_x == 0)
}

/// Ceres's window with no write to it landing: WY's match in the frame and
/// its start on this line as the phase has them (`being_fetched` only once
/// it started: `reachable` says when).
pub(super) const fn ceres_window_waits(l: &Mode3Loop, wy_triggered: bool, started: bool) -> bool {
    let w = &l.pace.window;
    (w.wy_triggered == wy_triggered)
        & (w.wx_triggered == started)
        & (started | !w.being_fetched)
        & !w.started_after_line
        & !w.wx_166_interrupt_glitch
        & !w.enable_pending
        & !w.wy_just_checked
        & !w.wx_just_changed
}

/// The ghost before the window starts: no dot counted.
pub(super) const fn window_unstarted(g: Ghost) -> bool {
    (g.window == 0) & (g.window_fine == 0)
}

/// The window on both sides: off (LCDC's bit 5 clear, and no WY match in
/// the frame), or on, WY matched in the frame (ceres's `wy_triggered`, the
/// netlist's REJO) and WX held up to `LAST_WX`: armed, the
/// netlist's window waiting and ceres's check not past WX (at it, the
/// window starts in the next dot), or started.
pub(super) const fn window_agrees(l: &Mode3Loop, net: &NetPipeline, g: Ghost) -> bool {
    let wx_held = net.wy_latch & (net.wx == l.wx) & (l.wx <= LAST_WX);
    match window_phase(l, g) {
        WindowPhase::Off => {
            ceres_window_waits(l, false, false) & net_window_off(net) & window_unstarted(g)
        }
        WindowPhase::Armed => {
            ceres_window_waits(l, true, false)
                & window_unstarted(g)
                & net_window_waits(net)
                & wx_held
                & (signed(l.pace.bg.position) + WX_AHEAD <= l.wx as i16)
        }
        // The netlist's window is `expected`'s there.
        WindowPhase::Starting { .. } | WindowPhase::Inside => {
            ceres_window_waits(l, true, true) & wx_held
        }
    }
}

/// At WX 0 the lead-in that runs in the window matches SCX before it would
/// go back to its start: at -9 SCX's low bits are 7. Otherwise the
/// netlist's fine scroll waits at 7 for a match, where ceres's lead-in
/// starts again (`window-wx-0-lead-in-unmatched`, left out:
/// `wx0-lead-in-matches`).
pub(super) const fn lead_in_matches(l: &Mode3Loop, mixer_scx: u8) -> bool {
    // @fact window-wx-0-lead-in-unmatched
    let at_end = l.window_enabled
        & (l.wx == 0)
        & l.pace.window.wx_triggered
        & (signed(l.pace.bg.position) == LAST_UNMATCHED);
    !at_end | (mixer_scx & 7 == 7)
}

/// Ceres waits at the window's start position, its first pixel not drawn,
/// for an object at WX + 1 (its X 8 or more: WX 7 or more).
const fn first_pixel_held(l: &Mode3Loop) -> bool {
    let p = signed(l.pace.bg.position);
    l.objects_enabled & (p >= 0) & (p == start_position(l.wx, 6))
}

/// The window's eighth dot restarts the netlist's fine scroll with no RYFA,
/// its pixels at the start's eighth position, always reached in that dot:
/// while they stand still there (an object at WX + 1), RYFA stays low, and
/// RENE, which takes it at each clock, with it.
const fn window_ryfa_held(l: &Mode3Loop, net_p: i16) -> bool {
    (net_p >= 0) & (net_p == start_position(l.wx, 8))
}
