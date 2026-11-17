//! The objects: ceres's against the netlist's sprite store.

#[cfg_attr(
    not(test),
    expect(clippy::wildcard_imports, reason = "the check's parts share its items")
)]
use super::*;

/// As the line's loop starts, ceres's objects and the netlist's store hold
/// the same sprites (the OAM scan's proofs), none fetched, at X 8 or more.
pub(super) fn starting_objects(l: &Mode3Loop, net: &NetPipeline) -> bool {
    let rest = Ghost::at_rest(&l.pace.bg);
    (net.fetched == 0)
        & !l.pace.object_fetch_aborted
        & objects_ok(l, rest)
        & (!l.objects_enabled | objects_agree(l, net, rest))
}

/// Ceres's objects: left to draw from the nearest, all at X 8 or more (the
/// lead-in's are left out), none left behind the position; while it fetches
/// one, it is the nearest, at the position's X.
pub(super) fn objects_ok(l: &Mode3Loop, g: Ghost) -> bool {
    let objs = &l.pace.objs;
    let at = l.pace.bg.object_match_x();
    let mut ok = objs.count <= MAX_OBJECTS;
    let mut i = 0;
    while i < MAX_OBJECTS {
        if i < objs.count {
            ok &= objs.entries[i].x >= FIRST_OBJECT_X;
            if i + 1 < objs.count {
                ok &= objs.entries[i].x >= objs.entries[i + 1].x;
            }
        }
        i += 1;
    }
    let next = objs.next();
    // With the objects off, the loop drops those it passed a dot later.
    let ahead = match next {
        Some(e) => !l.objects_enabled | (e.x >= at),
        None => true,
    };
    let fetched = match next {
        Some(e) => l.objects_enabled & (e.x == at),
        None => false,
    };
    let (_, net_len) = shifted(&l.pace.bg, g.extra);
    // The fetcher at its fine scroll's count: in the window's first two
    // dots ceres's FIFO is cleared, and the count is the ghost's.
    let rest_age = match g.window {
        1 => g.window_fine,
        2 => {
            if g.window_fine < 7 {
                g.window_fine + 1
            } else {
                7
            }
        }
        _ => fine_of(net_len),
    };
    let quiet_line = l.objects_enabled
        | ((g.since_push == 2) & (g.extra == 0) & !g.frozen & (g.age == rest_age));
    // The fetch waits for the tile while the fetcher has not reached its
    // high byte's second dot, a step a dot (or the FIFO ran out and got its
    // row).
    let step = l.pace.bg.step as u8;
    let waits = !matches!(l.pace.state, State::Mode3ObjectWait)
        | (step <= FetcherStep::DataHighT2 as u8)
        | ((l.pace.bg.fifo.len() == FULL) & matches_step(l.pace.bg.step, FetcherStep::GetTileT1));
    ok & ahead & (!fetching(&l.pace) | fetched) & quiet_line & waits
}

/// The netlist's sprite store against ceres's objects: at each X up to 167,
/// as many slots not fetched yet whose X its pixel counter has not passed as
/// ceres has objects left (and the one ceres pushed in this dot, which the
/// netlist fetches a dot later).
pub(super) fn objects_agree(l: &Mode3Loop, net: &NetPipeline, g: Ghost) -> bool {
    if !l.objects_enabled {
        return true;
    }
    // The object ceres pushed in this dot, still in the netlist's store.
    let mut ok =
        (g.since_push != 0) | (net_objects_at(net, net.px) == objects_at(l, net, g, net.px));
    let mut slot = 0;
    while slot < SLOTS {
        if net_object(net, slot) {
            ok &= net_objects_at(net, net.xs[slot]) == objects_at(l, net, g, net.xs[slot]);
        }
        slot += 1;
    }
    let objs = &l.pace.objs;
    let mut i = 0;
    while i < MAX_OBJECTS {
        if (i < objs.count) && (objs.entries[i].x <= LAST_OBJECT_X) {
            let x = objs.entries[i].x;
            ok &= net_objects_at(net, x) == objects_at(l, net, g, x);
        }
        i += 1;
    }
    ok
}

/// A slot of the store holds an object the netlist will still fetch.
pub(super) const fn net_object(net: &NetPipeline, slot: usize) -> bool {
    (net.fetched & (1 << slot) == 0)
        & (net.xs[slot] != NO_X)
        & (net.xs[slot] >= net.px)
        & (net.xs[slot] <= LAST_OBJECT_X)
}

pub(super) const fn net_objects_at(net: &NetPipeline, x: u8) -> u8 {
    let mut n = 0;
    let mut slot = 0;
    while slot < SLOTS {
        if net_object(net, slot) & (net.xs[slot] == x) {
            n += 1;
        }
        slot += 1;
    }
    n
}

pub(super) fn objects_at(l: &Mode3Loop, net: &NetPipeline, g: Ghost, at: u8) -> u8 {
    let objs = &l.pace.objs;
    let mut found = u8::from((g.since_push == 0) & (at == net.px));
    let mut i = 0;
    while i < MAX_OBJECTS {
        if (i < objs.count) && (objs.entries[i].x == at) {
            found += 1;
        }
        i += 1;
    }
    found
}

/// The sprite store's fetched slots hold no X.
pub(super) const fn store_holds(net: &NetPipeline) -> bool {
    let mut holds = true;
    let mut slot = 0;
    while slot < SLOTS {
        holds &= (net.fetched & (1 << slot) == 0) | (net.xs[slot] == NO_X);
        slot += 1;
    }
    holds
}
