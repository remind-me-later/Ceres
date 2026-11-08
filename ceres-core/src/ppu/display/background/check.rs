//! The background pipeline against the netlist's (`netlist_proofs.rs`).
//!
//! Ceres's mode 3 loop runs a dot's work in the dot's first half dot (odd
//! `h`); the netlist's pipeline (`ceres_netlist::render_edge`) takes its two
//! edges, the clock rising at odd `h` and falling at even `h`. After both,
//! the netlist's pipeline is a function of ceres's background state
//! (`pipeline_for`): its fetcher's count, its first fetch's flip-flops and its
//! fine scroll follow from the FIFO's length, which counts the dots of each
//! tile; the pixel counter, and the fine scroll's discard, follow from the
//! position. Neither depends on SCX: ceres's lead-in matches SCX's low bits two
//! dots before the netlist's fine scroll does, and runs those two dots at
//! positions -7 and -6, which the netlist spends discarding.
//!
//! This holds while the window and the objects are idle: ceres's loop then
//! pops a pixel and steps its fetcher every dot (`ceres_dot`).
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
    clippy::verbose_bit_mask,
    reason = "Aeneas models the mask, not trailing_zeros"
)]

use {
    super::Background,
    crate::ppu::{
        PX_WIDTH,
        display::{fetcher::FetcherStep, pos},
    },
    ceres_netlist::render_edge::{EdgeInputs, NetPipeline},
};

/// SCX's address.
const SCX: u16 = 0xFF43;
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

/// The netlist's pipeline after ceres's dot and its two edges, with its SCX
/// and its inputs as they are.
#[expect(
    clippy::cast_sign_loss,
    clippy::cast_possible_truncation,
    reason = "the pixel counter, positive by then"
)]
pub(crate) const fn pipeline_for(bg: &Background, scx: u8, inputs: EdgeInputs) -> NetPipeline {
    let p = signed(bg.position);
    let len = bg.fifo.len();
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
    if first_round(bg) {
        // The first fetch's restart, not the fine scroll's, began the tile.
        rene = false;
        if len == 7 {
            fetch_count = FETCH_DONE;
            (lyzu, nyka, pory, pygo, started, fine) = (false, false, false, false, false, 0);
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
        scx,
        inputs,
    }
}

/// The netlist's pipeline at the end of the dot before ceres's loop starts
/// (`h = 162` of a visible line): mode 3 rose at `h = 154`, the first
/// fetch's count is at 4, the fine scroll discards. Measured: the same on
/// every SCX (gbmicrotest's `hblank_int_scx0` to 7).
pub(crate) const fn before_start(scx: u8, inputs: EdgeInputs) -> NetPipeline {
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
        scx,
        inputs,
    }
}

/// Ceres's first dot of the loop (`Mode3Start`): mode 3 starts, then the
/// dot's work.
pub(crate) fn ceres_first_dot(bg: &mut Background, mixer_scx: u8) {
    bg.start();
    ceres_dot(bg, mixer_scx);
}

/// The first dot from the netlist's state before it, and ceres's after the
/// line before (its position back at the lead-in's start, and no blank pixel
/// inserted: the window was idle): both related.
pub(crate) fn start_agrees(
    bg: &mut Background,
    net: &mut NetPipeline,
    mixer_scx: u8,
    rising: EdgeInputs,
    falling: EdgeInputs,
) -> bool {
    let before = !net.inputs.clk
        & quiet(&net.inputs)
        & (net.scx & 7 == mixer_scx & 7)
        & (*net == before_start(net.scx, net.inputs))
        & (bg.position == pos(-16))
        & !bg.insert_blank;
    if !(before & rising.clk & !falling.clk & quiet(&rising) & quiet(&falling)) {
        return true;
    }
    ceres_first_dot(bg, mixer_scx);
    net.edge(rising);
    net.edge(falling);
    related(bg, net, mixer_scx)
}

/// The background's shapes a background-only line passes through.
const fn reachable(bg: &Background, mixer_scx: u8) -> bool {
    let p = signed(bg.position);
    let len = bg.fifo.len();
    let shapes = (len >= 1)
        & (len <= FULL)
        & matches_step(bg.fetcher.step, step_for(len))
        & !bg.insert_blank
        & (p >= LEAD_IN)
        & (p <= PX_WIDTH as i16);
    let lead_in = if p <= LAST_UNMATCHED {
        // Unmatched: a pixel popped a dot since the start, and no earlier
        // position matched.
        first_round(bg)
            & (p > LEAD_IN)
            & (p + len as i16 == LEAD_IN + FULL as i16)
            & (mixer_scx & 7 >= bg.position & 7)
    } else {
        // Matched at position m, m = SCX (mod 8): the position jumped to -8,
        // and since then it has gained a pixel for each one the FIFO lost
        // (and 8 for each row pushed), so that m = -16 - (p + len) (mod 8).
        let since_match = p + len as i16 + (mixer_scx & 7) as i16;
        (!first_round(bg) | (p > -8)) & (since_match & 7 == 0)
    };
    shapes & lead_in
}

const fn matches_step(a: FetcherStep, b: FetcherStep) -> bool {
    a as u8 == b as u8
}

/// The netlist's inputs in a background-only mode 3: no reset, no line
/// start, the scan stopped, and the CPU not addressing SCX.
const fn quiet(i: &EdgeInputs) -> bool {
    !i.ppu_reset & !i.hard_reset & !i.line_start & !i.stop_scan & (i.addr != SCX)
}

/// Ceres's background and the netlist's pipeline after a dot (at even `h`),
/// with the mixer's SCX.
pub(crate) fn related(bg: &Background, net: &NetPipeline, mixer_scx: u8) -> bool {
    reachable(bg, mixer_scx)
        & !net.inputs.clk
        & quiet(&net.inputs)
        & (net.scx & 7 == mixer_scx & 7)
        & (*net == pipeline_for(bg, net.scx, net.inputs))
}

/// A dot of ceres's mode 3 loop with the window and the objects idle
/// (`Step::OutputPixel`): the FIFO's pixel, then the fetcher (its read is
/// the PPU's, and does not change when the pixels come out).
pub(crate) fn ceres_dot(bg: &mut Background, mixer_scx: u8) {
    _ = bg.pop(mixer_scx, false);
    bg.advance(false, None);
}

/// A dot from related states: both stay related, and mode 3 goes on; or,
/// once ceres has drawn the line (its `HBlankStart` runs in this dot's first
/// half dot), the netlist's mode 3 stops at the dot's second edge.
pub(crate) fn step_agrees(
    bg: &mut Background,
    net: &mut NetPipeline,
    mixer_scx: u8,
    rising: EdgeInputs,
    falling: EdgeInputs,
) -> bool {
    if !(related(bg, net, mixer_scx) & rising.clk & !falling.clk & quiet(&rising) & quiet(&falling))
    {
        return true;
    }
    let drawn = bg.position == PX_WIDTH;
    if !drawn {
        ceres_dot(bg, mixer_scx);
    }
    net.edge(rising);
    let stopped_early = net.stop_mode3();
    net.edge(falling);
    if drawn {
        !stopped_early & net.stop_mode3()
    } else {
        !stopped_early & !net.stop_mode3() & related(bg, net, mixer_scx)
    }
}
