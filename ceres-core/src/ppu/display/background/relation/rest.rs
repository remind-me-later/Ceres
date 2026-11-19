//! The netlist's pipeline outside mode 3: from mode 3's end, through HBlank
//! (and VBlank), the next line's scan and mode 3's rise, to `before_start`
//! at `h = 162`.
//!
//! Outside mode 3 the pipeline does not follow ceres: it rests. Mode 3's
//! fall (XYMU, through VOGA a rising edge after the stop) clears the
//! fetcher's flip-flops, the fine scroll and the sprite fetch's mode 3
//! steps, and nothing sets them again before the scan's stop
//! (`hblank_rest`); the line's start (the scan's `line_start`) clears the
//! pixel counter, VOGA, the window and the sprites fetched, and restarts
//! the sprite fetch's steps, which count up to their last by `h = 5`; the
//! scan's stop at `h = 154` raises mode 3, and the first fetch's count
//! reaches 4 by `h = 162` (`pipe_at`, an exact shape at each edge of the
//! scan, measured on NetChip's runs of the test ROMs and proved here edge by
//! edge). What the pipeline takes from the scan is its `line_start`,
//! `stop_scan` and `ceno` (`scan_inputs`: their edges, which the scan's
//! claims show it gives, `objects::relation::outputs_at`), and the clock;
//! LCDC, LY, hclk, VBlank and the CPU's bus are free. SCX's, WY's and WX's
//! latches, the window's WY match and line counter, and the sprite store's
//! X are left as they are: `before_start` carries them over.

#[cfg_attr(
    not(test),
    expect(clippy::wildcard_imports, reason = "the check's parts share its items")
)]
use super::*;

/// What the pipeline takes in an edge besides the scan and the clock: LCDC,
/// LY, hclk, VBlank and the CPU's bus.
#[derive(Clone, Copy)]
#[cfg_attr(kani, derive(kani::Arbitrary))]
pub(crate) struct Free {
    pub lcdc: u8,
    pub ly: u8,
    pub hclk: bool,
    pub mode1: bool,
    pub cpu_wr_sync: bool,
    pub addr: u16,
    pub data: u8,
}

/// The fetcher's count is three flip-flops.
const COUNT_MAX: u8 = 7;

/// What mode 3's fall clears, and nothing outside mode 3 sets: XYMU low,
/// the fetcher's flip-flops (but its count), the fine scroll (its count at
/// 0, ROXY discarding) and the sprite fetch's mode 3 steps (its step
/// counter, three flip-flops, at most at its last).
const fn out_of_mode3(p: &NetPipeline) -> bool {
    let f = &p.fetcher;
    !p.mode3
        & (f.count <= COUNT_MAX)
        & !f.lyzu
        & !f.nyka
        & !f.pory
        & !f.pygo
        & !f.started
        & (p.fine == 0)
        & !p.puxa
        & !p.nyze
        & p.discard
        & !p.ryfa
        & !p.rene
        & (p.sprite.step <= SPRITE_DONE)
        & !p.sprite.tobu
        & !p.sprite.vonu
}

/// The inputs outside the scan's start and stop: no reset, no line start,
/// the scan not stopping.
const fn idle_inputs(i: &EdgeInputs) -> bool {
    !i.ppu_reset & !i.hard_reset & !i.line_start & !i.stop_scan
}

/// The window outside mode 3: no WX match (PYCO, NUNU), its first tile not
/// fetched (RYDY, SOVY), and NOPA following PYNU: nothing clocks it but the
/// clock, and PYNU only falls (at the line's start or with LCDC's bit 5).
const fn window_rests(p: &NetPipeline) -> bool {
    !p.pyco & !p.nunu & !p.rydy & !p.sovy & (!p.in_window | p.nopa)
}

/// The pipeline in HBlank and VBlank: out of mode 3, the window resting,
/// the inputs idle.
pub(crate) const fn hblank_rest(p: &NetPipeline) -> bool {
    out_of_mode3(p) & window_rests(p) & idle_inputs(&p.inputs)
}

/// What the line's start clears: the pixel counter, VOGA, the window's
/// flip-flops (but NOPA, which follows PYNU at the next rise) and column
/// counter, the sprites fetched.
const fn line_cleared(p: &NetPipeline) -> bool {
    !p.voga
        & (p.px == 0)
        & !p.pyco
        & !p.nunu
        & !p.in_window
        & (p.win_x == 0)
        & !p.rydy
        & !p.sovy
        & (p.fetched == 0)
}

/// The sprite fetch's steps from the line's start (`h = -6`, where the
/// scan's `line_start` resets them) to their last at `h = 5`: they count at
/// each rise of the clock once the line's start has fallen.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "between -3 and 4, `h + 5` halved is 1 to 4"
)]
const fn lead_step(h: i32) -> u8 {
    if h <= -4 {
        0
    } else if h >= 5 {
        5
    } else {
        ((h + 5) >> 1) as u8
    }
}

/// The scan's outputs and the clock at the edge `h` of the scan: the
/// line's start over `-6` and `-5` (CATU before ANEL), the stop at `154`
/// (BYBA before DOBA), CENO from `-2` to `157`; the clock high at odd `h`.
pub(crate) const fn scan_inputs(h: i32, free: Free) -> EdgeInputs {
    EdgeInputs {
        clk: h % 2 != 0,
        ppu_reset: false,
        hard_reset: false,
        lcdc: free.lcdc,
        ly: free.ly,
        hclk: free.hclk,
        mode1: free.mode1,
        line_start: (h == -6) | (h == -5),
        stop_scan: h == 154,
        ceno: (h >= -2) & (h <= 157),
        cpu_wr_sync: free.cpu_wr_sync,
        addr: free.addr,
        data: free.data,
    }
}

/// `i` is as `scan_inputs` has it at `h`, whatever is free.
const fn inputs_at(i: &EdgeInputs, h: i32) -> bool {
    let free = Free {
        lcdc: i.lcdc,
        ly: i.ly,
        hclk: i.hclk,
        mode1: i.mode1,
        cpu_wr_sync: i.cpu_wr_sync,
        addr: i.addr,
        data: i.data,
    };
    let want = scan_inputs(h, free);
    (i.clk == want.clk)
        & (i.ppu_reset == want.ppu_reset)
        & (i.hard_reset == want.hard_reset)
        & (i.line_start == want.line_start)
        & (i.stop_scan == want.stop_scan)
        & (i.ceno == want.ceno)
}

/// The first fetch's count and LYZU from mode 3's rise (`h = 154`) to
/// `before_start` (`h = 162`): the stop clears the count, which then counts
/// at each fall of the clock, LYZU taking the count's low bit at each rise.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "from 154 to 162, the count is 0 to 4"
)]
const fn rise_count(h: i32) -> u8 {
    ((h - 154) / 2) as u8
}

const fn rise_lyzu(h: i32) -> bool {
    (h == 157) | (h == 158) | (h == 161) | (h == 162)
}

/// The pipeline at the edge `h` of the scan, from the line's end (`-8`) to
/// `before_start` (`162`).
pub(crate) const fn pipe_at(p: &NetPipeline, h: i32) -> bool {
    let s = &p.sprite;
    if h <= -7 {
        hblank_rest(p)
    } else if h <= 153 {
        // NOPA follows PYNU at the first rise, `-5`, where SOBU falls (no X
        // matches outside mode 3) and the steps' reset with it. The steps
        // count; from `h = 6` at rest, TYFO taking the last step's low bit.
        let rest = (h < 6) | s.tyfo & !s.sobu & !s.suda & !s.seba;
        out_of_mode3(p)
            & line_cleared(p)
            & ((h == -6) | !p.nopa)
            & ((h == -6) | !s.sobu)
            & inputs_at(&p.inputs, h)
            & s.busy
            & (s.step == lead_step(h))
            & rest
    } else {
        let f = &p.fetcher;
        p.mode3
            & line_cleared(p)
            & !p.nopa
            & inputs_at(&p.inputs, h)
            & (f.count == rise_count(h))
            & (f.lyzu == rise_lyzu(h))
            & !f.nyka
            & !f.pory
            & !f.pygo
            & !f.started
            & (p.fine == 0)
            & !p.puxa
            & !p.nyze
            & p.discard
            & !p.ryfa
            & !p.rene
            & !s.sobu
            & !s.suda
            & (s.step == SPRITE_DONE)
            & s.busy
            & !s.tobu
            & !s.vonu
            & !s.seba
            & s.tyfo
    }
}

/// An edge of HBlank or VBlank: from `hblank_rest`, with no line start and
/// no stop, the pipeline rests.
pub(crate) const fn hblank_agrees(p: NetPipeline, i: EdgeInputs) -> bool {
    if !(hblank_rest(&p) & idle_inputs(&i)) {
        return true;
    }
    let mut p = p;
    p.edge(i);
    hblank_rest(&p)
}

/// Mode 3's end: from a pair related where ceres has drawn the line (its
/// `HBlankStart` runs in this dot), on inputs `step_agrees` takes, the
/// netlist's mode 3 stops at the dot's second edge (`step_agrees`); the
/// next edge, the clock's rise, takes VOGA and drops mode 3, and the
/// pipeline rests.
pub(crate) fn end_agrees(
    l: &Mode3Loop,
    net: NetPipeline,
    g: Ghost,
    mixer_scx: u8,
    rising: EdgeInputs,
    falling: EdgeInputs,
    after: EdgeInputs,
) -> bool {
    if !(related(l, &net, g)
        & drawn(&l.pace)
        & rising.clk
        & !falling.clk
        & quiet(&rising, l)
        & quiet(&falling, l)
        & writes(&net, &rising, &falling, l.mixer_scx)
        & (!l.window_enabled
            | window::holds_wx(&net.inputs, &rising) & window::holds_wx(&rising, &falling))
        & window::lead_in_matches(l, mixer_scx)
        & after.clk
        & idle_inputs(&after))
    {
        return true;
    }
    let mut net = net;
    net.edge(rising);
    net.edge(falling);
    net.edge(after);
    hblank_rest(&net)
}

/// An edge of the scan, `h + 1`, from the pipeline at `h`, with the scan's
/// outputs at `h + 1` and anything free: the pipeline is at `h + 1`; at
/// `162`, as `before_start` has it.
pub(crate) fn scan_agrees(p: NetPipeline, h: i32, free: Free) -> bool {
    if !((h >= -8) & (h <= 161) & pipe_at(&p, h)) {
        return true;
    }
    let mut p = p;
    p.edge(scan_inputs(h + 1, free));
    pipe_at(&p, h + 1) & ((h + 1 != 162) | (p == before_start(&p)))
}
