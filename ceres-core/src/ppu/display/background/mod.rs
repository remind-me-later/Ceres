//! The background pipeline: its pixel FIFO, its fetcher's steps, and where
//! the line's pixels are.
//!
//! What decides when the background's pixels come out, apart from the rest
//! of the PPU: the PPU does the fetcher's reads (`fetcher.rs`) and mixes the
//! pixels with the objects' (`pixels.rs`), and calls these in the order
//! SameBoy's mode 3 loop runs them (`mode3.rs`).

#[cfg(any(kani, feature = "check"))]
pub(crate) mod check;
#[cfg(kani)]
mod netlist_proofs;

use {
    super::{
        fetcher::{Fetcher, FetcherStep},
        fifo::{Fifo, Item},
        pos,
    },
    crate::ppu::{ATTR_CGB_PALETTE, ATTR_PRIORITY_B, ATTR_X_FLIP_B, PX_WIDTH},
};

/// The last logical position of the window insertion glitch's compare: past
/// it, the compare is with 0.
const LAST_LOGICAL_POSITION: u8 = 167;

/// The background pipeline's state.
#[derive(Clone)]
pub(super) struct Background {
    /// SameBoy's `position_in_line`: wraps like its `uint8_t` (240..=255 is
    /// -16..=-1, the lead-in).
    pub position: u8,
    /// The lead-in dropped pixels for SCX's low bits (SameBoy's
    /// `line_has_fractional_scrolling`).
    pub fractional: bool,
    /// The next pixel is a blank one inserted before the FIFO's front (the
    /// window's pixel insertion glitch).
    pub insert_blank: bool,
    pub fifo: Fifo,
    pub fetcher: Fetcher,
}

/// What became of the FIFO's front pixel.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Popped {
    /// The lead-in went back to its start (position -9 without a match):
    /// the pixel is lost.
    Restarted,
    /// It was dropped: the lead-in's, or one scrolled out.
    Dropped,
    /// It goes to the LCD.
    Shown,
}

impl Default for Background {
    fn default() -> Self {
        Self {
            position: pos(-16),
            fractional: false,
            insert_blank: false,
            fifo: Fifo::default(),
            fetcher: Fetcher::default(),
        }
    }
}

impl Background {
    /// Mode 3 starts: the FIFO holds 8 pixels of junk, which the lead-in
    /// drops, and the fetcher begins a tile.
    pub(super) fn start(&mut self) {
        self.fifo.clear();
        self.fifo.push_bg_row(0, 0, 0, false, false);
        self.fetcher.step = FetcherStep::GetTileT1;
    }

    /// The X an object must have to be fetched at this position: the
    /// position plus 8, and 0 in the lead-in.
    pub(super) const fn object_match_x(&self) -> u8 {
        let x = self.position.wrapping_add(8);
        if x > 240 { 0 } else { x }
    }

    /// The position goes back to the lead-in's start, for the next line.
    pub(super) const fn restart_line(&mut self) {
        self.position = pos(-16);
        self.fractional = false;
    }

    /// The line is drawn (`skip_slow_mode_3`): a fetch cut off in its high
    /// byte keeps the last byte read.
    pub(super) const fn end_line(&mut self) {
        self.restart_line();
        if matches!(
            self.fetcher.step,
            FetcherStep::DataHighT1 | FetcherStep::DataHighT2
        ) {
            self.fetcher.data[1] = self.fetcher.data[0];
        }
    }

    /// The FIFO's front pixel (or the blank one inserted before it), and
    /// what became of it, with the mixer's SCX and whether the window's
    /// first tile is being fetched: `None` if the FIFO is empty.
    pub(super) fn pop(&mut self, scx: u8, window_fetched: bool) -> Option<(Item, Popped)> {
        if self.fifo.is_empty() {
            return None;
        }
        let item = if self.insert_blank {
            self.insert_blank = false;
            Item::default()
        } else {
            self.fifo.pop()
        };

        // (position + 16 < 8) in u8 arithmetic: the lead-in range.
        let position = self.position;
        if position.wrapping_add(16) < 8 {
            if position == pos(-17) {
                self.position = pos(-16);
            } else if position & 7 == scx & 7 || window_fetched && position & 7 == 6 && scx & 7 == 7
            {
                self.position = pos(-8);
            } else if position == pos(-9) {
                self.position = pos(-16);
                return Some((item, Popped::Restarted));
            } else {
                self.fractional = true;
            }
        }

        // Drop pixels for scrolling (negative positions compare >= 160 in u8).
        let shown = self.position < PX_WIDTH;
        self.position = self.position.wrapping_add(1);
        Some((
            item,
            if shown {
                Popped::Shown
            } else {
                Popped::Dropped
            },
        ))
    }

    /// The fetcher's step after its read: the next, or the push of its row
    /// (which the CGB's WX glitch skips in the high byte's second dot).
    /// `insertion_wx` is WX, if the DMG's window insertion glitch can
    /// happen.
    pub(super) fn advance(&mut self, cgb_wx_glitch: bool, insertion_wx: Option<u8>) {
        match self.fetcher.step {
            FetcherStep::DataHighT2 if !cgb_wx_glitch => self.push(insertion_wx),
            FetcherStep::Push => self.push(insertion_wx),
            step => self.fetcher.step = step.next(),
        }
    }

    /// The fetcher pushes its row once the FIFO is empty, and begins the
    /// next tile.
    fn push(&mut self, insertion_wx: Option<u8>) {
        self.fetcher.step = FetcherStep::Push;
        if !self.fifo.is_empty() {
            return;
        }
        if let Some(wx) = insertion_wx {
            // See https://github.com/LIJI32/SameBoy/issues/278
            let mut logical_position = self.position.wrapping_add(7);
            if logical_position > LAST_LOGICAL_POSITION {
                logical_position = 0;
            }
            if wx == logical_position {
                self.fifo.rewind_to_blank_pixel();
                return;
            }
        }
        let attr = self.fetcher.attributes;
        let [low, high] = self.fetcher.data;
        self.fifo.push_bg_row(
            low,
            high,
            attr & ATTR_CGB_PALETTE,
            attr & ATTR_PRIORITY_B != 0,
            attr & ATTR_X_FLIP_B != 0,
        );
        self.fetcher.step = FetcherStep::GetTileT1;
    }
}
