//! What the CGB up to revision C does at fixed points of the line clock, as
//! gambatte measured it: when the CPU may access VRAM, the palettes and
//! OAM around mode 3 and the line end, when an HBlank HDMA runs, when an
//! OBJ size write reaches the object search, and when the window's WY check
//! sees WY and LCDC.
//!
//! These follow the timing unit's clock (`timing.rs`: `h` in half dots, 912
//! per line, 0 where LY changes for a read). On the other models the
//! pixel pipeline's own state decides.

use {
    super::{LAST_LINE, LINE_LENGTH, LINES, State, model_ge_cgb_d},
    crate::ppu::{LCDC_WIN_EN_B, Ppu},
};

/// Dots and half dots per line.
const LINE: i64 = LINE_LENGTH as i64;
const LINE_H: i64 = 2 * LINE;

const LAST_VISIBLE_LINE: u8 = LINES - 1;

/// The line cycle from which the CGB-C locks VRAM in mode 3.
pub(in crate::ppu) const VRAM_LOCK_CYCLE: i64 = 79;
/// The line cycle from which it locks the palettes.
pub(in crate::ppu) const PALETTE_LOCK_CYCLE: i64 = 80;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Pending {
    after: u64,
    val: u8,
}

/// A register as the WY check sees it: a write is in place for the checks
/// after `Pending::after`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct LateReg {
    val: u8,
    pending: [Option<Pending>; 4],
}

impl LateReg {
    /// A write at `now` that checks up to `after` do not see.
    fn write(&mut self, now: u64, after: u64, val: u8) {
        // Earlier writes whose window closed are in place.
        self.settle(now);
        if let Some(slot) = self.pending.iter_mut().find(|p| p.is_none()) {
            *slot = Some(Pending { after, val });
        } else {
            self.val = val;
        }
    }

    /// The writes in place at `now`.
    fn settle(&mut self, now: u64) {
        for slot in &mut self.pending {
            if let Some(p) = *slot
                && now > p.after
            {
                self.val = p.val;
                *slot = None;
            }
        }
    }

    const fn set(&mut self, val: u8) {
        self.val = val;
        self.pending = [None; 4];
    }
}

#[derive(Clone, Debug, Default)]
pub(super) struct CgbC {
    /// Half dots since power on.
    now: u64,
    /// Mode 3 of the current line is over.
    hblank: bool,
    /// `h` when HBlank began.
    hblank_h: i64,
    /// WY and LCDC as the WY check sees them.
    wy: LateReg,
    lcdc: LateReg,
    /// The LCDC write in progress went through `cgb_c_write_lcdc`.
    lcdc_tracked: bool,
}

/// `n` CPU cycles in half dots.
const fn cc(ds: bool, n: i64) -> i64 {
    if ds { n } else { 2 * n }
}

impl Ppu {
    /// The CGB follows these windows (up to revision C).
    #[must_use]
    pub(crate) const fn cgb_c_timing(&self) -> bool {
        self.is_cgb_hardware() && !model_ge_cgb_d(self.model)
    }

    /// The line and half dot on the timing unit's clock.
    fn clock(&self) -> (u8, i64) {
        let (line, h) = self.d.timing.position();
        (line, i64::from(h))
    }

    /// The line cycle (dots) of the clock: gambatte's `lyCounter`, which
    /// counts CPU cycles.
    fn line_cycles(&self) -> i64 {
        let ds = self.double_speed();
        let left = LINE_H - self.clock().1;
        let left = if ds { left } else { left / 2 };
        LINE - (left >> u32::from(ds))
    }

    /// Advances a half dot (after the timing unit): runs the WY checks.
    pub(super) fn cgb_c_unit(&mut self) {
        if !self.cgb_c_timing() {
            return;
        }
        self.d.cgb_c.now += 1;
        let (line, h) = self.clock();
        if h == 0 {
            self.d.cgb_c.hblank = false;
        }
        if !self.d.timing.is_on() {
            return;
        }
        // The WY checks at the end of a line: at line cycle 450 against the
        // line, at 454 against the next one.
        let ly = match h {
            900 => line,
            908 if line == LAST_LINE => 0,
            908 => line + 1,
            _ => return,
        };
        let c = &mut self.d.cgb_c;
        c.wy.settle(c.now);
        c.lcdc.settle(c.now);
        if ly != 0 && c.lcdc.val & LCDC_WIN_EN_B != 0 && c.wy.val == ly {
            self.d.window.wy_triggered = true;
        }
    }

    /// The VRAM and palette locks at the start of mode 3 follow the line
    /// clock: `Some(false)` before line cycle `threshold`, `Some(true)`
    /// after it while the PPU is still in its object search; `None` leaves
    /// it to the PPU's own lock.
    pub(in crate::ppu) fn cgb_c_mode3_lock(&self, threshold: i64) -> Option<bool> {
        let (line, _) = self.clock();
        if !self.cgb_c_timing()
            || !self.d.timing.is_on()
            || !self.d.line_clock_valid()
            || line >= LINES
        {
            return None;
        }
        if self.line_cycles() + i64::from(self.double_speed()) < threshold {
            return Some(false);
        }
        matches!(
            self.d.state(),
            State::OamScanStart
                | State::OamScanNext
                | State::OamScanObject
                | State::Mode3PalettesLock
        )
        .then_some(true)
    }

    /// The OAM lock at the end of a line follows the line clock:
    /// `Some(blocked)` in the last dots of the line, unlocked before them
    /// once mode 3 is over, `None` elsewhere.
    pub(in crate::ppu) fn cgb_c_oam_lock(&self, write: bool) -> Option<bool> {
        if !self.cgb_c_timing() || !self.d.timing.is_on() || !self.d.line_clock_valid() {
            return None;
        }
        let (line, _) = self.clock();
        let left = if write {
            4
        } else {
            4 - i64::from(self.double_speed())
        };
        if self.line_cycles() + left >= LINE {
            Some(!(LAST_VISIBLE_LINE..LAST_LINE).contains(&line))
        } else if line >= LINES || self.d.cgb_c.hblank {
            Some(false)
        } else {
            None
        }
    }

    /// The line cycle (relative to `line`) after which the object search
    /// sees an OBJ_SIZE write landing now, `units_early` before a read's
    /// point; `-1` if it lands before the line.
    pub(in crate::ppu) fn cgb_c_size_change_cycle(
        &self,
        line: u8,
        units_early: i64,
    ) -> Option<i64> {
        let ds = self.double_speed();
        let (clock_line, h) = self.clock();
        let mut h = h + units_early + cc(ds, 2);
        if clock_line != line {
            // Only a write in the last dots of the line before counts.
            let next = if clock_line == LAST_LINE {
                0
            } else {
                clock_line + 1
            };
            if next != line {
                return None;
            }
            h -= LINE_H;
        }
        if h < 0 {
            return Some(-1);
        }
        let left = LINE_H - h;
        let t = if ds { left } else { left / 2 };
        Some(LINE - (t >> u32::from(ds)))
    }

    /// A WY write: the WY checks see it 2 cycles later.
    pub(in crate::ppu) fn cgb_c_write_wy(&mut self, val: u8) {
        let ds = self.double_speed();
        let on = self.d.timing.is_on();
        let c = &mut self.d.cgb_c;
        if on {
            c.wy.write(c.now, c.now + cc(ds, 2).cast_unsigned(), val);
        } else {
            c.wy.set(val);
        }
    }

    /// An LCDC write `units_early` before a read's point: the WY checks see
    /// it 2 cycles after that point.
    pub(crate) fn cgb_c_write_lcdc(&mut self, val: u8, units_early: i64) {
        let ds = self.double_speed();
        let c = &mut self.d.cgb_c;
        let after = c.now.cast_signed() + units_early + cc(ds, 2);
        c.lcdc.write(c.now, after.cast_unsigned(), val);
        c.lcdc_tracked = true;
    }

    /// LCDC writes that do not go through `cgb_c_write_lcdc` land at once.
    pub(in crate::ppu) const fn cgb_c_lcdc_tracked(&self) -> bool {
        self.d.cgb_c.lcdc_tracked
    }

    /// The LCDC write is complete.
    pub(crate) const fn cgb_c_lcdc_write_done(&mut self) {
        self.d.cgb_c.lcdc_tracked = false;
    }

    pub(in crate::ppu) const fn cgb_c_set_lcdc(&mut self, val: u8) {
        self.d.cgb_c.lcdc.set(val);
    }

    /// An HBlank HDMA enabled now starts with the current HBlank (gambatte's
    /// `isHdmaPeriod(cc + 4)`); `None` before mode 0 began.
    pub(crate) fn cgb_c_hdma_enable_in_hblank(&self) -> Option<bool> {
        self.cgb_c_hdma_period(4)
    }

    /// gambatte's `isHdmaPeriod(cc + offset)`; `None` before mode 0 began.
    pub(crate) fn cgb_c_hdma_period(&self, offset: i64) -> Option<bool> {
        if !self.cgb_c_timing() || !self.d.timing.is_on() {
            return None;
        }
        let (line, h) = self.clock();
        if line >= LINES {
            return Some(false);
        }
        let c = &self.d.cgb_c;
        if !c.hblank {
            return None;
        }
        let ds = self.double_speed();
        let at = h + cc(ds, offset);
        Some(at + cc(ds, 3 + 3 * i64::from(ds)) < LINE_H && at >= c.hblank_h + 3)
    }

    /// HBlank began.
    pub(super) fn cgb_c_hblank(&mut self) {
        let h = self.clock().1;
        self.d.cgb_c.hblank = true;
        self.d.cgb_c.hblank_h = h;
    }

    /// The palettes become accessible 2 cycles after mode 0 begins on
    /// gambatte's clock (`m0Time + 2`); `None` outside HBlank.
    pub(in crate::ppu) fn cgb_c_palettes_unlocked(&self) -> Option<bool> {
        let (line, h) = self.clock();
        let c = &self.d.cgb_c;
        if !self.cgb_c_timing() || !self.d.timing.is_on() || !c.hblank || line >= LINES {
            return None;
        }
        // Mode 0 begins (`m0Time`) 3 units after HBlankStart.
        Some(h >= c.hblank_h + 3 + cc(self.double_speed(), 2))
    }
}
