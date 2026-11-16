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
//! pace (`Mode3Loop`): the loop runs all of the window's logic itself (its
//! check, its start, its pixel insertion, the glitches its fetcher's step
//! takes). Its hooks cannot change the pace (`Mode3Ppu`): the fetcher's read
//! and the object's push leave any tile in the fetcher, which the loop
//! pushes onto the FIFO, and the object's reads and the pixels' mixing do
//! nothing.
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
            Pace,
            fetcher::{Fetcher, FetcherStep},
            fifo::Item,
            mode3::{Mode3Flow, Mode3Ppu, mode3},
            pos,
            state::State,
        },
    },
    ceres_netlist::render_edge::{
        EdgeInputs, NO_X, NetFetcher, NetPipeline, NetSpriteFetch, SLOTS,
    },
};

mod ghost;
mod objects;
mod pipeline;
mod window;
mod writes;

pub(crate) use {ghost::*, pipeline::*, window::*};
#[cfg_attr(
    not(test),
    expect(clippy::wildcard_imports, reason = "the check's parts share its items")
)]
use {objects::*, writes::*};

/// Ceres's mode 3 loop on the display's pace alone: the loop runs all of
/// the window's logic itself (its check, its start, its pixel insertion,
/// the glitches its fetcher's step takes); the hooks read the pace and
/// cannot change it (`Mode3Ppu`). Here the fetcher's read and the object's
/// push (the hooks that change what the fetcher holds) leave any tile in
/// it, for Kani, which the loop pushes onto the FIFO: the proofs hold for
/// whatever the PPU's reads find. The other hooks do nothing, and no pixel
/// is drawn.
pub(crate) struct Mode3Loop {
    pub pace: Pace,
    /// What the fetcher read.
    pub fetcher: Fetcher,
    /// SCX as the pixel mixer sees it, LCDC's object and window enables, and
    /// WX.
    pub mixer_scx: u8,
    pub objects_enabled: bool,
    pub window_enabled: bool,
    pub wx: u8,
}

impl Mode3Loop {
    /// Runs the loop from `resume`, its hooks beside the pace.
    pub(crate) fn run(&mut self, resume: State) -> Mode3Flow {
        let mut hooks = Hooks {
            fetcher: &mut self.fetcher,
            mixer_scx: self.mixer_scx,
            objects_enabled: self.objects_enabled,
            window_enabled: self.window_enabled,
            wx: self.wx,
        };
        mode3(&mut hooks, &mut self.pace, resume)
    }
}

/// The loop's hooks: its registers, and the fetcher, which they change.
struct Hooks<'a> {
    fetcher: &'a mut Fetcher,
    mixer_scx: u8,
    objects_enabled: bool,
    window_enabled: bool,
    wx: u8,
}

impl Mode3Ppu for Hooks<'_> {
    fn objects_enabled(&self) -> bool {
        self.objects_enabled
    }

    fn mixer_scx(&self) -> u8 {
        self.mixer_scx
    }

    fn window_enabled(&self) -> bool {
        self.window_enabled
    }

    fn wx(&self) -> u8 {
        self.wx
    }

    fn cgb_hardware(&self) -> bool {
        false
    }

    fn fetcher(&self) -> &Fetcher {
        self.fetcher
    }

    fn fetcher_read(&mut self, _: &Pace) {
        #[cfg(kani)]
        {
            *self.fetcher = kani::any();
        }
    }

    fn read_object_attributes(&mut self, _: &Pace) {}

    fn read_object_low(&mut self, _: &Pace) {}

    fn read_object_high(&mut self, _: &Pace) {}

    fn push_object(&mut self, _: &Pace) {
        #[cfg(kani)]
        {
            *self.fetcher = kani::any();
        }
    }

    fn mix_pixel(&mut self, _: Item, _: Popped) {}

    fn signal_mode3_end(&mut self) {}
}

/// A dot of ceres's mode 3: the sleep it is in passes a dot, and when it
/// ends the loop runs (`step_state_machine`); once the line is drawn,
/// `HBlankStart` runs a dot later (`mode3_done`'s sleep: the rest of it
/// starts the next line).
pub(crate) fn ceres_dot(l: &mut Mode3Loop) {
    if !l.pace.wake() {
        return;
    }
    let state = l.pace.state;
    if matches!(l.run(state), Mode3Flow::Done) {
        l.pace.sleep_in_line(State::HBlankStart, 1);
    }
}

/// The loop's sleeps between two dots: a dot after its pixel, while an
/// object's fetch waits for the tile or reads (two dots for the attributes
/// and the low byte), or the line is drawn.
const fn between_dots(pace: &Pace) -> bool {
    match pace.state {
        State::Mode3Pixel
        | State::Mode3ObjectWait
        | State::Mode3ObjectFetch
        | State::Mode3ObjectHigh
        | State::HBlankStart => pace.wait == 1,
        State::Mode3ObjectAttributes | State::Mode3ObjectLow => (pace.wait == 1) | (pace.wait == 2),
        _ => false,
    }
}

/// The line is drawn: `HBlankStart` runs in the next dot.
const fn drawn(pace: &Pace) -> bool {
    matches!(pace.state, State::HBlankStart)
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

/// Ceres's first dot of the loop (`Mode3Start`): mode 3 starts
/// (`mode3_start`, the background's part), then the loop.
pub(crate) fn ceres_first_dot(l: &mut Mode3Loop) {
    l.pace.bg.start();
    _ = l.run(State::Mode3);
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
        & (l.pace.bg.position == pos(-16))
        & !l.pace.bg.insert_blank
        & store_holds(net)
        & window_agrees(l, net, Ghost::at_rest(&l.pace.bg))
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
    let snapshot = Before::of(&l.pace);
    ceres_first_dot(l);
    net.edge(rising);
    net.edge(falling);
    let rest = Ghost {
        age: 0,
        ..Ghost::at_rest(&l.pace.bg)
    };
    related(l, net, rest.after(snapshot, &l.pace))
}

/// The WX the armed window takes. Below 2 the window starts in the lead-in,
/// and the two sides part in its start (WX 0 and 1, task 1.3); at 166 the
/// WX 166 glitch ends mode 3 early (`locks-with-wx-166-glitch`).
const FIRST_WX: u8 = 2;

const LAST_WX: u8 = 165;

/// Ceres's window check compares WX with its position plus this.
const WX_AHEAD: i16 = 7;

/// The window's column counter's bits.
const WIN_X_MASK: u8 = 0x1F;

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
        & between_dots(&l.pace)
        & (drawn(&l.pace) == (l.pace.bg.position == PX_WIDTH))
        & !l.pace.object_fetch_aborted
        & reachable(l, net.scx, g)
        & objects_agree(l, net, g)
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
    let drawn = drawn(&l.pace);
    let before = Before::of(&l.pace);
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
        !stopped_early & !net.stop_mode3() & related(l, net, g.after(before, &l.pace))
    }
}
