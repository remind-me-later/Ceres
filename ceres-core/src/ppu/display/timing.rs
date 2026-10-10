//! The LCD's line timing and STAT interrupt, structured like the DMG's
//! circuit (as traced from the die in GateBoy).
//!
//! One interrupt line is the OR of four sources, each gated by its STAT
//! enable, and IF takes its rising edges:
//!
//! - LY=LYC: the comparison, sampled once per M-cycle (`ROPO`);
//! - OAM: the line-end pulse (`RUTU`), while not in VBlank;
//! - VBlank: latched at the line end when LY reaches 144 (`POPU`);
//! - HBlank: set when mode 3 ends (`WODU`), cleared by the line reset that
//!   follows the line end (`ATEJ`), while not in VBlank. The line reset is
//!   skipped when LY becomes 144, so HBlank holds into VBlank.
//!
//! What gambatte models as separate events with suppression rules (an event
//! is lost when another source already holds the line) falls out of the OR.
//!
//! For now this runs as a shadow of the live model (the `debug` feature): it
//! requests nothing and only records its interrupts in the PPU trace, so the
//! two can be compared.
//!
//! Times are in half dots on gambatte's line clock (`h`, 0 where LY changes
//! for a CPU read, 912 per line), so they compare directly with `gstat.rs`.

use {
    super::{LAST_LINE, LINE_LENGTH, LINES},
    crate::{
        interrupts::Interrupts,
        ppu::{
            Ppu, STAT_IF_HBLANK_B, STAT_IF_LYC_B, STAT_IF_OAM_B, STAT_IF_VBLANK_B, STAT_LYC_B,
            STAT_MODE_B,
        },
    },
};

/// Half dots per line.
const LINE_H: i32 = 2 * LINE_LENGTH;
/// The line-end pulse rises 8 half dots (an M-cycle) before the line and
/// lasts an M-cycle.
const LINE_END_H: i32 = LINE_H - 8;
/// The line reset that clears the HBlank gate, just after the pulse.
const LINE_RESET_H: i32 = LINE_END_H + 2;
/// VBlank is latched half an M-cycle after the pulse.
const VBLANK_H: i32 = LINE_END_H + 4;
/// LY=LYC is sampled once per M-cycle, at this phase.
const LY_MATCH_PHASE: i32 = 4;
/// On line 153 LY resets to 0 early, at this point of the line (after the
/// LY=LYC sample there).
const LINE153_LY_RESET_H: i32 = 4;
/// A written LYC reaches the comparison this many half dots after the
/// write, which then follows it at once (LY changes only at its samples).
const LYC_WRITE_LATENCY: u8 = 3;

const LAST_VISIBLE_LINE: u8 = LINES - 1;

/// The enables that STAT's bits 3-6 hold.
const STAT_ENABLES: u8 = STAT_IF_HBLANK_B | STAT_IF_VBLANK_B | STAT_IF_OAM_B | STAT_IF_LYC_B;

#[expect(
    clippy::struct_excessive_bools,
    reason = "Independent signals of the circuit"
)]
#[derive(Clone, Default)]
pub(super) struct LcdTiming {
    on: bool,
    /// The line on the clock and the half dot in it.
    line: u8,
    h: i32,

    /// LY as the comparator sees it.
    ly: u8,
    /// `RUTU`: the line-end pulse.
    line_end: bool,
    /// `POPU`: VBlank.
    vblank: bool,
    /// `WODU`: the HBlank gate.
    hblank: bool,
    /// Mode 3 ended in the last unit: the HBlank gate rises in this one.
    hblank_pending: bool,
    /// `ROPO`: LY=LYC, as last sampled.
    ly_match: bool,

    /// STAT's enable bits and LYC.
    enables: u8,
    lyc: u8,
    /// `SUKO`: the interrupt line.
    stat_line: bool,
    /// A written LYC and the half dots until it reaches the comparison.
    lyc_pending: Option<(u8, u8)>,
    /// The line rose in the last unit.
    unit_edge: bool,
    /// The unit requests the interrupts and `gstat`'s go nowhere (an
    /// experiment: `Gb::set_timing_live`).
    live: bool,
}

impl LcdTiming {
    /// The interrupt line for these enables.
    const fn line_for(&self, enables: u8) -> bool {
        (enables & STAT_IF_LYC_B != 0 && self.ly_match)
            || (enables & STAT_IF_OAM_B != 0 && self.line_end && !self.vblank)
            || (enables & STAT_IF_VBLANK_B != 0 && self.vblank)
            || (enables & STAT_IF_HBLANK_B != 0 && self.hblank && !self.vblank)
    }

    /// Recomputes the interrupt line: `true` on a rising edge.
    const fn update_line(&mut self) -> bool {
        let line = self.line_for(self.enables);
        let rose = line && !self.stat_line;
        self.stat_line = line;
        rose
    }

    /// Advances a half dot: `true` if the interrupt line rose.
    const fn unit(&mut self) -> bool {
        self.h += 1;
        if self.h == LINE_H {
            self.h = 0;
            self.line = if self.line == LAST_LINE {
                0
            } else {
                self.line + 1
            };
        }
        if !self.on {
            return false;
        }

        if self.hblank_pending {
            self.hblank_pending = false;
            self.hblank = true;
        }
        match self.h {
            0 => self.line_end = false,
            LINE_END_H => {
                self.line_end = true;
                self.ly = if self.line == LAST_LINE {
                    0
                } else {
                    self.line + 1
                };
            }
            // No line reset as LY becomes 144: HBlank holds into VBlank.
            LINE_RESET_H if self.line != LAST_VISIBLE_LINE => self.hblank = false,
            _ => {}
        }
        let mut rose = false;
        if let Some((lyc, left)) = self.lyc_pending {
            if left <= 1 {
                self.lyc = lyc;
                self.lyc_pending = None;
                self.ly_match = self.ly == self.lyc;
                rose |= self.update_line();
            } else {
                self.lyc_pending = Some((lyc, left - 1));
            }
        }
        if self.h.rem_euclid(8) == LY_MATCH_PHASE {
            self.ly_match = self.ly == self.lyc;
        }
        if self.h == LINE153_LY_RESET_H && self.line == LAST_LINE {
            self.ly = 0;
        }
        rose |= self.update_line();
        // VBlank is latched on the same clock edge as LY=LYC, a gate later:
        // LY=LYC dropping as VBlank starts leaves the line low in between.
        if self.h == VBLANK_H {
            match self.line {
                LAST_VISIBLE_LINE => self.vblank = true,
                LAST_LINE => self.vblank = false,
                _ => {}
            }
            rose |= self.update_line();
        }
        rose
    }
}

/// A CPU read sees the signals this many half dots after the unit's clock.
const READ_LATENCY: u8 = 4;

/// The OAM scan (mode 2) as reads see it: from a little after the line
/// reset, for 77 dots.
const SCAN_START_H: i32 = LINE_RESET_H + 4;
const SCAN_END_H: i32 = 2 * 77 + READ_LATENCY as i32;

impl LcdTiming {
    /// The unit `n` half dots ahead.
    fn ahead(&self, n: u8) -> Self {
        let mut t = self.clone();
        for _ in 0..n {
            t.unit();
        }
        t
    }

    /// STAT's mode as a read sees it; `pipeline` is the mode the drawing
    /// pipeline is in, which tells mode 3 from mode 0.
    const fn read_mode(&self, pipeline: u8) -> u8 {
        let next_visible = self.line < LAST_VISIBLE_LINE || self.line == LAST_LINE;
        let scanning =
            (self.h >= SCAN_START_H && next_visible) || (self.h < SCAN_END_H && self.line < LINES);
        let visible =
            self.line < LINES && !(self.line == LAST_VISIBLE_LINE && self.h >= SCAN_START_H);
        // VBlank shows once the clock has turned over to line 144.
        if self.vblank && self.line >= LINES {
            1
        } else if scanning {
            2
        } else if visible && pipeline != 0 {
            3
        } else {
            0
        }
    }
}

impl Ppu {
    /// LY as a CPU read sees it; `None` where the line clock does not hold
    /// (the first line after the LCD is turned on, a cut off line).
    pub(in crate::ppu) fn timing_read_ly(&self) -> Option<u8> {
        if !self.d.line_clock_valid() {
            return None;
        }
        Some(self.d.timing.ahead(READ_LATENCY).ly)
    }

    /// STAT's mode and LY=LYC flag as a CPU read sees them, over `stat`
    /// (the enables and the pipeline's mode). The flag comes from a latch
    /// (`RUPO`) that follows the synced comparison a half dot late, and
    /// drops as soon as LY changes.
    pub(in crate::ppu) fn timing_read_stat(&self, stat: u8) -> Option<u8> {
        if !self.d.line_clock_valid() {
            return None;
        }
        let flag_view = self.d.timing.ahead(READ_LATENCY - 1);
        let lyc_flag = flag_view.ly_match && flag_view.ly == flag_view.lyc;
        let t = flag_view.ahead(1);
        let mode = t.read_mode(stat & STAT_MODE_B);
        Some((stat & !(STAT_MODE_B | STAT_LYC_B)) | mode | if lyc_flag { STAT_LYC_B } else { 0 })
    }

    /// Advances the shadow timing unit by a half dot; its interrupt is
    /// recorded with the unit's (`take_timing_edge`).
    pub(super) const fn timing_unit(&mut self, ints: &mut Interrupts) {
        let rose = self.d.timing.unit();
        self.d.timing.unit_edge = rose;
        if rose && self.timing_live() {
            ints.request_lcd();
        }
    }

    /// The line rose outside a unit (a register write).
    fn timing_rose(&mut self, ints: &mut Interrupts) {
        self.trace_shadow_lcd_interrupt();
        if self.timing_live() {
            ints.request_lcd();
        }
    }

    /// Whether the interrupt line rose in the last unit.
    pub(in crate::ppu) const fn take_timing_edge(&mut self) -> bool {
        let rose = self.d.timing.unit_edge;
        self.d.timing.unit_edge = false;
        rose
    }

    /// Mode 3 ended: the HBlank gate rises in the next unit.
    pub(super) const fn timing_mode3_end(&mut self) {
        self.d.timing.hblank_pending = true;
    }

    /// The LCD was turned on: the line clock starts at line 0.
    pub(in crate::ppu) fn timing_lcd_on(&mut self, ints: &mut Interrupts) {
        let t = &mut self.d.timing;
        t.on = true;
        t.line = 0;
        t.h = 0;
        t.ly = 0;
        t.line_end = false;
        t.vblank = false;
        t.hblank = false;
        t.hblank_pending = false;
        t.ly_match = t.lyc == 0;
        if t.update_line() {
            self.timing_rose(ints);
        }
    }

    /// The LCD was turned off: LY=LYC keeps its last value.
    pub(in crate::ppu) const fn timing_lcd_off(&mut self) {
        let t = &mut self.d.timing;
        t.on = false;
        t.ly = 0;
        t.line_end = false;
        t.vblank = false;
        t.hblank = false;
        t.hblank_pending = false;
        t.update_line();
    }

    /// Checks the clock against the start of a line on `line_clock` (as
    /// `gstat_line_end` does).
    pub(super) const fn timing_line_end(&mut self, line: u8) {
        self.d.timing.line = line;
        self.d.timing.h = LINE_H - 19;
    }

    /// The line and half dot on the unit's clock.
    pub(in crate::ppu) const fn timing_position(&self) -> (u8, i32) {
        (self.d.timing.line, self.d.timing.h)
    }

    /// The unit requests the STAT interrupts (`Gb::set_timing_live`).
    pub(in crate::ppu) const fn timing_live(&self) -> bool {
        self.d.timing.live
    }

    /// See `Gb::set_timing_live`.
    pub(crate) const fn set_timing_live(&mut self, live: bool) {
        self.d.timing.live = live;
    }

    /// A STAT write. On the DMG the write briefly sets every enable: any
    /// active source raises the interrupt (with the LCD off, a set LY=LYC).
    pub(in crate::ppu) fn timing_write_stat(&mut self, val: u8, ints: &mut Interrupts) {
        let t = &mut self.d.timing;
        let glitch = t.line_for(STAT_ENABLES) && !t.stat_line;
        t.stat_line |= glitch;
        t.enables = val & STAT_ENABLES;
        if t.update_line() || glitch {
            self.timing_rose(ints);
        }
    }

    /// An LYC write (see `LYC_WRITE_LATENCY`).
    pub(in crate::ppu) const fn timing_write_lyc(&mut self, val: u8) {
        let t = &mut self.d.timing;
        if t.on {
            t.lyc_pending = Some((val, LYC_WRITE_LATENCY));
        } else {
            t.lyc = val;
            t.lyc_pending = None;
        }
    }
}
