//! The window: off, armed, starting or inside, on both sides.

#[cfg_attr(
    not(test),
    expect(clippy::wildcard_imports, reason = "the check's parts share its items")
)]
use super::*;

/// The window's start takes nine dots: from the tenth, the netlist's
/// pipeline is the background's again, in the window.
pub(crate) const INSIDE: u8 = 10;

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
/// fired, its fine scroll at `fine` in the first, with the objects off.
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
pub(super) const fn window_starting(
    l: &Mode3Loop,
    net: &NetPipeline,
    dot: u8,
    fine: u8,
    rerun: bool,
) -> NetPipeline {
    let pace = &l.pace;
    let position = signed(pace.bg.position);
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
            pipeline_at(position + 1, len_at(fine), first, net)
        }
    } else if dot == 2 {
        let base = pipeline_at(position + 1, len_at(fine), false, &started);
        NetPipeline {
            px,
            fetcher: NetFetcher {
                count: 0,
                lyzu: (if fine < FETCH_DONE { fine } else { FETCH_DONE }) & 1 != 0,
                nyka: false,
                pory: false,
                pygo: (fine >= 6) | (fine == 0),
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
        let base = pipeline_at(position + 1, len_at(fine), false, &started);
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
    } else if dot == 8 {
        let base = pipeline_at(position, pace.bg.fifo.len(), false, &started);
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
            rene: false,
            pyco: wx_matched(l, position),
            nunu: wx_matched(l, position),
            rydy: false,
            sovy: false,
            win_x: 1,
            ..pipeline_at(position, pace.bg.fifo.len(), false, &started)
        }
    }
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

/// At WX 0 with SCX's low bits at 7, ceres's position comes back to WX's
/// (-7) as it pushes a tile after its match: its window inserts a blank
/// pixel there (`window_pixel_insertion`), drawn in the next dot with the
/// FIFO held, and its fetcher runs a step ahead of the FIFO for the tile.
/// The netlist inserts nothing: its fine scroll counts on to 7 in that dot,
/// and holds 7 for its match.
const fn blank_ahead(l: &Mode3Loop) -> bool {
    let bg = &l.pace.bg;
    let len = bg.fifo.len();
    (l.wx == 0) & (len >= 3) & (len <= FULL) & (bg.step as u8 == step_for(len) as u8 + 1)
}

/// The FIFO's length the netlist's fine scroll counts by: ceres's, but in
/// the dot ceres draws the blank pixel (`blank_ahead`, the FIFO full), a
/// pixel on.
const fn counted_len(l: &Mode3Loop) -> u8 {
    let len = l.pace.bg.fifo.len();
    if blank_ahead(l) & (len == FULL) {
        FULL - 1
    } else {
        len
    }
}

/// The netlist's pipeline ten dots or more after the window started: the
/// background's (in the lead-in at ceres's position and FIFO length, from
/// the first pixel on with the objects' history, `background`), in the
/// window (NOPA and its latch), its column a tile behind ceres's from
/// ceres's read of a tile's last byte (a dot earlier with the fetcher
/// ahead, `blank_ahead`) to the netlist's restart for it.
pub(super) const fn window_inside(l: &Mode3Loop, net: &NetPipeline, g: Ghost) -> NetPipeline {
    let pace = &l.pace;
    let position = signed(pace.bg.position);
    let inside = if position < 0 {
        pipeline_at(position, counted_len(l), false, net)
    } else {
        background(l, net, g)
    };
    let matched = wx_matched(l, position);
    // Ceres steps its column as it reads a tile's last byte, the netlist as
    // its fetcher restarts for the tile (its fine scroll at 0): between,
    // ceres holds the tile, or has pushed it and the netlist's pixels have
    // not reached its third. While an object is fetched the netlist's
    // pixels stand still and ceres's fetcher runs on.
    let read = if position < 0 {
        inside.fine + blank_ahead(l) as u8 >= 4
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
    let shape = if dot <= 5 {
        (len == 0)
            & (bg.step as u8 == dot)
            & (p == held)
            & pace.window.being_fetched
            & (pace.window.tile_x == 0)
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
        & ghost_rests(l, g)
        & objects_ok(l, g)
}

/// Ceres's loop ten dots or more after its window check fired: the
/// background's shapes past the window's start, a tile's fetch a FIFO, the
/// window's column within its five bits. At WX 0 the lead-in may run on
/// (`lead_in_ok`), and at its end insert a blank pixel (`blank_ahead`).
pub(super) fn window_inside_ok(l: &Mode3Loop, latched: u8, g: Ghost) -> bool {
    let pace = &l.pace;
    let bg = &pace.bg;
    let p = signed(bg.position);
    let len = bg.fifo.len();
    // No object before the first pixel: the ghost rests in the lead-in, and
    // from the blank's dot to the tile's push the fetcher is a step ahead.
    // From the first pixel on, the objects' history as outside the window.
    let history = if p < 0 {
        ghost_rests(l, g)
            & if blank_ahead(l) {
                p + i16::from(len) == 2
            } else {
                matches_step(bg.step, step_for(len))
            }
    } else {
        history_ok(pace, g)
    };
    // Ceres's window matches WX again at -7, the tile pushed.
    let blank = (p == LAST_DISCARDED - 1) & (len == FULL) & (l.wx == 0);
    let lead_in = if l.wx == 0 {
        // The lead-in's first round pops a pixel a dot from the start's
        // sixth (`lead_position`): four are gone by its tenth.
        let first_left = (p + i16::from(len) > 0) | (len <= FULL - (INSIDE - 6));
        // A tile pushed at -7 starts with the blank: no pixel of it popped
        // at -7.
        let unblanked = (p == LAST_DISCARDED) & (len == FULL - 1);
        (p >= lead_position(INSIDE))
            & first_left
            & !unblanked
            & lead_in_ok(p, counted_len(l), latched, l.mixer_scx, g.rerun)
    } else {
        p + 3 >= i16::from(l.wx)
    };
    (len >= 1)
        & (len <= FULL)
        & history
        & lead_in
        & (p <= i16::from(PX_WIDTH))
        & !pace.window.being_fetched
        & (pace.window.tile_x <= WIN_X_MASK)
        & (!bg.insert_blank | blank)
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
/// window starts in the next dot), or started, with the objects off.
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

/// With the objects on, the window's start clear of them: as ceres's check
/// reaches WX (and while the window starts), no object is being fetched,
/// the history of the last one is at rest (the netlist's pixels and fetcher
/// back with ceres's background, `ghost_rests`), and the next object matches past the start (its X at least WX + 5, the position
/// of the window's tenth dot plus 8). An object fetched in the start, or
/// just before it, is left out (`window-objects-clear`).
pub(crate) fn objects_clear(l: &Mode3Loop, g: Ghost) -> bool {
    let pace = &l.pace;
    let near = match window_phase(l, g) {
        WindowPhase::Armed => signed(pace.bg.position) + WX_AHEAD >= i16::from(l.wx),
        WindowPhase::Starting { .. } => true,
        WindowPhase::Off | WindowPhase::Inside => false,
    };
    let settled = ghost_rests(l, g) & !fetching(pace);
    let past_start = match pace.objs.next() {
        Some(e) => i16::from(e.x) >= start_position(l.wx, INSIDE) + OBJECT_AHEAD,
        None => true,
    };
    !(l.objects_enabled & l.window_enabled & near) | (settled & past_start)
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
