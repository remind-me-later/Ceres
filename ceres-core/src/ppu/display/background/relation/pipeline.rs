//! The netlist's pipeline as ceres's loop and the ghost give it.

#[cfg_attr(
    not(test),
    expect(clippy::wildcard_imports, reason = "the check's parts share its items")
)]
use super::*;

/// The position as a signed value: the lead-in's 240..=255 are -16..=-1.
pub(crate) const fn signed(position: u8) -> i16 {
    if position >= pos(-16) {
        position as i16 - 256
    } else {
        position as i16
    }
}

/// The fetcher's step after a dot that leaves `len` pixels in the FIFO: a
/// tile's fetch takes six dots, and it waits to push its row until the FIFO
/// is empty.
pub(crate) const fn step_for(len: u8) -> FetcherStep {
    match len {
        8 => FetcherStep::GetTileT1,
        7 => FetcherStep::GetTileT2,
        6 => FetcherStep::DataLowT1,
        5 => FetcherStep::DataLowT2,
        4 => FetcherStep::DataHighT1,
        3 => FetcherStep::DataHighT2,
        _ => FetcherStep::Push,
    }
}

/// The FIFO still holds the junk row mode 3 starts with: the position has not
/// passed the FIFO's pixels since the start.
pub(super) const fn first_round(bg: &Background) -> bool {
    signed(bg.position) + bg.fifo.len() as i16 <= 0
}

/// The netlist's pipeline with its pixels at position `p` (ceres's
/// numbering) and `len` pixels of the tile left, with its SCX, its sprite
/// store and its inputs as `net` has them; `first` while the FIFO holds the
/// junk row mode 3 starts with (and the lead-in did not go back to its
/// start: the first fetch's restart, not the fine scroll's, began the tile).
/// No sprite is fetched: the fetch's steps rest at their last, TAKA until
/// the first fetch is done. The fetcher's flip-flops are `fetcher_for` the
/// fine scroll's count.
#[expect(
    clippy::cast_sign_loss,
    clippy::cast_possible_truncation,
    reason = "the pixel counter, positive by then"
)]
pub(crate) const fn pipeline_at(p: i16, len: u8, first: bool, net: &NetPipeline) -> NetPipeline {
    let mut fine = FINE_AHEAD.wrapping_sub(len) & 7;
    let mut fetch_count = if fine < FETCH_DONE { fine } else { FETCH_DONE };
    let before = (fine + 7) & 7;
    let mut lyzu = (if before < FETCH_DONE {
        before
    } else {
        FETCH_DONE
    }) & 1
        != 0;
    let mut nyka = (fine >= 6) | (fine == 0);
    let mut pory = nyka;
    let mut pygo = (fine == 7) | (fine <= 1);
    let mut started = true;
    let mut ryfa = fine == 0;
    let mut rene = fine == 1;
    let mut busy = false;
    if first {
        // The first fetch's restart, not the fine scroll's, began the tile.
        rene = false;
        if len == 7 {
            fetch_count = FETCH_DONE;
            (lyzu, nyka, pory, pygo, started, fine) = (false, false, false, false, false, 0);
            busy = true;
        } else if len == 6 {
            fetch_count = 0;
            (lyzu, nyka, pory, pygo, started, fine, ryfa) =
                (true, true, true, false, false, 0, false);
        } else {
            // The first tile runs as the others.
        }
    }
    NetPipeline {
        mode3: true,
        voga: false,
        px: if p + PX_AHEAD > 0 {
            (p + PX_AHEAD) as u8
        } else {
            0
        },
        fetcher: NetFetcher {
            count: fetch_count,
            lyzu,
            nyka,
            pory,
            pygo,
            started,
        },
        fine,
        puxa: p == NET_MATCH,
        nyze: p == NET_MATCH,
        discard: p <= LAST_DISCARDED,
        ryfa,
        rene,
        sprite: NetSpriteFetch {
            busy,
            ..at_rest(net).sprite
        },
        ..at_rest(net)
    }
}

/// The netlist's fetcher `age` dots after it restarted for a tile (7: seven
/// or more), its first fetch done (`started`): it counts to 5, and NYKA,
/// PORY and PYGO follow it a clock apart.
pub(super) const fn fetcher_for(age: u8) -> NetFetcher {
    let before = (age + 7) & 7;
    let waits = (age >= 6) | (age == 0);
    NetFetcher {
        count: if age < FETCH_DONE { age } else { FETCH_DONE },
        lyzu: (if before < FETCH_DONE {
            before
        } else {
            FETCH_DONE
        }) & 1
            != 0,
        nyka: waits,
        pory: waits,
        pygo: (age == 7) | (age <= 1),
        started: true,
    }
}

/// Where the netlist's sprite fetch is, locked to ceres's object fetch: the
/// dot of ceres's push and the one after, then, two or more dots after a
/// push, as ceres's reads go (the attributes' read, the low byte's two dots,
/// the high byte's).
#[derive(Clone, Copy)]
pub(super) enum SpritePhase {
    Pushed,
    AfterPush,
    Attributes,
    LowFirst,
    Low,
    High,
}

impl SpritePhase {
    /// The phase after ceres's dot, from the dots since its last push and
    /// its loop's sleep; `None` between fetches.
    const fn of(since_push: u8, state: State, wait: i32) -> Option<Self> {
        match since_push {
            0 => Some(Self::Pushed),
            1 => Some(Self::AfterPush),
            _ => match (state, wait) {
                (State::Mode3ObjectAttributes, 1) => Some(Self::Attributes),
                (State::Mode3ObjectLow, 2) => Some(Self::LowFirst),
                (State::Mode3ObjectLow, _) => Some(Self::Low),
                (State::Mode3ObjectHigh, _) => Some(Self::High),
                _ => None,
            },
        }
    }

    /// The netlist's sprite fetch in the phase.
    const fn net(self) -> NetSpriteFetch {
        let (sobu, suda, step, busy, tobu, vonu, seba, tyfo) = match self {
            Self::Pushed => (false, false, 4, true, true, true, true, false),
            Self::AfterPush => (false, false, SPRITE_DONE, false, false, true, true, true),
            Self::Attributes => (true, true, 0, true, false, false, false, false),
            Self::LowFirst => (false, false, 1, true, false, false, false, true),
            Self::Low => (false, false, 2, true, false, false, false, false),
            Self::High => (false, false, 3, true, true, false, false, true),
        };
        NetSpriteFetch {
            sobu,
            suda,
            step,
            busy,
            tobu,
            vonu,
            seba,
            tyfo,
        }
    }
}

/// The dots of the netlist's tile that its fine scroll has counted: its
/// count is 6 minus the pixels left (mod 8).
pub(super) const fn fine_of(len: u8) -> u8 {
    FINE_AHEAD.wrapping_sub(len) & 7
}

/// The netlist's pixels: ceres's position and FIFO length, `extra` pixels
/// on (a tile's pixels further when the FIFO ran out).
pub(super) const fn shifted(bg: &Background, extra: u8) -> (i16, u8) {
    let len = bg.fifo.len() as i16 - extra as i16;
    let len = if len <= 0 { len + FULL as i16 } else { len };
    #[expect(
        clippy::cast_sign_loss,
        clippy::cast_possible_truncation,
        reason = "1 to 8"
    )]
    (signed(bg.position) + extra as i16, len as u8)
}

/// The netlist's pipeline after ceres's dot and its two edges, with its SCX,
/// its sprite store and its inputs as `net` has them: its pixels `extra`
/// on, its fetcher `age` dots into its tile, and its sprite fetch where
/// ceres's object fetch has it (its steps locked to ceres's reads, two dots
/// after ceres's push the slot fetched).
pub(crate) const fn expected(l: &Mode3Loop, net: &NetPipeline, g: Ghost) -> NetPipeline {
    match window_phase(l, g) {
        WindowPhase::Starting { dot, fine } => {
            return window_starting(l, net, dot, fine, g.rerun);
        }
        WindowPhase::Inside => return window_inside(l, net),
        WindowPhase::Off | WindowPhase::Armed => {}
    }
    let (p, len) = shifted(&l.pace.bg, g.extra);
    let first = (p + len as i16 <= 0) & !g.rerun;
    let mut e = pipeline_at(p, len, first, net);
    if !first {
        e.fetcher = NetFetcher {
            started: e.fetcher.started,
            ..fetcher_for(g.age)
        };
    }
    if g.frozen {
        // RENE takes RYFA at each clock, and RYFA holds.
        e.rene = e.ryfa;
    }
    if let Some(phase) = SpritePhase::of(g.since_push, l.pace.state, l.pace.wait) {
        e.sprite = phase.net();
    }
    e
}

/// The FIFO's length at which the netlist's fine scroll counts `fine`.
pub(super) const fn len_at(fine: u8) -> u8 {
    let len = FINE_AHEAD.wrapping_sub(fine) & 7;
    if len == 0 { FULL } else { len }
}

/// The netlist's pipeline with no sprite fetched: the fetch's steps at
/// their last, its SCX, sprite store and inputs as `net` has them.
pub(super) const fn at_rest(net: &NetPipeline) -> NetPipeline {
    NetPipeline {
        sprite: NetSpriteFetch {
            sobu: false,
            suda: false,
            step: SPRITE_DONE,
            busy: true,
            tobu: false,
            vonu: false,
            seba: false,
            tyfo: true,
        },
        ..*net
    }
}

/// The netlist's pipeline at the end of the dot before ceres's loop starts
/// (`h = 162` of a visible line): mode 3 rose at `h = 154`, the first
/// fetch's count is at 4, the fine scroll discards. Measured: the same on
/// every SCX and whatever sprites the scan found (`mapping-mode3-start`).
pub(crate) const fn before_start(net: &NetPipeline) -> NetPipeline {
    NetPipeline {
        mode3: true,
        voga: false,
        px: 0,
        fetcher: NetFetcher {
            count: 4,
            lyzu: true,
            nyka: false,
            pory: false,
            pygo: false,
            started: false,
        },
        fine: 0,
        puxa: false,
        nyze: false,
        discard: true,
        ryfa: false,
        rene: false,
        ..at_rest(net)
    }
}

pub(super) const fn matches_step(a: FetcherStep, b: FetcherStep) -> bool {
    a as u8 == b as u8
}
