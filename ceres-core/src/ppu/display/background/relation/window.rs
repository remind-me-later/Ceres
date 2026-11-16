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
    if dot == 1 {
        pipeline_at(position + 1, len_at(fine), first, net)
    } else if dot == 2 {
        let base = pipeline_at(position + 1, len_at(fine), false, &started);
        NetPipeline {
            px: l.wx.wrapping_add(1),
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
            rene: fine == 0,
            // The fine scroll is held: it does not match through the start
            // (WX 1 starts the window a pixel before its match).
            puxa: false,
            nyze: false,
            nopa: false,
            sovy: false,
            ..base
        }
    } else if dot <= 7 {
        let base = pipeline_at(position + 1, len_at(fine), false, &started);
        NetPipeline {
            px: l.wx.wrapping_add(1),
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
            puxa: false,
            nyze: false,
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
            pyco: false,
            nunu: false,
            rydy: false,
            sovy: false,
            win_x: 1,
            ..pipeline_at(position, pace.bg.fifo.len(), false, &started)
        }
    }
}

/// The netlist's pipeline ten dots or more after the window started, with
/// the objects off: the background's at ceres's position and FIFO length,
/// in the window (NOPA and its latch), its column a tile behind ceres's
/// from ceres's read of a tile's last byte to the netlist's restart for it.
pub(super) const fn window_inside(l: &Mode3Loop, net: &NetPipeline) -> NetPipeline {
    let pace = &l.pace;
    let inside = pipeline_at(signed(pace.bg.position), pace.bg.fifo.len(), false, net);
    NetPipeline {
        pyco: false,
        nunu: false,
        nopa: true,
        in_window: true,
        rydy: false,
        sovy: false,
        win_x: pace.window.tile_x.wrapping_sub((inside.fine >= 4) as u8) & WIN_X_MASK,
        ..inside
    }
}

/// Ceres's loop `dot` dots (1 to 9) after its window check fired: the FIFO
/// cleared and the position held at WX's while the fetcher fetches the
/// window's first tile (five dots from its second step), the row pushed in
/// the sixth, and a pixel a dot from the seventh, the first of which ends
/// `being_fetched`.
pub(super) fn window_starting_ok(l: &Mode3Loop, g: Ghost, dot: u8, fine: u8) -> bool {
    let pace = &l.pace;
    let bg = &pace.bg;
    let p = signed(bg.position);
    let len = bg.fifo.len();
    let wx = i16::from(l.wx);
    let shape = if dot <= 5 {
        (len == 0)
            & (bg.step as u8 == dot)
            & (p + WX_AHEAD == wx)
            & pace.window.being_fetched
            & (pace.window.tile_x == 0)
    } else {
        (len == 14 - dot)
            & matches_step(bg.step, step_for(len))
            & (p + 13 - i16::from(dot) == wx)
            & (pace.window.being_fetched == (dot == 6))
            & (pace.window.tile_x == 1)
    };
    // With WX at 165 the line is drawn after the eighth dot.
    shape
        & (p <= i16::from(PX_WIDTH))
        & (fine <= 7)
        & !bg.insert_blank
        & ghost_rests(l, g)
        & objects_ok(l, g)
}

/// Ceres's loop ten dots or more after its window check fired: the
/// background's shapes past the window's start, a tile's fetch a FIFO, the
/// window's column within its five bits.
pub(super) fn window_inside_ok(l: &Mode3Loop, g: Ghost) -> bool {
    let pace = &l.pace;
    let bg = &pace.bg;
    let p = signed(bg.position);
    let len = bg.fifo.len();
    (len >= 1)
        & (len <= FULL)
        & matches_step(bg.step, step_for(len))
        & (p + 3 >= i16::from(l.wx))
        & (p <= i16::from(PX_WIDTH))
        & !pace.window.being_fetched
        & (pace.window.tile_x <= WIN_X_MASK)
        & !bg.insert_blank
        & ghost_rests(l, g)
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
/// netlist's REJO) and WX held between `FIRST_WX` and `LAST_WX`: armed, the
/// netlist's window waiting and ceres's check not past WX (at it, the
/// window starts in the next dot), or started, with the objects off.
pub(super) const fn window_agrees(l: &Mode3Loop, net: &NetPipeline, g: Ghost) -> bool {
    let wx_held = net.wy_latch & (net.wx == l.wx) & (l.wx >= FIRST_WX) & (l.wx <= LAST_WX);
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
            ceres_window_waits(l, true, true) & wx_held & !l.objects_enabled
        }
    }
}
