//! The ghost: the history neither pipeline keeps, and the shapes the relation allows.

#[cfg_attr(
    not(test),
    expect(clippy::wildcard_imports, reason = "the check's parts share its items")
)]
use super::*;

/// What the relation keeps besides the two pipelines: the history neither
/// holds. The harnesses take any; `Ghost::after` follows ceres's dot.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct Ghost {
    /// The lead-in went back to its start: the netlist's first fetch is
    /// long done.
    pub rerun: bool,
    /// Dots since ceres pushed an object's row (2: two or more).
    pub since_push: u8,
    /// Pixels the netlist has drawn past ceres's position: it stops for an
    /// object two dots after ceres does, and goes on two dots after it.
    pub extra: u8,
    /// The netlist's pixels stood still in the last dot.
    pub frozen: bool,
    /// Dots since the netlist's fetcher restarted for a tile (7: seven or
    /// more): it counts on while the pixels stand still.
    pub age: u8,
    /// Dots since ceres's window check fired on this line (0: it has not;
    /// `INSIDE`: ten or more).
    pub window: u8,
    /// The netlist's fine scroll's count in the first of them, which ceres's
    /// cleared FIFO no longer gives.
    pub window_fine: u8,
}

/// Ceres's loop before a dot, as `Ghost::after` takes it: its sleep, its
/// position, its FIFO's length, and whether the window had started.
#[derive(Clone, Copy)]
pub(crate) struct Before {
    state: State,
    wait: i32,
    position: u8,
    len: u8,
    window_started: bool,
}

impl Before {
    pub(crate) const fn of(pace: &Pace) -> Self {
        Self {
            state: pace.state,
            wait: pace.wait,
            position: pace.bg.position,
            len: pace.bg.fifo.len(),
            window_started: pace.window.wx_triggered,
        }
    }
}

impl Ghost {
    /// Nothing fetched on the line yet, with the netlist's fetcher as its
    /// fine scroll has it.
    pub(crate) const fn at_rest(bg: &Background) -> Self {
        Self {
            rerun: false,
            since_push: 2,
            extra: 0,
            frozen: false,
            age: fine_of(bg.fifo.len()),
            window: 0,
            window_fine: 0,
        }
    }

    /// After ceres's dot from `before`: the push of an object's row (the dot
    /// after its high byte), the netlist's pixels standing still for a fetch
    /// (from two dots after ceres's stop to two dots after its push), the
    /// lead-in back at its start, and the netlist's fetcher's restart, as
    /// its fine scroll begins a tile. Once ceres's window check has fired,
    /// the dots since, the last push's history through the start's first two
    /// dots, and the objects' history again from its seventh, ceres drawing.
    pub(crate) fn after(self, before: Before, pace: &Pace) -> Self {
        let (state, wait, position) = (before.state, before.wait, before.position);
        let fired = !before.window_started & pace.window.wx_triggered;
        let window = if fired {
            1
        } else if (self.window >= 1) & (self.window < INSIDE) {
            self.window + 1
        } else {
            self.window
        };
        // From the start's seventh dot ceres draws again, and an object at its
        // position (0 or more) stops it: the objects' history as outside it.
        let drawing = (window >= DRAWS_AGAIN) & (signed(pace.bg.position) >= 0);
        if (window != 0) & ((window < INSIDE) | (signed(pace.bg.position) < 0)) & !drawing {
            // At WX 0 the window can start before SCX's discard, and the
            // lead-in run on in it, back to its start too; no object is
            // fetched before the first pixel.
            let rerun = self.rerun | ((position == pos(-9)) & (pace.bg.position == pos(-16)));
            let window_fine = if fired {
                (fine_of(before.len) + 1) & 7
            } else {
                self.window_fine
            };
            // The last object's push may still hold the netlist in the
            // start's first two dots: the dots since it, whether the
            // netlist's pixels stood still, and its fetcher's age in its tile
            // (in the first dot, as its fine scroll there has it at rest).
            let since_push = if matches!(state, State::Mode3ObjectHigh) & (wait == 1) {
                0
            } else if self.since_push < 2 {
                self.since_push + 1
            } else {
                2
            };
            let frozen = since_push <= 1;
            let age = if !fired {
                // The second dot: on from the first.
                if self.age < 7 { self.age + 1 } else { 7 }
            } else if (signed(position) + i16::from(before.len) <= 0) & !rerun {
                window_fine
            } else if !frozen & (window_fine == 0) {
                0
            } else if self.age < 7 {
                self.age + 1
            } else {
                7
            };
            if window > 2 {
                return Self {
                    rerun,
                    window,
                    window_fine,
                    ..Self::at_rest(&pace.bg)
                };
            }
            return Self {
                rerun,
                since_push,
                extra: 0,
                frozen,
                age,
                window,
                window_fine,
            };
        }
        let pushed = matches!(state, State::Mode3ObjectHigh) & (wait == 1);
        let since_push = if pushed {
            0
        } else if self.since_push < 2 {
            self.since_push + 1
        } else {
            2
        };
        let popped = pace.bg.position != position;
        let frozen = (since_push <= 1) | ((self.extra == 2) & fetching(pace));
        let extra = (self.extra + u8::from(!frozen)).wrapping_sub(u8::from(popped));
        let rerun = self.rerun | ((position == pos(-9)) & (pace.bg.position == pos(-16)));
        let (p, len) = shifted(&pace.bg, extra);
        let fine = fine_of(len);
        // In the line's first round the netlist's first fetch is done (7) or
        // restarts (6) as its fine scroll goes; from there its fetcher
        // counts as in any tile, on while an object stops its pixels (an
        // object at X 0 stops them at the restart).
        let age = if (p + i16::from(len) <= 0) & !rerun & (len >= FULL - 2) & !frozen {
            fine
        } else if !frozen & (fine == 0) {
            0
        } else if self.age < 7 {
            self.age + 1
        } else {
            7
        };
        Self {
            rerun,
            since_push,
            extra,
            frozen,
            age,
            window,
            window_fine: self.window_fine,
        }
    }
}

/// Ceres's loop is fetching an object: it waits for the tile, or reads.
pub(super) const fn fetching(pace: &Pace) -> bool {
    matches!(
        pace.state,
        State::Mode3ObjectWait
            | State::Mode3ObjectFetch
            | State::Mode3ObjectAttributes
            | State::Mode3ObjectLow
            | State::Mode3ObjectHigh
    )
}

/// The shapes a line with the window idle passes through, with the mixer's
/// SCX in the last dot (`mixer_scx`) and the dot before (`latched`, which
/// the netlist's latch holds), and the ghost.
pub(super) fn reachable(l: &Mode3Loop, latched: u8, g: Ghost) -> bool {
    // At WX 0 ceres's check fires in the line's first dot, before the
    // lead-in could run again, and the lead-in does not run again in the
    // window (`lead_in_matches`).
    if l.window_enabled & (l.wx == 0) & g.rerun {
        return false;
    }
    match window_phase(l, g) {
        WindowPhase::Starting { dot, fine } => {
            return window_starting_ok(l, latched, g, dot, fine);
        }
        WindowPhase::Inside => return window_inside_ok(l, latched, g),
        WindowPhase::Armed if matches!(l.pace.state, State::Mode3WindowDelay) => {
            return window_delay_ok(l, g);
        }
        WindowPhase::Off | WindowPhase::Armed => {}
    }
    let bg = &l.pace.bg;
    let p = signed(bg.position);
    let len = bg.fifo.len();
    let shapes =
        (len >= 1) & (len <= FULL) & !bg.insert_blank & (p >= LEAD_IN) & (p <= i16::from(PX_WIDTH));
    // The ghost rests in the lead-in, but for an object at X 0 to 7, fetched
    // there.
    let lead_in = (p >= 0)
        | l.objects_enabled
        | (g == Ghost {
            rerun: g.rerun,
            ..Ghost::at_rest(bg)
        });
    // An object at X 0 stops ceres at the lead-in's start.
    let at_zero = (p == LEAD_IN) & (len == FULL) & fetching(&l.pace);
    shapes
        & (lead_in_ok(p, len, latched, l.mixer_scx, g.rerun) | at_zero)
        & lead_in
        & history_ok(&l.pace, g)
        & objects_ok(l, g)
        // Armed at WX 0, ceres's check fires in the line's first dot
        // (`window_delay_ok`), or matched SCX and reaches WX in its second,
        // its first pixel popped.
        & (!l.window_enabled | (l.wx != 0) | (p == LAST_DISCARDED - 1) & (len == FULL - 1))
}

/// The objects' history in the ghost against ceres's background: the
/// fetcher's step, the netlist's pixels ahead and its fetcher's age, as
/// ceres's object fetches and pushes leave them.
pub(super) const fn history_ok(pace: &Pace, g: Ghost) -> bool {
    let bg = &pace.bg;
    // The netlist's fetcher only runs ahead of its fine scroll's count.
    let (net_p, net_len) = shifted(bg, g.extra);
    let fetcher_ahead = ((net_p + net_len as i16 <= 0) & !g.rerun)
        | if g.frozen {
            // While the pixels stand still the fetcher counts on.
            (g.age > fine_of(net_len)) | (g.age == 7)
        } else if fine_of(net_len) == 0 {
            // The fine scroll began the tile in this dot: the fetcher
            // restarted.
            g.age == 0
        } else {
            g.age >= fine_of(net_len)
        };
    fetcher_ok(pace, g)
        & (g.since_push <= 2)
        & (g.extra <= 2)
        & (g.age <= 7)
        & fetcher_ahead
        & ghost_ok(pace, g)
        & fetch_age_ok(pace, g)
}

/// The lead-in from its start (at -16, the FIFO full) at position `p` with
/// `len` pixels of the tile left: unmatched, the position has gained a pixel
/// for each one the FIFO lost since the round began (at its start again only
/// on a `rerun`); matched, the position jumped to -8 and gained a pixel. The
/// netlist's compares still to come go as ceres's went (`pending_compares`).
pub(super) const fn lead_in_ok(p: i16, len: u8, latched: u8, mixer_scx: u8, rerun: bool) -> bool {
    let first_round = p + len as i16 <= 0;
    let round = if p <= LAST_UNMATCHED {
        first_round & ((p > LEAD_IN) | rerun) & (p + len as i16 == LEAD_IN + FULL as i16)
    } else {
        !first_round | (p > -8)
    };
    round & pending_compares(p, len, latched, mixer_scx, rerun)
}

/// Ceres's line at WX 0 after its first dot, with SCX's low bits not 0: its
/// check fired at the lead-in's start, cleared the FIFO, and waits a dot to
/// start the window (`Mode3WindowDelay`). The netlist drew a pixel of its
/// first round (the ghost's `extra`), and its fetcher is the first fetch's.
pub(super) fn window_delay_ok(l: &Mode3Loop, g: Ghost) -> bool {
    let bg = &l.pace.bg;
    (l.wx == 0)
        & (l.pace.wait == 1)
        & (signed(bg.position) == LEAD_IN)
        & bg.fifo.is_empty()
        & matches_step(bg.step, FetcherStep::GetTileT1)
        & !bg.insert_blank
        & !l.pace.window.being_fetched
        // The check reset the window's column before the loop slept.
        & (l.pace.window.tile_x == 0)
        & (l.mixer_scx & 7 != 0)
        & (g == Ghost {
            since_push: 2,
            extra: 1,
            frozen: false,
            age: 7,
            ..Ghost::at_rest(bg)
        })
        // Nothing drawn: the objects as the line started.
        & objects_ok(l, Ghost::at_rest(bg))
}

/// The ghost with nothing fetched, as the window's phases keep it.
pub(super) fn ghost_rests(l: &Mode3Loop, g: Ghost) -> bool {
    g == Ghost {
        rerun: g.rerun,
        window: g.window,
        window_fine: g.window_fine,
        ..Ghost::at_rest(&l.pace.bg)
    }
}

/// The ghost against ceres's phase: the netlist's pixels stand still from
/// the third dot of an object's fetch (two pixels past ceres's) to the dot
/// after ceres's push, and draw on, a pixel behind, as ceres's do.
pub(super) const fn ghost_ok(pace: &Pace, g: Ghost) -> bool {
    match g.since_push {
        // Ceres's push: it drew a pixel, or fetches the next object at the
        // same X.
        0 => {
            g.frozen
                & if fetching(pace) {
                    // The next object at the same X: its tile is there.
                    matches!(pace.state, State::Mode3ObjectFetch) & (g.extra == 2)
                } else {
                    g.extra == 1
                }
        }
        1 => {
            g.frozen
                & if fetching(pace) {
                    // Its attributes, or ceres drew a pixel and stopped at the
                    // next X.
                    (matches!(pace.state, State::Mode3ObjectAttributes)
                        & (pace.wait == 2)
                        & (g.extra == 2))
                        | (matches!(pace.state, State::Mode3ObjectWait | State::Mode3ObjectFetch)
                            & (g.extra == 1))
                } else {
                    g.extra == 0
                }
        }
        _ => {
            let reads = matches!(
                pace.state,
                State::Mode3ObjectAttributes | State::Mode3ObjectLow | State::Mode3ObjectHigh
            );
            let second = !matches!(pace.state, State::Mode3ObjectAttributes) | (pace.wait == 1);
            if reads {
                // The reads begin a dot after the stop at the earliest: the
                // netlist drew its two pixels, and stands still from the
                // attributes' second dot.
                (g.extra == 2) & (!second | g.frozen)
            } else if fetching(pace) {
                (g.extra >= 1) & (!g.frozen | (g.extra == 2))
            } else {
                (g.extra == 0) & !g.frozen
            }
        }
    }
}

/// While both stand still for an object, the netlist's fetcher counts a dot
/// a dot: two steps behind ceres's while ceres waits for the tile, then on
/// to done as ceres reads (4 as it starts, 7 by the low byte), or further
/// (its fine scroll's count, or an earlier object of the tile).
pub(super) const fn fetch_age_ok(pace: &Pace, g: Ghost) -> bool {
    let (_, net_len) = shifted(&pace.bg, g.extra);
    let fine = fine_of(net_len);
    if matches!(
        pace.state,
        State::Mode3ObjectFetch
            | State::Mode3ObjectAttributes
            | State::Mode3ObjectLow
            | State::Mode3ObjectHigh
    ) & !matches_step(pace.bg.step, FetcherStep::Push)
    {
        // The fetch began with the fetcher at its high byte's second dot or
        // further, a tile in the FIFO, and stepped it to its push.
        return false;
    }
    if matches!(pace.state, State::Mode3ObjectFetch)
        & (g.since_push == 2)
        & (g.extra == 2)
        & (net_len > pace.bg.fifo.len())
    {
        // Two pixels or fewer left: ceres's fetcher waited at its push, and
        // the fetch began in the dot of the stop.
        return false;
    }
    if matches!(pace.state, State::Mode3ObjectWait | State::Mode3ObjectFetch)
        & (g.since_push == 2)
        & (g.extra == 2)
    {
        // The fetch waited for the tile, so ceres's fetcher was not at its
        // push when it stopped: the netlist's runs two steps behind it, and
        // the fetch began as ceres's reached its high byte's second dot.
        let step = if matches!(pace.state, State::Mode3ObjectFetch) {
            FetcherStep::DataHighT2 as u8 + 1
        } else {
            pace.bg.step as u8
        };
        return g.age == step.wrapping_sub(2);
    }
    if fetching(pace) & (g.since_push <= 1) & (g.extra == 2) {
        // The next object at the same X: the netlist's fetcher was done
        // long before.
        return (g.age == 7) | ((net_len > pace.bg.fifo.len()) & (g.age == fine));
    }
    if matches!(pace.state, State::Mode3ObjectAttributes)
        & (pace.wait == 2)
        & (g.since_push == 2)
        & !g.frozen
        & (net_len <= pace.bg.fifo.len())
    {
        // The fetch began in the dot of the stop, ceres's fetcher at its high
        // byte's second dot or further (the netlist's at 3 or more), and the
        // netlist drew on for two dots.
        return g.age >= 5;
    }
    if !(fetching(pace) & g.frozen & (g.since_push == 2)) {
        return true;
    }
    if net_len > pace.bg.fifo.len() {
        return (g.age == fine) | (g.age == 7);
    }
    let phase = match pace.state {
        State::Mode3ObjectWait => (pace.bg.step as u8).wrapping_sub(2),
        State::Mode3ObjectFetch => 4,
        State::Mode3ObjectAttributes => {
            if pace.wait == 2 {
                5
            } else {
                6
            }
        }
        _ => 7,
    };
    // At least: an earlier object in the tile left the fetcher further on.
    g.age >= if fine > phase { fine } else { phase }
}

/// Ceres's fetcher: a tile's fetch takes six dots and waits to push its row
/// until the FIFO is empty (`step_for`), as the netlist's does a fine scroll
/// count behind; once the netlist's fetcher ran on while the pixels stood
/// still (its age past the fine scroll's count), ceres's ran on too, two
/// steps ahead of the netlist's count, to its push.
pub(super) const fn fetcher_ok(pace: &Pace, g: Ghost) -> bool {
    let bg = &pace.bg;
    let len = bg.fifo.len();
    let step = bg.step;
    let (_, net_len) = shifted(bg, g.extra);
    // Ceres pushed its row a dot or two ago, before the netlist's pixels
    // reach the tile and its fetcher restarts for it.
    let fresh = matches_step(step, step_for(len)) & (len >= FULL - 1) & (net_len >= FULL - 1);
    // In the first round at rest (no object at X 0 to 7 fetched in it) the
    // fetcher keeps to the FIFO.
    let rests = (g.since_push == 2) & (g.extra == 0) & !g.frozen & (g.age == fine_of(len));
    if first_round(bg) & !g.rerun & !fetching(pace) & rests {
        matches_step(step, step_for(len))
    } else if !fetching(pace) & (len == FULL) {
        // A full FIFO after a pixel: its row was pushed in this dot.
        matches_step(step, FetcherStep::GetTileT1)
    } else if net_len > len {
        // The netlist's pixels went on into the next tile: ceres's fetcher
        // waits to push it.
        matches_step(step, FetcherStep::Push)
    } else if g.age == fine_of(net_len) {
        // While ceres stops for an object its fetcher runs on with the
        // netlist's pixels; otherwise it keeps to its own FIFO.
        matches_step(step, step_for(if fetching(pace) { net_len } else { len }))
    } else if g.age <= 3 {
        step as u8 == g.age + 2
    } else {
        matches_step(step, FetcherStep::Push) | fresh
    }
}

/// The netlist's fine scroll still has to compare the positions ceres's
/// last two dots compared, the one before with `latched` (its count is then
/// 6 minus the FIFO's length), the last with `mixer_scx`: each matches if
/// and only if ceres's did. Its first fetch not done (the FIFO's first pixel
/// popped, `first_round`), its next dot does not compare. Ceres at position
/// `p`, `len` pixels of its tile left.
pub(super) const fn pending_compares(
    p: i16,
    len: u8,
    latched: u8,
    mixer_scx: u8,
    rerun: bool,
) -> bool {
    let count = FINE_AHEAD.wrapping_sub(len) & 7;
    let compares = !((p + len as i16 <= 0) & !rerun & (len == FULL - 1));
    let next = compares & (count == latched & 7);
    let after = (count + 1) & 7 == mixer_scx & 7;
    if p > LAST_DISCARDED {
        // The netlist has matched too, or matches in its next dot.
        true
    } else if p == LAST_DISCARDED {
        // Ceres matched the dot before: the netlist matches in its next dot,
        // and stops comparing.
        next
    } else {
        // Ceres matched in its last dot (-7), or has not.
        !next & (after == (p == LAST_DISCARDED - 1))
    }
}
