//! Render's background pipeline and sprite fetch, edge by edge, for
//! ceres-core's proofs.
//!
//! [`crate::render`] settles render delta cycle by delta cycle, as the
//! simulator does: it is the transcription, and the replay checks it
//! against the model's traces. Kani cannot take it: a single edge, all of it
//! concrete, did not settle in minutes (it takes this one, from any state,
//! in under a second). This is the part of it that decides where mode 3 ends:
//! the pixel counter, the fetcher's count, the first fetch's flip-flops, the
//! fine scroll, mode 3's latch, SCX, the sprite fetch (the sprite store's X,
//! its fetched slots, and the fetch's steps), and the window (WY and WX, its
//! matches, its counters, and its start). `tests/render_edge.rs` checks it
//! against `NetRender`, edge by edge, on `NetChip`'s runs of the test ROMs,
//! wherever the scan saves no sprite.
//!
//! An edge is a few nanoseconds of changes, and so is this. At 0 ns the
//! clock changes, and the clocks made from it (`dot`, `segu`, `clkpipe`,
//! `fetch_clock`) with it; each nanosecond after it applies the flip-flops'
//! changes clocked the nanosecond before (`tco` is 1 ns), takes the inputs
//! that arrive in it, and settles the latches and the asynchronous resets.
//! What a clock's edge in it clocks changes the next nanosecond. Within a
//! nanosecond the latches settle in the order they feed each other, once.
//! A sprite's X match reaches mode 3's stop 4 ns after it rises, and its
//! fall at once.

use crate::render::{NetRender, RenderInputs};

/// The pixel counter's count at which mode 3 stops.
const STOP_PX: u8 = 167;
/// The fetcher's count at which it is done (LAXU and NYVA high: 5).
const FETCH_DONE: u8 = 5;
/// The sprite fetch's last step.
const SPRITE_DONE: u8 = 5;
/// The sprite store's slots, and the X of an empty or fetched one.
pub const SLOTS: usize = 10;
pub const NO_X: u8 = 0xFF;
/// The nanoseconds after the edge in which something can still change: the
/// CPU's data bus arrives at 4 ns, and what it opens settles by 6; a match
/// that rises at 2 ns reaches mode 3's stop at 6.
const NS: u8 = 7;
/// The delay from a sprite's X match to mode 3's stop.
const MATCH_HELD_AFTER: u8 = 4;
const NEVER: u8 = u8::MAX;

/// When the inputs reach render, as `NetRender::edge_with_pins` takes them.
const INPUTS_AT: u8 = 1;
const SCAN_AT: u8 = 2;
const MODE1_AT: u8 = 3;
const DATA_AT: u8 = 4;

/// SCX's, WY's and WX's addresses.
const SCX: u16 = 0xFF43;
const WY: u16 = 0xFF4A;
const WX: u16 = 0xFF4B;
/// LCDC's window enable.
const LCDC_WIN_EN: u8 = 0x20;
/// The window's column counter's bits.
const WIN_X_MASK: u8 = 0x1F;
/// LCDC's sprite enable.
const LCDC_OBJ_EN: u8 = 2;

/// What the pipeline takes in: render's inputs, as an edge leaves them.
#[derive(Clone, Copy)]
#[expect(clippy::struct_excessive_bools, reason = "the inputs' levels")]
pub struct EdgeInputs {
    /// The PPU's clock.
    pub clk: bool,
    /// The LCD's reset and the chip's.
    pub ppu_reset: bool,
    pub hard_reset: bool,
    /// LCDC, LY, the line clock (hclk) and VBlank (mode 1).
    pub lcdc: u8,
    pub ly: u8,
    pub hclk: bool,
    pub mode1: bool,
    /// From the OAM scan: the line starts, the scan stops, CENO.
    pub line_start: bool,
    pub stop_scan: bool,
    pub ceno: bool,
    /// The CPU's write strobe, its address and the data bus.
    pub cpu_wr_sync: bool,
    pub addr: u16,
    pub data: u8,
}

impl EdgeInputs {
    /// Render's inputs that the pipeline takes.
    #[must_use]
    pub const fn of(i: &RenderInputs) -> Self {
        Self {
            clk: i.clk,
            ppu_reset: i.ppu_reset,
            hard_reset: i.hard_reset,
            lcdc: i.lcdc,
            ly: i.ly,
            hclk: i.hclk,
            mode1: i.mode1,
            line_start: i.line_start,
            stop_scan: i.stop_scan,
            ceno: i.ceno,
            cpu_wr_sync: i.cpu_wr_sync,
            addr: i.addr,
            data: i.data,
        }
    }
}

/// Render's background pipeline and sprite fetch: its flip-flops and
/// latches, and its inputs, as the last edge left them.
#[derive(Clone, Copy, PartialEq, Eq)]
#[expect(clippy::struct_excessive_bools, reason = "the netlist's flip-flops")]
pub struct NetPipeline {
    /// XYMU: mode 3. VOGA: its end, a clock later.
    pub mode3: bool,
    pub voga: bool,
    /// The pixel counter.
    pub px: u8,
    /// The fetcher's count, LYZU, and the first fetch's NYKA, PORY, PYGO
    /// and POKY (`started`).
    pub fetch_count: u8,
    pub lyzu: bool,
    pub nyka: bool,
    pub pory: bool,
    pub pygo: bool,
    pub started: bool,
    /// The fine scroll: its count, PUXA, NYZE, ROXY (`discard`), RYFA, RENE.
    pub fine: u8,
    pub puxa: bool,
    pub nyze: bool,
    pub discard: bool,
    pub ryfa: bool,
    pub rene: bool,
    /// SCX's, WY's and WX's latches.
    pub scx: u8,
    pub wy: u8,
    pub wx: u8,
    /// The window: WY matched (SARY, and the frame's latch REJO), WX matched
    /// (PYCO, NUNU), in it (PYNU, and NOPA a clock later), its column and
    /// line counters, its first tile not fetched yet (RYDY, and SOVY).
    pub sary: bool,
    pub wy_latch: bool,
    pub pyco: bool,
    pub nunu: bool,
    pub nopa: bool,
    pub in_window: bool,
    pub win_x: u8,
    pub win_y: u8,
    pub rydy: bool,
    pub sovy: bool,
    /// The sprite store: its slots' X (`NO_X` once fetched), the slots
    /// fetched (bit `i` for slot `i`).
    pub xs: [u8; SLOTS],
    pub fetched: u16,
    /// The sprite fetch: requested (SOBU, SUDA), its steps, TAKA (`busy`),
    /// and the steps' flip-flops TOBU, VONU, SEBA, TYFO.
    pub sobu: bool,
    pub suda: bool,
    pub sprite_step: u8,
    pub busy: bool,
    pub tobu: bool,
    pub vonu: bool,
    pub seba: bool,
    pub tyfo: bool,
    pub inputs: EdgeInputs,
}

impl PartialEq for EdgeInputs {
    fn eq(&self, other: &Self) -> bool {
        (self.clk == other.clk)
            & (self.ppu_reset == other.ppu_reset)
            & (self.hard_reset == other.hard_reset)
            & (self.lcdc == other.lcdc)
            & (self.ly == other.ly)
            & (self.hclk == other.hclk)
            & (self.mode1 == other.mode1)
            & (self.line_start == other.line_start)
            & (self.stop_scan == other.stop_scan)
            & (self.ceno == other.ceno)
            & (self.cpu_wr_sync == other.cpu_wr_sync)
            & (self.addr == other.addr)
            & (self.data == other.data)
    }
}

impl Eq for EdgeInputs {}

/// The pipeline's wires, from its flip-flops and latches and the inputs.
#[derive(Clone, Copy)]
#[expect(clippy::struct_excessive_bools, reason = "the netlist's gates")]
struct Wires {
    stop_mode3: bool,
    dot: bool,
    segu: bool,
    clkpipe: bool,
    fetch_done: bool,
    fetch_clock: bool,
    /// The slots whose X matches the pixel counter, the lowest of them, and
    /// whether any does.
    selected: u16,
    x_match: bool,
    /// The sprite fetch's start (TEKY: SOBU, not SUDA yet), and its last
    /// step with the sprite loaded (WUTY).
    step_reset: bool,
    loaded: bool,
    /// The window: WY and WX matched (PAFU ROGE, NOGY NUKO), its reset
    /// (XOFO), its column's clock (VETU).
    wy_match: bool,
    wxy_match: bool,
    win_reset: bool,
    wx_clk: bool,
}

/// The flip-flops' changes clocked in a nanosecond, for the next.
#[derive(Clone, Copy)]
struct Changes {
    mode3: Option<bool>,
    voga: Option<bool>,
    px_low: Option<u8>,
    px_high: Option<u8>,
    fetch_count: Option<u8>,
    lyzu: Option<bool>,
    nyka: Option<bool>,
    pory: Option<bool>,
    pygo: Option<bool>,
    fine: Option<u8>,
    puxa: Option<bool>,
    nyze: Option<bool>,
    ryfa: Option<bool>,
    rene: Option<bool>,
    fetched: Option<u16>,
    sobu: Option<bool>,
    suda: Option<bool>,
    sprite_step: Option<u8>,
    tobu: Option<bool>,
    vonu: Option<bool>,
    seba: Option<bool>,
    tyfo: Option<bool>,
    sary: Option<bool>,
    pyco: Option<bool>,
    nunu: Option<bool>,
    nopa: Option<bool>,
    win_x: Option<u8>,
    win_y: Option<u8>,
    sovy: Option<bool>,
}

impl Changes {
    const NONE: Self = Self {
        mode3: None,
        voga: None,
        px_low: None,
        px_high: None,
        fetch_count: None,
        lyzu: None,
        nyka: None,
        pory: None,
        pygo: None,
        fine: None,
        puxa: None,
        nyze: None,
        ryfa: None,
        rene: None,
        fetched: None,
        sobu: None,
        suda: None,
        sprite_step: None,
        tobu: None,
        vonu: None,
        seba: None,
        tyfo: None,
        sary: None,
        pyco: None,
        nunu: None,
        nopa: None,
        win_x: None,
        win_y: None,
        sovy: None,
    };
}

const fn rose(before: bool, now: bool) -> bool {
    !before & now
}

const fn fell(before: bool, now: bool) -> bool {
    before & !now
}

impl NetPipeline {
    /// The pipeline's part of render.
    #[must_use]
    pub const fn of(r: &NetRender) -> Self {
        Self {
            mode3: r.mode3,
            voga: r.voga,
            px: r.px,
            fetch_count: r.fetch_count,
            lyzu: r.lyzu,
            nyka: r.nyka,
            pory: r.pory,
            pygo: r.pygo,
            started: r.started,
            fine: r.fine,
            puxa: r.puxa,
            nyze: r.nyze,
            discard: r.discard,
            ryfa: r.ryfa,
            rene: r.rene,
            scx: r.scx,
            wy: r.wy,
            wx: r.wx,
            sary: r.sary,
            wy_latch: r.wy_latch,
            pyco: r.pyco,
            nunu: r.nunu,
            nopa: r.nopa,
            in_window: r.in_window,
            win_x: r.win_x,
            win_y: r.win_y,
            rydy: r.rydy,
            sovy: r.sovy,
            xs: r.store.xs,
            fetched: r.store.fetched,
            sobu: r.sobu,
            suda: r.suda,
            sprite_step: r.sprite_step,
            busy: r.busy,
            tobu: r.tobu,
            vonu: r.vonu,
            seba: r.seba,
            tyfo: r.tyfo,
            inputs: EdgeInputs::of(&r.inputs),
        }
    }

    /// The slots whose X matches the pixel counter while the sprites are
    /// enabled in mode 3.
    // @hw die: the X matchers; the store's enable (rendering, not CENO)
    #[must_use]
    pub const fn matches(&self, i: &EdgeInputs) -> u16 {
        let enabled = self.mode3 & !i.ceno & (i.lcdc & LCDC_OBJ_EN != 0);
        let mut matches = 0;
        let mut slot = 0;
        while slot < SLOTS {
            if enabled & (self.xs[slot] == self.px) {
                matches |= 1 << slot;
            }
            slot += 1;
        }
        matches
    }

    /// Mode 0, as render's `mode0` has it outside VBlank once an edge has
    /// settled: mode 3's stop, which a sprite's match holds off.
    #[must_use]
    pub const fn stop_mode3(&self) -> bool {
        (self.px == STOP_PX) & (self.matches(&self.inputs) == 0)
    }

    /// A tile's restart of the fetcher: the fine scroll's last dot (RYFA,
    /// not RENE yet), the window's first tile fetched (SOVY, not RYDY any
    /// more), or the first fetch done.
    // @hw die: TEVO, and its terms
    const fn restart(&self) -> bool {
        (self.ryfa & !self.rene) | (self.sovy & !self.rydy) | self.first_fetch_done()
    }

    /// The first fetch of the line is done (NYKA and PORY, not POKY yet).
    const fn first_fetch_done(&self) -> bool {
        self.mode3 & !self.started & self.nyka & self.pory
    }

    /// The wires, with the inputs as they are and the delayed X match.
    // @hw die: WODU XUGU XENA, TYFA SOCY VYBO SEGU, SACU; MOFU, NYXU
    // (fetch_done); RYCE TEKY SECA (step_reset); WUTY (loaded)
    const fn wires(&self, i: &EdgeInputs, held: bool) -> Wires {
        let matches = self.matches(i);
        let x_match = matches != 0;
        let stop_mode3 = (self.px == STOP_PX) & !held;
        let dot = !self.rydy & self.started & !x_match & !stop_mode3 & i.clk;
        let segu = !dot;
        let win_start = self.in_window & !self.nopa;
        let fetch_reset = i.stop_scan | win_start | self.restart();
        let fetch_done = (self.fetch_count & FETCH_DONE == FETCH_DONE) & !fetch_reset;
        let odd_step = self.sprite_step & 1 != 0;
        Wires {
            stop_mode3,
            dot,
            segu,
            clkpipe: segu | self.discard,
            fetch_done,
            fetch_clock: !i.clk | fetch_done,
            selected: matches & (!matches).wrapping_add(1),
            x_match,
            step_reset: (self.sobu & !self.suda) | i.ppu_reset | i.line_start,
            loaded: self.tyfo & odd_step & self.seba & self.vonu,
            wy_match: (i.lcdc & LCDC_WIN_EN != 0) & (self.wy == i.ly),
            wxy_match: self.wy_latch & (self.wx == self.px),
            win_reset: (i.lcdc & LCDC_WIN_EN == 0) | i.line_start,
            wx_clk: self.restart() & self.in_window,
        }
    }

    /// The latches and the asynchronous resets, settled in the order they
    /// feed each other.
    const fn settle(&mut self, i: &EdgeInputs) {
        if i.line_start {
            self.voga = false;
            self.px = 0;
            self.fetched = 0;
        }
        if i.ppu_reset | self.voga {
            self.mode3 = false;
        }
        if !self.mode3 | (self.in_window & !self.nopa) {
            // The window's start restarts the first fetch's NYKA and PORY.
            self.nyka = false;
            self.pory = false;
        }
        if !self.mode3 {
            self.lyzu = false;
            self.pygo = false;
            self.started = false;
            self.puxa = false;
            self.nyze = false;
            self.ryfa = false;
            self.rene = false;
            self.tobu = false;
            self.vonu = false;
        } else if self.pygo {
            self.started = true;
        } else {
            // POKY holds.
        }
        let restart = self.restart();
        // The fetcher's reset: the scan's stop, the window's start, a restart.
        // The chip's reset clears NOPA at once and the window a delta later
        // (through its reset's wire): the start glitches up between them.
        let starts = self.in_window & !(self.nopa & !i.ppu_reset);
        if i.stop_scan | starts | restart {
            self.fetch_count = 0;
        }
        if !self.mode3 | restart {
            self.fine = 0;
        }
        if self.puxa & !self.nyze {
            self.discard = false;
        } else if !self.mode3 {
            self.discard = true;
        } else {
            // ROXY holds.
        }
        if i.hard_reset {
            self.scx = 0;
            self.wy = 0;
            self.wx = 0;
        }
        // The window's latches and its counters' resets.
        if i.ppu_reset {
            self.sary = false;
            self.pyco = false;
            self.nunu = false;
            self.nopa = false;
            self.sovy = false;
        }
        if i.mode1 | i.ppu_reset {
            self.wy_latch = false;
            self.win_y = 0;
        } else if self.sary {
            self.wy_latch = true;
        } else {
            // REJO holds.
        }
        let win_reset = (i.lcdc & LCDC_WIN_EN == 0) | i.line_start;
        if win_reset {
            self.in_window = false;
            self.win_x = 0;
        } else if self.nunu {
            self.in_window = true;
        } else {
            // PYNU holds.
        }
        if self.pory | i.ppu_reset {
            self.rydy = false;
        } else if self.in_window & !self.nopa {
            self.rydy = true;
        } else {
            // RYDY holds.
        }
        // The store's X latches: a slot fetched, or a line's start, empties
        // them.
        let mut slot = 0;
        while slot < SLOTS {
            if i.line_start | (self.fetched & (1 << slot) != 0) {
                self.xs[slot] = NO_X;
            }
            slot += 1;
        }
        // The sprite fetch's start resets its steps and sets TAKA; its last
        // step, or the first fetch done, clears TAKA.
        let w = self.wires(i, false);
        if w.step_reset {
            self.sprite_step = 0;
            self.busy = true;
        } else if w.loaded | self.first_fetch_done() {
            self.busy = false;
        } else {
            // TAKA holds.
        }
    }

    /// SCX's, WY's and WX's latches, open while the write strobe is up (it opens them a
    /// few deltas after the clock) on the address and the data bus as they
    /// are.
    const fn latch_scx(&mut self, i: &EdgeInputs) {
        if i.hard_reset {
            self.scx = 0;
            self.wy = 0;
            self.wx = 0;
        } else if i.cpu_wr_sync {
            match i.addr {
                SCX => self.scx = i.data,
                WY => self.wy = i.data,
                WX => self.wx = i.data,
                _ => {}
            }
        } else {
            // The latches hold.
        }
    }

    /// What the edges of the clocks made from the PPU's (from `before` to
    /// `now`) clock, from the pipeline as it is, for the next nanosecond.
    const fn clocked(
        &self,
        i: &EdgeInputs,
        before: Wires,
        now: Wires,
        clk_edge: bool,
        hclk_rose: bool,
    ) -> Changes {
        let mut c = Changes::NONE;
        let clk_rose = clk_edge & i.clk;
        let clk_fell = clk_edge & !i.clk;
        let restart = self.restart();
        // @hw die: XYMU (nor_latch); VOGA (dffr)
        if !(i.ppu_reset | self.voga) & i.stop_scan & !self.mode3 {
            c.mode3 = Some(true);
        }
        if !i.line_start & clk_rose {
            c.voga = Some(before.stop_mode3);
        }
        // @hw die: XEHO SAVY XODU XYDO (dffr)
        if !i.line_start & rose(before.clkpipe, now.clkpipe) {
            c.px_low = Some(self.px.wrapping_add(1) & 0x0F);
        }
        // @hw die: LAXU MESU NYVA (dffr); LYZU (dffr)
        let win_start = self.in_window & !self.nopa;
        if !(i.stop_scan | win_start | restart) & rose(before.fetch_clock, now.fetch_clock) {
            c.fetch_count = Some(self.fetch_count.wrapping_add(1) & 7);
        }
        if self.mode3 & clk_rose {
            c.lyzu = Some(self.fetch_count & 1 != 0);
            c.nyka = Some(before.fetch_done);
            c.pygo = Some(self.pory);
            c.rene = Some(self.ryfa);
        }
        if self.mode3 & clk_fell {
            c.pory = Some(self.nyka);
            c.nyze = Some(self.puxa);
        }
        // @hw die: RYKU ROGA RUBU (dffr); PUXA NYZE (dffr); RYFA RENE (dffr)
        if !(!self.mode3 | restart) & fell(before.dot, now.dot) & (self.fine != 7) {
            c.fine = Some(self.fine.wrapping_add(1));
        }
        if self.mode3 & rose(before.dot, now.dot) {
            c.puxa = Some(self.discard & (self.fine == self.scx & 7));
        }
        if self.mode3 & rose(before.segu, now.segu) {
            c.ryfa = Some((self.fine == 7) & !now.wxy_match);
        }
        // @hw die: SOBU SUDA (dffr); TOXE TULY TESE (dffr); TOBU VONU SEBA
        // TYFO (dffr); EBOJ..FONO (dffr)
        if clk_rose {
            c.sobu = Some(before.x_match & !self.rydy & before.fetch_done & !self.busy);
        }
        if clk_fell {
            c.suda = Some(self.sobu);
        }
        if !now.step_reset & clk_rose & (self.sprite_step != SPRITE_DONE) {
            c.sprite_step = Some(self.sprite_step + 1);
        }
        if self.mode3 & clk_rose {
            c.tobu = Some(self.sprite_step & 2 != 0);
            c.vonu = Some(self.tobu);
        }
        if clk_fell {
            c.tyfo = Some(self.sprite_step & 1 != 0);
            c.seba = Some(self.mode3 & self.vonu);
        }
        if !i.line_start & rose(before.loaded, now.loaded) {
            c.fetched = Some(now.selected);
        }
        self.clocked_window(&mut c, i, before, now, (clk_rose, clk_fell, hclk_rose));
        c
    }

    /// What the clocks' edges clock of the window, into `c`; `edges` are
    /// the clock's rise and fall and hclk's rise.
    // @hw die: SARY (dffr); PYCO NUNU NOPA (dffr); WYKA..XOLO (dffr); SOVY
    const fn clocked_window(
        &self,
        c: &mut Changes,
        i: &EdgeInputs,
        before: Wires,
        now: Wires,
        edges: (bool, bool, bool),
    ) {
        let (clk_rose, clk_fell, hclk_rose) = edges;
        if !i.ppu_reset {
            if hclk_rose {
                // The match as the wires had it: LY changes with hclk.
                c.sary = Some(before.wy_match);
            }
            if rose(before.dot, now.dot) {
                c.pyco = Some(now.wxy_match);
            }
            if clk_fell {
                c.nunu = Some(self.pyco);
            }
            if clk_rose {
                c.nopa = Some(self.in_window);
                c.sovy = Some(self.rydy);
            }
        }
        if !now.win_reset & rose(before.wx_clk, now.wx_clk) {
            c.win_x = Some(self.win_x.wrapping_add(1) & WIN_X_MASK);
        }
    }

    /// The changes clocked the nanosecond before take effect; the pixel
    /// counter's bit 3, falling, clocks its high half.
    #[expect(clippy::cognitive_complexity, reason = "a field a change, in turn")]
    const fn apply(&mut self, c: &Changes, next: &mut Changes, i: &EdgeInputs) {
        let px_before = self.px;
        if let Some(v) = c.mode3 {
            self.mode3 = v;
        }
        if let Some(v) = c.voga {
            self.voga = v;
        }
        if let Some(v) = c.px_low {
            self.px = self.px & 0xF0 | v;
        }
        if let Some(v) = c.px_high {
            self.px = self.px & 0x0F | v << 4;
        }
        if let Some(v) = c.fetch_count {
            self.fetch_count = v;
        }
        if let Some(v) = c.lyzu {
            self.lyzu = v;
        }
        if let Some(v) = c.nyka {
            self.nyka = v;
        }
        if let Some(v) = c.pory {
            self.pory = v;
        }
        if let Some(v) = c.pygo {
            self.pygo = v;
        }
        if let Some(v) = c.fine {
            self.fine = v;
        }
        if let Some(v) = c.puxa {
            self.puxa = v;
        }
        if let Some(v) = c.nyze {
            self.nyze = v;
        }
        if let Some(v) = c.ryfa {
            self.ryfa = v;
        }
        if let Some(v) = c.rene {
            self.rene = v;
        }
        if let Some(v) = c.fetched {
            self.fetched = v;
        }
        if let Some(v) = c.sobu {
            self.sobu = v;
        }
        if let Some(v) = c.suda {
            self.suda = v;
        }
        if let Some(v) = c.sprite_step {
            self.sprite_step = v;
        }
        if let Some(v) = c.tobu {
            self.tobu = v;
        }
        if let Some(v) = c.vonu {
            self.vonu = v;
        }
        if let Some(v) = c.seba {
            self.seba = v;
        }
        if let Some(v) = c.tyfo {
            self.tyfo = v;
        }
        if let Some(v) = c.sary {
            self.sary = v;
        }
        if let Some(v) = c.pyco {
            self.pyco = v;
        }
        if let Some(v) = c.nunu {
            self.nunu = v;
        }
        if let Some(v) = c.nopa {
            self.nopa = v;
        }
        if let Some(v) = c.win_x {
            self.win_x = v;
        }
        if let Some(v) = c.win_y {
            self.win_y = v;
        }
        if let Some(v) = c.sovy {
            self.sovy = v;
        }
        // @hw die: TUHU TUKY TAKO SYBE (dffr)
        if !i.line_start & fell(px_before & 8 != 0, self.px & 8 != 0) {
            next.px_high = Some((self.px >> 4).wrapping_add(1) & 0x0F);
        }
    }

    /// The next edge of the PPU's clock, with render's inputs after it.
    pub const fn edge(&mut self, inputs: EdgeInputs) {
        let old = self.inputs;
        // 0 ns: the clock, and the write strobe.
        let mut now_in = EdgeInputs {
            clk: inputs.clk,
            cpu_wr_sync: inputs.cpu_wr_sync,
            ..old
        };
        // The X match as it reaches mode 3's stop: caught up at the last
        // edge's end, and a rise pending until `held_at`.
        let mut held = self.matches(&old) != 0;
        let mut held_at = NEVER;
        let mut before = self.wires(&old, held);
        let mut wires = self.wires(&now_in, held);
        let mut changes = self.clocked(&now_in, before, wires, old.clk != inputs.clk, false);
        self.latch_scx(&now_in);
        let mut ns = 1;
        while ns < NS {
            if ns == INPUTS_AT {
                now_in.ppu_reset = inputs.ppu_reset;
                now_in.hard_reset = inputs.hard_reset;
                now_in.lcdc = inputs.lcdc;
                now_in.ly = inputs.ly;
                now_in.hclk = inputs.hclk;
                // stop_scan falls with the other inputs, rises with the scan's.
                now_in.stop_scan = old.stop_scan & inputs.stop_scan;
            }
            if ns == SCAN_AT {
                now_in.line_start = inputs.line_start;
                now_in.stop_scan = inputs.stop_scan;
                now_in.ceno = inputs.ceno;
            }
            if ns == MODE1_AT {
                now_in.mode1 = inputs.mode1;
            }
            if ns == DATA_AT {
                now_in.addr = inputs.addr;
                now_in.data = inputs.data;
            }
            let hclk_rose = (ns == INPUTS_AT) & !old.hclk & inputs.hclk;
            let was_in_window = self.in_window;
            let mut next = Changes::NONE;
            self.apply(&changes, &mut next, &now_in);
            if ns == held_at {
                held = true;
            }
            self.settle(&now_in);
            self.latch_scx(&now_in);
            // @hw die: VYNO..TEKE (dffr): the window's line, as it leaves the
            // window.
            if !(now_in.mode1 | now_in.ppu_reset) & was_in_window & !self.in_window {
                next.win_y = Some(self.win_y.wrapping_add(1));
            }
            // The match's fall reaches the stop at once, its rise later.
            let x_match = self.matches(&now_in) != 0;
            if !x_match {
                held = false;
                held_at = NEVER;
            } else if !wires.x_match & (held_at == NEVER) & !held {
                held_at = ns + MATCH_HELD_AFTER;
            } else {
                // The rise is pending or done.
            }
            before = wires;
            wires = self.wires(&now_in, held);
            let clocked = self.clocked(&now_in, before, wires, false, hclk_rose);
            changes = merge(next, clocked);
            ns += 1;
        }
        self.inputs = inputs;
    }

    /// The pipeline is render's.
    #[must_use]
    pub fn same_as(&self, r: &NetRender) -> bool {
        *self == Self::of(r)
    }
}

/// The changes of a nanosecond: the pixel counter's high half and the
/// window's line (clocked by latches), and what the clocks clocked.
const fn merge(high: Changes, mut c: Changes) -> Changes {
    c.px_high = high.px_high;
    c.win_y = high.win_y;
    c
}
