//! Mode 3's pipeline against the netlist's (`netlist_proofs.rs`).
//!
//! Ceres's mode 3 loop runs a dot's work in the dot's first half dot (odd
//! `h`); the netlist's pipeline (`ceres_netlist::render_edge`) takes its two
//! edges, the clock rising at odd `h` and falling at even `h`. After both,
//! the netlist's background is a function of ceres's (`pipeline_at`): its
//! fetcher's count, its first fetch's flip-flops and its fine scroll follow
//! from the FIFO's length, which counts the dots of each tile; the pixel
//! counter, and the fine scroll's discard, follow from the position. Neither
//! depends on SCX: ceres's lead-in compares SCX's low bits with the position
//! two dots before the netlist's fine scroll compares them with its count,
//! and spends the two dots after its match at positions -7 and -6, which
//! the netlist spends discarding.
//!
//! SCX can change between the two compares. Ceres's mixer takes a write a
//! dot before the netlist's latch does (`mapping-scx-write`), so each of the
//! netlist's compares uses the SCX ceres's compared with: after ceres's dot,
//! the latch holds the mixer's SCX of the dot before, and takes this dot's in
//! the next. The relation keeps the two compares the netlist still has to
//! make, and that they come out as ceres's did (`reachable`).
//!
//! Objects shift the netlist's pixels against ceres's: ceres stops for one
//! as its position plus 8 reaches its X, the netlist's pixel counter (6
//! ahead) two dots later; the netlist's pixels then stand still while its
//! fetcher counts on, and go on two dots after ceres's push. `Ghost` keeps
//! what neither side holds of that, `Ghost::after` follows it from ceres's
//! dot, and `expected` is the netlist's pipeline from both: the background
//! at the position plus the pixels the netlist drew past, its fetcher at its
//! age, and its sprite fetch locked to ceres's reads. The rules were fitted
//! on joined dumps of both (35.5 million dots of 103 ROMs) before Kani
//! checked them.
//!
//! Ceres's side is its mode 3 loop itself (`mode3::mode3`), on the display's
//! state alone (`Mode3Loop`): the fetchers' reads, the object's row and the
//! pixels' mixing, which do not decide when the pixels come out, do nothing.
//! The window is off, armed, starting or started (`WindowPhase`,
//! `window_agrees`). Armed, ceres's check compares WX with its position plus
//! 7, the netlist's with its pixel counter (6 ahead) at the dot's rise, and
//! neither matches before ceres's check reaches WX. Then ceres clears its
//! FIFO and fetches the window's first tile for six dots, its position
//! held; the netlist draws one more pixel, holds its pixels for six dots as
//! its fetcher restarts and counts, and restarts its fine scroll with its
//! fetcher in the eighth dot (`window_starting`). From the tenth dot the
//! netlist's pipeline is the background's at ceres's position again, in the
//! window (`window_inside`). The ghost counts the dots and keeps the fine
//! scroll's count the start found. The started window's lines have the
//! objects off.
#![cfg_attr(
    not(kani),
    expect(
        dead_code,
        reason = "outside Kani, nothing runs the steps yet (Charon does not take them)"
    )
)]
#![expect(
    clippy::needless_bitwise_bool,
    reason = "written for Aeneas (Lean), as `timing::check` is: a lazy && or || is a branch, and its symbolic execution forks on every one"
)]
#![expect(
    clippy::option_if_let_else,
    clippy::manual_range_contains,
    reason = "Aeneas takes neither closures nor ranges"
)]

use {
    super::{Background, Popped},
    crate::ppu::{
        PX_WIDTH,
        display::{
            Display,
            fetcher::FetcherStep,
            fifo::Item,
            mode3::{Mode3Flow, Mode3Ppu, mode3},
            pos,
            state::State,
        },
    },
    ceres_netlist::render_edge::{EdgeInputs, NO_X, NetPipeline, SLOTS},
};

/// Ceres's mode 3 loop on the display's state alone: nothing read, no pixel
/// drawn.
pub(crate) struct Mode3Loop {
    pub d: Display,
    /// SCX as the pixel mixer sees it, LCDC's object and window enables, and
    /// WX.
    pub mixer_scx: u8,
    pub objects_enabled: bool,
    pub window_enabled: bool,
    pub wx: u8,
}

impl Mode3Ppu for Mode3Loop {
    fn d(&mut self) -> &mut Display {
        &mut self.d
    }

    fn objects_enabled(&self) -> bool {
        self.objects_enabled
    }

    fn mixer_scx(&self) -> u8 {
        self.mixer_scx
    }

    fn window_check(&mut self) -> bool {
        self.d
            .mode3_window(self.window_enabled, self.wx, self.mixer_scx, false)
    }

    fn window_activated(&mut self) {
        self.d.mode3_window_activated();
    }

    fn window_pixel_insertion(&mut self) {}

    fn fetcher_read(&mut self) {}

    fn fetcher_glitches(&self) -> (bool, Option<u8>) {
        (false, None)
    }

    fn read_object_attributes(&mut self) {}

    fn read_object_low(&mut self) {}

    fn read_object_high(&mut self) {}

    fn push_object(&mut self) {}

    fn mix_pixel(&mut self, _: Item, _: Popped) {}

    fn signal_mode3_end(&mut self) {}
}

/// A dot of ceres's mode 3: the sleep it is in passes a dot, and when it
/// ends the loop runs (`step_state_machine`); once the line is drawn,
/// `HBlankStart` runs a dot later (`mode3_done`'s sleep: the rest of it
/// starts the next line).
pub(crate) fn ceres_dot(l: &mut Mode3Loop) {
    if !l.d.wake() {
        return;
    }
    let state = l.d.state;
    if matches!(mode3(l, state), Mode3Flow::Done) {
        l.d.sleep_in_line(State::HBlankStart, 1);
    }
}

/// The loop's sleeps between two dots: a dot after its pixel, while an
/// object's fetch waits for the tile or reads (two dots for the attributes
/// and the low byte), or the line is drawn.
const fn between_dots(d: &Display) -> bool {
    match d.state {
        State::Mode3Pixel
        | State::Mode3ObjectWait
        | State::Mode3ObjectFetch
        | State::Mode3ObjectHigh
        | State::HBlankStart => d.wait == 1,
        State::Mode3ObjectAttributes | State::Mode3ObjectLow => (d.wait == 1) | (d.wait == 2),
        _ => false,
    }
}

/// The line is drawn: `HBlankStart` runs in the next dot.
const fn drawn(d: &Display) -> bool {
    matches!(d.state, State::HBlankStart)
}

/// SCX's and WX's addresses.
const SCX: u16 = 0xFF43;
const WX: u16 = 0xFF4B;
/// LCDC's object and window enables.
const LCDC_OBJ_EN: u8 = 2;
const LCDC_WIN_EN: u8 = 0x20;
/// The objects a line holds; the X of the first one the proofs take (those
/// before match in the lead-in), and of the last the netlist fetches.
const MAX_OBJECTS: usize = 10;
const FIRST_OBJECT_X: u8 = 8;
const LAST_OBJECT_X: u8 = 167;
/// The sprite fetch's last step.
const SPRITE_DONE: u8 = 5;
/// The FIFO's length when it is full.
const FULL: u8 = 8;
/// The lead-in's first position, and the last one at which it can still
/// match SCX.
const LEAD_IN: i16 = -16;
const LAST_UNMATCHED: i16 = -9;
/// The positions the netlist spends discarding after ceres's match, and the
/// one at which its fine scroll matches.
const LAST_DISCARDED: i16 = -6;
const NET_MATCH: i16 = -5;
/// The pixel counter is ahead of ceres's position by this, once it counts.
const PX_AHEAD: i16 = 6;
/// The netlist's fine scroll is this minus the FIFO's length (mod 8), and its
/// fetcher's count stops at 5.
const FINE_AHEAD: u8 = 6;
const FETCH_DONE: u8 = 5;

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
const fn first_round(bg: &Background) -> bool {
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
        fetch_count,
        lyzu,
        nyka,
        pory,
        pygo,
        started,
        fine,
        puxa: p == NET_MATCH,
        nyze: p == NET_MATCH,
        discard: p <= LAST_DISCARDED,
        ryfa,
        rene,
        busy,
        ..at_rest(net)
    }
}

/// The netlist's fetcher (its count, LYZU, NYKA, PORY, PYGO) `age` dots
/// after it restarted for a tile (7: seven or more): it counts to 5, and
/// NYKA, PORY and PYGO follow it a clock apart.
const fn fetcher_for(age: u8) -> (u8, bool, bool, bool, bool) {
    let before = (age + 7) & 7;
    let waits = (age >= 6) | (age == 0);
    (
        if age < FETCH_DONE { age } else { FETCH_DONE },
        (if before < FETCH_DONE {
            before
        } else {
            FETCH_DONE
        }) & 1
            != 0,
        waits,
        waits,
        (age == 7) | (age <= 1),
    )
}

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
    pub(crate) const fn of(d: &Display) -> Self {
        Self {
            state: d.state,
            wait: d.wait,
            position: d.bg.position,
            len: d.bg.fifo.len(),
            window_started: d.window.wx_triggered,
        }
    }
}

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

/// The dots of the netlist's tile that its fine scroll has counted: its
/// count is 6 minus the pixels left (mod 8).
const fn fine_of(len: u8) -> u8 {
    FINE_AHEAD.wrapping_sub(len) & 7
}

/// The netlist's pixels: ceres's position and FIFO length, `extra` pixels
/// on (a tile's pixels further when the FIFO ran out).
const fn shifted(bg: &Background, extra: u8) -> (i16, u8) {
    let len = bg.fifo.len() as i16 - extra as i16;
    let len = if len <= 0 { len + FULL as i16 } else { len };
    #[expect(
        clippy::cast_sign_loss,
        clippy::cast_possible_truncation,
        reason = "1 to 8"
    )]
    (signed(bg.position) + extra as i16, len as u8)
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
    /// the dots since, and the rest as with nothing fetched (the objects are
    /// off there).
    pub(crate) fn after(self, before: Before, d: &Display) -> Self {
        let (state, wait, position) = (before.state, before.wait, before.position);
        let fired = !before.window_started & d.window.wx_triggered;
        let window = if fired {
            1
        } else if (self.window >= 1) & (self.window < INSIDE) {
            self.window + 1
        } else {
            self.window
        };
        if window != 0 {
            return Self {
                rerun: self.rerun,
                window,
                window_fine: if fired {
                    (fine_of(before.len) + 1) & 7
                } else {
                    self.window_fine
                },
                ..Self::at_rest(&d.bg)
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
        let popped = d.bg.position != position;
        let frozen = (since_push <= 1) | ((self.extra == 2) & fetching(d));
        let extra = (self.extra + u8::from(!frozen)).wrapping_sub(u8::from(popped));
        let rerun = self.rerun | ((position == pos(-9)) & (d.bg.position == pos(-16)));
        let (p, len) = shifted(&d.bg, extra);
        let fine = fine_of(len);
        let age = if (p + i16::from(len) <= 0) & !rerun {
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
            window: 0,
            window_fine: 0,
        }
    }
}

/// Ceres's loop is fetching an object: it waits for the tile, or reads.
const fn fetching(d: &Display) -> bool {
    matches!(
        d.state,
        State::Mode3ObjectWait
            | State::Mode3ObjectFetch
            | State::Mode3ObjectAttributes
            | State::Mode3ObjectLow
            | State::Mode3ObjectHigh
    )
}

/// The netlist's pipeline after ceres's dot and its two edges, with its SCX,
/// its sprite store and its inputs as `net` has them: its pixels `extra`
/// on, its fetcher `age` dots into its tile, and its sprite fetch where
/// ceres's object fetch has it (its steps locked to ceres's reads, two dots
/// after ceres's push the slot fetched).
pub(crate) const fn expected(l: &Mode3Loop, net: &NetPipeline, g: Ghost) -> NetPipeline {
    match window_phase(l, g) {
        WindowPhase::Starting { dot, fine } => return window_starting(l, net, dot, fine),
        WindowPhase::Inside => return window_inside(l, net),
        WindowPhase::Off | WindowPhase::Armed => {}
    }
    let (p, len) = shifted(&l.d.bg, g.extra);
    let first = (p + len as i16 <= 0) & !g.rerun;
    let mut e = pipeline_at(p, len, first, net);
    if !first {
        (e.fetch_count, e.lyzu, e.nyka, e.pory, e.pygo) = fetcher_for(g.age);
    }
    if g.frozen {
        // RENE takes RYFA at each clock, and RYFA holds.
        e.rene = e.ryfa;
    }
    let (state, wait) = (l.d.state, l.d.wait);
    let sprite = match g.since_push {
        0 => Some((false, false, 4, true, true, true, true, false)),
        1 => Some((false, false, SPRITE_DONE, false, false, true, true, true)),
        _ => match (state, wait) {
            (State::Mode3ObjectAttributes, 1) => {
                Some((true, true, 0, true, false, false, false, false))
            }
            (State::Mode3ObjectLow, 2) => Some((false, false, 1, true, false, false, false, true)),
            (State::Mode3ObjectLow, _) => Some((false, false, 2, true, false, false, false, false)),
            (State::Mode3ObjectHigh, _) => Some((false, false, 3, true, true, false, false, true)),
            _ => None,
        },
    };
    if let Some(s) = sprite {
        (
            e.sobu,
            e.suda,
            e.sprite_step,
            e.busy,
            e.tobu,
            e.vonu,
            e.seba,
            e.tyfo,
        ) = s;
    }
    e
}

/// The FIFO's length at which the netlist's fine scroll counts `fine`.
const fn len_at(fine: u8) -> u8 {
    let len = FINE_AHEAD.wrapping_sub(fine) & 7;
    if len == 0 { FULL } else { len }
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
const fn window_starting(l: &Mode3Loop, net: &NetPipeline, dot: u8, fine: u8) -> NetPipeline {
    let d = &l.d;
    let position = signed(d.bg.position);
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
    if dot == 1 {
        pipeline_at(position + 1, len_at(fine), false, net)
    } else if dot == 2 {
        NetPipeline {
            px: l.wx.wrapping_add(1),
            fetch_count: 0,
            lyzu: (if fine < FETCH_DONE { fine } else { FETCH_DONE }) & 1 != 0,
            nyka: false,
            pory: false,
            pygo: (fine >= 6) | (fine == 0),
            fine: held,
            ryfa: false,
            rene: fine == 0,
            nopa: false,
            sovy: false,
            ..pipeline_at(position + 1, len_at(fine), false, &started)
        }
    } else if dot <= 7 {
        NetPipeline {
            px: l.wx.wrapping_add(1),
            fetch_count: dot - 2,
            lyzu: dot & 1 == 0,
            nyka: false,
            pory: false,
            pygo: false,
            fine: held,
            ryfa: false,
            rene: false,
            ..pipeline_at(position + 1, len_at(fine), false, &started)
        }
    } else if dot == 8 {
        NetPipeline {
            pygo: false,
            ryfa: false,
            rydy: false,
            win_x: 1,
            ..pipeline_at(position, d.bg.fifo.len(), false, &started)
        }
    } else {
        NetPipeline {
            rene: false,
            pyco: false,
            nunu: false,
            rydy: false,
            sovy: false,
            win_x: 1,
            ..pipeline_at(position, d.bg.fifo.len(), false, &started)
        }
    }
}

/// The netlist's pipeline ten dots or more after the window started, with
/// the objects off: the background's at ceres's position and FIFO length,
/// in the window (NOPA and its latch), its column a tile behind ceres's
/// from ceres's read of a tile's last byte to the netlist's restart for it.
const fn window_inside(l: &Mode3Loop, net: &NetPipeline) -> NetPipeline {
    let d = &l.d;
    let inside = pipeline_at(signed(d.bg.position), d.bg.fifo.len(), false, net);
    NetPipeline {
        pyco: false,
        nunu: false,
        nopa: true,
        in_window: true,
        rydy: false,
        sovy: false,
        win_x: d.window.tile_x.wrapping_sub((inside.fine >= 4) as u8) & WIN_X_MASK,
        ..inside
    }
}

/// The netlist's pipeline with no sprite fetched: the fetch's steps at
/// their last, its SCX, sprite store and inputs as `net` has them.
const fn at_rest(net: &NetPipeline) -> NetPipeline {
    NetPipeline {
        sobu: false,
        suda: false,
        sprite_step: SPRITE_DONE,
        busy: true,
        tobu: false,
        vonu: false,
        seba: false,
        tyfo: true,
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
        fetch_count: 4,
        lyzu: true,
        nyka: false,
        pory: false,
        pygo: false,
        started: false,
        fine: 0,
        puxa: false,
        nyze: false,
        discard: true,
        ryfa: false,
        rene: false,
        ..at_rest(net)
    }
}

/// Ceres's first dot of the loop (`Mode3Start`): mode 3 starts
/// (`mode3_start`, the background's part), then the loop.
pub(crate) fn ceres_first_dot(l: &mut Mode3Loop) {
    l.d.bg.start();
    _ = mode3(l, State::Mode3);
}

/// The first dot from the netlist's state before it, and ceres's after the
/// line before (its position back at the lead-in's start, and no blank pixel
/// inserted: the window was idle), with the mixer's SCX in the dot before
/// (`last_scx`) and in this one: both related.
pub(crate) fn start_agrees(
    l: &mut Mode3Loop,
    net: &mut NetPipeline,
    last_scx: u8,
    rising: EdgeInputs,
    falling: EdgeInputs,
) -> bool {
    let before = !net.inputs.clk
        & !l.window_enabled
        & quiet(&net.inputs, l)
        & latch_holds(net)
        & (*net == before_start(net))
        & (l.d.bg.position == pos(-16))
        & !l.d.bg.insert_blank
        & store_holds(net)
        & window_agrees(l, net, Ghost::at_rest(&l.d.bg))
        & starting_objects(l, net);
    if !(before
        & rising.clk
        & !falling.clk
        & quiet(&rising, l)
        & quiet(&falling, l)
        & writes(net, &rising, &falling, last_scx))
    {
        return true;
    }
    let snapshot = Before::of(&l.d);
    ceres_first_dot(l);
    net.edge(rising);
    net.edge(falling);
    let rest = Ghost {
        age: 0,
        ..Ghost::at_rest(&l.d.bg)
    };
    related(l, net, rest.after(snapshot, &l.d))
}

/// As the line's loop starts, ceres's objects and the netlist's store hold
/// the same sprites (the OAM scan's proofs), none fetched, at X 8 or more.
fn starting_objects(l: &Mode3Loop, net: &NetPipeline) -> bool {
    let rest = Ghost::at_rest(&l.d.bg);
    (net.fetched == 0)
        & !l.d.obj_fetch.aborted
        & objects_ok(l, rest)
        & (!l.objects_enabled | objects_agree(l, net, rest))
}

/// The shapes a line with the window idle passes through, with the mixer's
/// SCX in the last dot (`mixer_scx`) and the dot before (`latched`, which
/// the netlist's latch holds), and the ghost.
fn reachable(l: &Mode3Loop, latched: u8, g: Ghost) -> bool {
    match window_phase(l, g) {
        WindowPhase::Starting { dot, fine } => return window_starting_ok(l, g, dot, fine),
        WindowPhase::Inside => return window_inside_ok(l, g),
        WindowPhase::Off | WindowPhase::Armed => {}
    }
    let bg = &l.d.bg;
    let p = signed(bg.position);
    let len = bg.fifo.len();
    let shapes = (len >= 1)
        & (len <= FULL)
        & fetcher_ok(&l.d, g)
        & !bg.insert_blank
        & (p >= LEAD_IN)
        & (p <= i16::from(PX_WIDTH))
        & (g.since_push <= 2)
        & (g.extra <= 2)
        & (g.age <= 7);
    let round = if p <= LAST_UNMATCHED {
        // Unmatched: the position has gained a pixel for each one the FIFO
        // lost since the round began, at -16 with the FIFO full.
        first_round(bg)
            & ((p > LEAD_IN) | g.rerun)
            & (p + i16::from(len) == LEAD_IN + i16::from(FULL))
    } else {
        // Matched: the position jumped to -8 and gained a pixel.
        !first_round(bg) | (p > -8)
    };
    // No object before the first pixel: the ghost rests in the lead-in.
    let lead_in = (p >= 0)
        | (g == Ghost {
            rerun: g.rerun,
            ..Ghost::at_rest(bg)
        });
    // The netlist's fetcher only runs ahead of its fine scroll's count.
    let (net_p, net_len) = shifted(bg, g.extra);
    let fetcher_ahead = ((net_p + i16::from(net_len) <= 0) & !g.rerun)
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
    shapes
        & round
        & lead_in
        & fetcher_ahead
        & ghost_ok(&l.d, g)
        & fetch_age_ok(&l.d, g)
        & objects_ok(l, g)
        & pending_compares(bg, latched, l.mixer_scx, g.rerun)
}

/// The ghost with nothing fetched, as the window's phases keep it.
fn ghost_rests(l: &Mode3Loop, g: Ghost) -> bool {
    g == Ghost {
        rerun: g.rerun,
        window: g.window,
        window_fine: g.window_fine,
        ..Ghost::at_rest(&l.d.bg)
    }
}

/// Ceres's loop `dot` dots (1 to 9) after its window check fired: the FIFO
/// cleared and the position held at WX's while the fetcher fetches the
/// window's first tile (five dots from its second step), the row pushed in
/// the sixth, and a pixel a dot from the seventh, the first of which ends
/// `being_fetched`.
fn window_starting_ok(l: &Mode3Loop, g: Ghost, dot: u8, fine: u8) -> bool {
    let d = &l.d;
    let bg = &d.bg;
    let p = signed(bg.position);
    let len = bg.fifo.len();
    let wx = i16::from(l.wx);
    let shape = if dot <= 5 {
        (len == 0)
            & (bg.fetcher.step as u8 == dot)
            & (p + WX_AHEAD == wx)
            & d.window.being_fetched
            & (d.window.tile_x == 0)
    } else {
        (len == 14 - dot)
            & matches_step(bg.fetcher.step, step_for(len))
            & (p + 13 - i16::from(dot) == wx)
            & (d.window.being_fetched == (dot == 6))
            & (d.window.tile_x == 1)
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
fn window_inside_ok(l: &Mode3Loop, g: Ghost) -> bool {
    let d = &l.d;
    let bg = &d.bg;
    let p = signed(bg.position);
    let len = bg.fifo.len();
    (len >= 1)
        & (len <= FULL)
        & matches_step(bg.fetcher.step, step_for(len))
        & (p + 3 >= i16::from(l.wx))
        & (p <= i16::from(PX_WIDTH))
        & !d.window.being_fetched
        & (d.window.tile_x <= WIN_X_MASK)
        & !bg.insert_blank
        & ghost_rests(l, g)
        & objects_ok(l, g)
}

/// The ghost against ceres's phase: the netlist's pixels stand still from
/// the third dot of an object's fetch (two pixels past ceres's) to the dot
/// after ceres's push, and draw on, a pixel behind, as ceres's do.
const fn ghost_ok(d: &Display, g: Ghost) -> bool {
    match g.since_push {
        // Ceres's push: it drew a pixel, or fetches the next object at the
        // same X.
        0 => {
            g.frozen
                & if fetching(d) {
                    // The next object at the same X: its tile is there.
                    matches!(d.state, State::Mode3ObjectFetch) & (g.extra == 2)
                } else {
                    g.extra == 1
                }
        }
        1 => {
            g.frozen
                & if fetching(d) {
                    // Its attributes, or ceres drew a pixel and stopped at the
                    // next X.
                    (matches!(d.state, State::Mode3ObjectAttributes)
                        & (d.wait == 2)
                        & (g.extra == 2))
                        | (matches!(d.state, State::Mode3ObjectWait | State::Mode3ObjectFetch)
                            & (g.extra == 1))
                } else {
                    g.extra == 0
                }
        }
        _ => {
            let reads = matches!(
                d.state,
                State::Mode3ObjectAttributes | State::Mode3ObjectLow | State::Mode3ObjectHigh
            );
            let second = !matches!(d.state, State::Mode3ObjectAttributes) | (d.wait == 1);
            if reads {
                // The reads begin a dot after the stop at the earliest: the
                // netlist drew its two pixels, and stands still from the
                // attributes' second dot.
                (g.extra == 2) & (!second | g.frozen)
            } else if fetching(d) {
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
const fn fetch_age_ok(d: &Display, g: Ghost) -> bool {
    let (_, net_len) = shifted(&d.bg, g.extra);
    let fine = fine_of(net_len);
    if matches!(
        d.state,
        State::Mode3ObjectFetch
            | State::Mode3ObjectAttributes
            | State::Mode3ObjectLow
            | State::Mode3ObjectHigh
    ) & !matches_step(d.bg.fetcher.step, FetcherStep::Push)
    {
        // The fetch began with the fetcher at its high byte's second dot or
        // further, a tile in the FIFO, and stepped it to its push.
        return false;
    }
    if matches!(d.state, State::Mode3ObjectFetch)
        & (g.since_push == 2)
        & (g.extra == 2)
        & (net_len > d.bg.fifo.len())
    {
        // Two pixels or fewer left: ceres's fetcher waited at its push, and
        // the fetch began in the dot of the stop.
        return false;
    }
    if matches!(d.state, State::Mode3ObjectWait | State::Mode3ObjectFetch)
        & (g.since_push == 2)
        & (g.extra == 2)
    {
        // The fetch waited for the tile, so ceres's fetcher was not at its
        // push when it stopped: the netlist's runs two steps behind it, and
        // the fetch began as ceres's reached its high byte's second dot.
        let step = if matches!(d.state, State::Mode3ObjectFetch) {
            FetcherStep::DataHighT2 as u8 + 1
        } else {
            d.bg.fetcher.step as u8
        };
        return g.age == step.wrapping_sub(2);
    }
    if fetching(d) & (g.since_push <= 1) & (g.extra == 2) {
        // The next object at the same X: the netlist's fetcher was done
        // long before.
        return (g.age == 7) | ((net_len > d.bg.fifo.len()) & (g.age == fine));
    }
    if matches!(d.state, State::Mode3ObjectAttributes)
        & (d.wait == 2)
        & (g.since_push == 2)
        & !g.frozen
        & (net_len <= d.bg.fifo.len())
    {
        // The fetch began in the dot of the stop, ceres's fetcher at its high
        // byte's second dot or further (the netlist's at 3 or more), and the
        // netlist drew on for two dots.
        return g.age >= 5;
    }
    if !(fetching(d) & g.frozen & (g.since_push == 2)) {
        return true;
    }
    if net_len > d.bg.fifo.len() {
        return (g.age == fine) | (g.age == 7);
    }
    let phase = match d.state {
        State::Mode3ObjectWait => (d.bg.fetcher.step as u8).wrapping_sub(2),
        State::Mode3ObjectFetch => 4,
        State::Mode3ObjectAttributes => {
            if d.wait == 2 {
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
const fn fetcher_ok(d: &Display, g: Ghost) -> bool {
    let bg = &d.bg;
    let len = bg.fifo.len();
    let step = bg.fetcher.step;
    let (_, net_len) = shifted(bg, g.extra);
    // Ceres pushed its row a dot or two ago, before the netlist's pixels
    // reach the tile and its fetcher restarts for it.
    let fresh = matches_step(step, step_for(len)) & (len >= FULL - 1) & (net_len >= FULL - 1);
    if first_round(bg) & !g.rerun {
        matches_step(step, step_for(len))
    } else if !fetching(d) & (len == FULL) {
        // A full FIFO after a pixel: its row was pushed in this dot.
        matches_step(step, FetcherStep::GetTileT1)
    } else if net_len > len {
        // The netlist's pixels went on into the next tile: ceres's fetcher
        // waits to push it.
        matches_step(step, FetcherStep::Push)
    } else if g.age == fine_of(net_len) {
        // While ceres stops for an object its fetcher runs on with the
        // netlist's pixels; otherwise it keeps to its own FIFO.
        matches_step(step, step_for(if fetching(d) { net_len } else { len }))
    } else if g.age <= 3 {
        step as u8 == g.age + 2
    } else {
        matches_step(step, FetcherStep::Push) | fresh
    }
}

/// Ceres's objects: left to draw from the nearest, all at X 8 or more (the
/// lead-in's are left out), none left behind the position; while it fetches
/// one, it is the nearest, at the position's X.
fn objects_ok(l: &Mode3Loop, g: Ghost) -> bool {
    let objs = &l.d.objs;
    let at = l.d.bg.object_match_x();
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
    let (_, net_len) = shifted(&l.d.bg, g.extra);
    let quiet_line = l.objects_enabled
        | ((g.since_push == 2) & (g.extra == 0) & !g.frozen & (g.age == fine_of(net_len)));
    // The fetch waits for the tile while the fetcher has not reached its
    // high byte's second dot, a step a dot (or the FIFO ran out and got its
    // row).
    let step = l.d.bg.fetcher.step as u8;
    let waits = !matches!(l.d.state, State::Mode3ObjectWait)
        | (step <= FetcherStep::DataHighT2 as u8)
        | ((l.d.bg.fifo.len() == FULL) & matches_step(l.d.bg.fetcher.step, FetcherStep::GetTileT1));
    ok & ahead & (!fetching(&l.d) | fetched) & quiet_line & waits
}

/// The netlist's sprite store against ceres's objects: at each X up to 167,
/// as many slots not fetched yet whose X its pixel counter has not passed as
/// ceres has objects left (and the one ceres pushed in this dot, which the
/// netlist fetches a dot later).
fn objects_agree(l: &Mode3Loop, net: &NetPipeline, g: Ghost) -> bool {
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
    let objs = &l.d.objs;
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
const fn net_object(net: &NetPipeline, slot: usize) -> bool {
    (net.fetched & (1 << slot) == 0)
        & (net.xs[slot] != NO_X)
        & (net.xs[slot] >= net.px)
        & (net.xs[slot] <= LAST_OBJECT_X)
}

const fn net_objects_at(net: &NetPipeline, x: u8) -> u8 {
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

fn objects_at(l: &Mode3Loop, net: &NetPipeline, g: Ghost, at: u8) -> u8 {
    let objs = &l.d.objs;
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

/// The netlist's fine scroll still has to compare the positions ceres's
/// last two dots compared, the one before with `latched` (its count is then
/// 6 minus the FIFO's length), the last with `mixer_scx`: each matches if
/// and only if ceres's did. Its first fetch not done (the FIFO's first pixel
/// popped, `first_round`), its next dot does not compare.
const fn pending_compares(bg: &Background, latched: u8, mixer_scx: u8, rerun: bool) -> bool {
    let p = signed(bg.position);
    let len = bg.fifo.len();
    let count = FINE_AHEAD.wrapping_sub(len) & 7;
    let compares = !(first_round(bg) & !rerun & (len == FULL - 1));
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

const fn matches_step(a: FetcherStep, b: FetcherStep) -> bool {
    a as u8 == b as u8
}

/// The netlist's inputs in mode 3: no reset, no line start, the scan
/// stopped (CENO low), the objects and the window on or off as ceres has
/// them; with the window on, not in VBlank and WX not written.
const fn quiet(i: &EdgeInputs, l: &Mode3Loop) -> bool {
    !i.ppu_reset
        & !i.hard_reset
        & !i.line_start
        & !i.stop_scan
        & !i.ceno
        & ((i.lcdc & LCDC_OBJ_EN != 0) == l.objects_enabled)
        & ((i.lcdc & LCDC_WIN_EN != 0) == l.window_enabled)
        & (!l.window_enabled | !i.mode1 & !(i.cpu_wr_sync & (i.addr == WX)))
}

/// The write strobe does not latch WX in an edge: the bus changes its
/// address a few nanoseconds after the strobe, so the strobe meets the
/// address of the edge before too.
const fn holds_wx(before: &EdgeInputs, now: &EdgeInputs) -> bool {
    !now.cpu_wr_sync | ((before.addr != WX) & (now.addr != WX))
}

/// The netlist's window at rest: no WY match in the frame, not in it, its
/// first tile not wanted.
const fn net_window_off(net: &NetPipeline) -> bool {
    !net.sary & !net.wy_latch & net_window_waits(net)
}

/// The netlist's window not started on this line: no WX match, not in it,
/// its first tile not wanted, its column at 0.
const fn net_window_waits(net: &NetPipeline) -> bool {
    !net.pyco & !net.nunu & !net.nopa & !net.in_window & !net.rydy & !net.sovy & (net.win_x == 0)
}

/// The WX the armed window takes: it starts at a pixel of the line, past
/// the lead-in.
const FIRST_WX: u8 = 7;
const LAST_WX: u8 = 165;
/// Ceres's window check compares WX with its position plus this.
const WX_AHEAD: i16 = 7;
/// The window's column counter's bits.
const WIN_X_MASK: u8 = 0x1F;

/// Ceres's window with no write to it landing: WY's match in the frame and
/// its start on this line as the phase has them (`being_fetched` only once
/// it started: `reachable` says when).
const fn ceres_window_waits(l: &Mode3Loop, wy_triggered: bool, started: bool) -> bool {
    let w = &l.d.window;
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
const fn window_unstarted(g: Ghost) -> bool {
    (g.window == 0) & (g.window_fine == 0)
}

/// The window on both sides: off (LCDC's bit 5 clear, and no WY match in
/// the frame), or on, WY matched in the frame (ceres's `wy_triggered`, the
/// netlist's REJO) and WX held between `FIRST_WX` and `LAST_WX`: armed, the
/// netlist's window waiting and ceres's check not past WX (at it, the
/// window starts in the next dot), or started, with the objects off.
const fn window_agrees(l: &Mode3Loop, net: &NetPipeline, g: Ghost) -> bool {
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
                & (signed(l.d.bg.position) + WX_AHEAD <= l.wx as i16)
        }
        // The netlist's window is `expected`'s there.
        WindowPhase::Starting { .. } | WindowPhase::Inside => {
            ceres_window_waits(l, true, true) & wx_held & !l.objects_enabled
        }
    }
}

/// The CPU strobes SCX's address in this edge.
const fn strobes_scx(i: &EdgeInputs) -> bool {
    i.cpu_wr_sync & (i.addr == SCX)
}

/// A write strobe that rises while the bus is on SCX's address keeps it there
/// (the edge before the strobe carries SCX's address, and 0xFF).
const fn keeps_address(before: &EdgeInputs, now: &EdgeInputs) -> bool {
    !(now.cpu_wr_sync & !before.cpu_wr_sync & (before.addr == SCX)) | (now.addr == SCX)
}

/// The write strobe does not rise with the dot's first edge where the
/// netlist's dot clock starts (POKY rises with PYGO, in ceres's third dot of
/// the line, at h = 167): the strobes rise at h = 1 (mod 8).
const fn strobe_misses_start(net: &NetPipeline, rising: &EdgeInputs) -> bool {
    !(net.mode3 & net.pory & !net.started & rising.cpu_wr_sync & !net.inputs.cpu_wr_sync)
}

/// The CPU writes SCX's low bits with `scx`'s in a dot's edges, if at all:
/// while it strobes SCX's address the data bus carries them, and if they are
/// not the latch's, the strobe comes with the dot's first edge.
const fn writes(net: &NetPipeline, rising: &EdgeInputs, falling: &EdgeInputs, scx: u8) -> bool {
    strobe_misses_start(net, rising)
        & (!strobes_scx(rising) | (rising.data & 7 == scx & 7))
        & (!strobes_scx(falling) | (falling.data & 7 == scx & 7))
        & keeps_address(&net.inputs, rising)
        & keeps_address(rising, falling)
        & ((net.scx & 7 == scx & 7) | strobes_scx(rising))
}

/// SCX's latch holds what the last edge strobed into it.
const fn latch_holds(net: &NetPipeline) -> bool {
    !strobes_scx(&net.inputs) | (net.inputs.data == net.scx)
}

/// Ceres's loop and the netlist's pipeline after a dot (at even `h`), with
/// the mixer's SCX in the dot and the ghost.
pub(crate) fn related(l: &Mode3Loop, net: &NetPipeline, g: Ghost) -> bool {
    related_but_pipeline(l, net, g) && (*net == expected(l, net, g))
}

/// `related` but for the pipeline's own flip-flops, which `expected` gives
/// from ceres's loop, the ghost and what it carries over from `net` (the
/// latches, the sprite store, the window's flip-flops, the inputs).
pub(crate) fn related_but_pipeline(l: &Mode3Loop, net: &NetPipeline, g: Ghost) -> bool {
    !net.inputs.clk
        & quiet(&net.inputs, l)
        & latch_holds(net)
        & store_holds(net)
        & window_agrees(l, net, g)
        & between_dots(&l.d)
        & (drawn(&l.d) == (l.d.bg.position == PX_WIDTH))
        & !l.d.obj_fetch.aborted
        & reachable(l, net.scx, g)
        & objects_agree(l, net, g)
}

/// The sprite store's fetched slots hold no X.
const fn store_holds(net: &NetPipeline) -> bool {
    let mut holds = true;
    let mut slot = 0;
    while slot < SLOTS {
        holds &= (net.fetched & (1 << slot) == 0) | (net.xs[slot] == NO_X);
        slot += 1;
    }
    holds
}

/// A dot from related states (the caller's to check), with the mixer's SCX
/// in this one (the loop has the last dot's): both stay related, and mode 3
/// goes on; or, once ceres has drawn the line (its `HBlankStart` runs in
/// this dot's first half dot), the netlist's mode 3 stops at the dot's
/// second edge.
pub(crate) fn step_agrees(
    l: &mut Mode3Loop,
    net: &mut NetPipeline,
    g: Ghost,
    mixer_scx: u8,
    rising: EdgeInputs,
    falling: EdgeInputs,
) -> bool {
    if !(rising.clk
        & !falling.clk
        & quiet(&rising, l)
        & quiet(&falling, l)
        & writes(net, &rising, &falling, l.mixer_scx)
        & (!l.window_enabled | holds_wx(&net.inputs, &rising) & holds_wx(&rising, &falling)))
    {
        return true;
    }
    let drawn = drawn(&l.d);
    let before = Before::of(&l.d);
    l.mixer_scx = mixer_scx;
    if !drawn {
        ceres_dot(l);
    }
    net.edge(rising);
    let stopped_early = net.stop_mode3();
    net.edge(falling);
    if drawn {
        !stopped_early & net.stop_mode3()
    } else {
        !stopped_early & !net.stop_mode3() & related(l, net, g.after(before, &l.d))
    }
}
