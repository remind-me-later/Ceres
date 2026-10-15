//! Where the CGB up to revision C differs from the pixel pipeline's own
//! access timing (which the other models follow), as gambatte's CGB-C tests
//! measured it. Each rule is an offset from an event of the pipeline or of
//! the LCD's line counter (`timing.rs`):
//!
//! - the CPU sees mode 3's VRAM lock 2 CPU cycles after the pipeline takes
//!   it, and the palette lock a dot after that;
//! - the palettes unlock 3 half dots and 2 CPU cycles after HBlank begins;
//! - OAM writes for the next line lock only once LY has changed;
//! - an HBlank HDMA can run from 3 half dots after HBlank begins to the
//!   line reset that ends the HBlank (`timing.rs`);
//! - the object search samples each entry's OBJ size twice, a dot apart, a
//!   little after the pipeline looks at the entry;
//! - the WY check runs twice at the line end, on the line counter's edges,
//!   and sees WY and LCDC 2 CPU cycles after a write.
//!
//! On the first line after the LCD is turned on, the mode 3 and OAM locks
//! are the pipeline's own.

use {
    super::{
        LAST_LINE, LINES, State, model_ge_cgb_d,
        timing::{LINE_END_H, LINE_RESET_H, VBLANK_H},
    },
    crate::ppu::{LCDC_WIN_EN_B, Ppu},
};

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
    /// WY and LCDC as the WY check sees them.
    wy: LateReg,
    lcdc: LateReg,
    /// The LCDC write in progress went through `cgb_c_write_lcdc`.
    lcdc_tracked: bool,
    // Half dots since a pipeline event, the event's own half dot counting
    // as 1:
    /// since the pipeline locked VRAM for mode 3 (0: unlocked);
    vram_lock_age: u16,
    /// since HBlank began (`None` from LY's change to the line's HBlank);
    hblank_age: Option<u16>,
    /// since the line's OAM scan began.
    scan_age: Option<u16>,
}

impl CgbC {
    /// The LCD was turned on.
    pub(super) const fn restart(&mut self) {
        self.hblank_age = None;
    }
}

/// An age a half dot later.
const fn older(age: Option<u16>) -> Option<u16> {
    match age {
        Some(age) => Some(age.saturating_add(1)),
        None => None,
    }
}

/// The palettes and an HBlank HDMA wait this many half dots after HBlank
/// begins (plus 2 CPU cycles for the palettes).
const HBLANK_WAIT: i64 = 3;
/// Half dots from the start of the OAM scan to LY's change, plus the event's
/// own half dot.
const SCAN_START_TO_LY: i64 = 8;

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

    /// The line and half dot on the LCD's line counter (`timing.rs`).
    fn clock(&self) -> (u8, i64) {
        let (line, h) = self.d.timing.position();
        (line, i64::from(h))
    }

    /// Advances a half dot (after the timing unit): runs the WY checks.
    pub(super) fn cgb_c_unit(&mut self) {
        if !self.cgb_c_timing() {
            return;
        }
        self.d.cgb_c.now += 1;
        let (line, h) = self.clock();
        if h == 0 {
            self.d.cgb_c.hblank_age = None;
        }
        if !self.d.timing.is_on() {
            return;
        }
        // The WY checks at the end of a line: half an M-cycle before the
        // line-end pulse against the line, at the VBlank latch's edge
        // against the next one.
        let ly = if h == i64::from(LINE_END_H) - 4 {
            line
        } else if h == i64::from(VBLANK_H) {
            if line == LAST_LINE { 0 } else { line + 1 }
        } else {
            return;
        };
        let c = &mut self.d.cgb_c;
        c.wy.settle(c.now);
        c.lcdc.settle(c.now);
        if ly != 0 && c.lcdc.val & LCDC_WIN_EN_B != 0 && c.wy.val == ly {
            self.d.window.wy_triggered = true;
        }
    }

    /// A half dot ran (after the pipeline's state machine): counts the time
    /// since the pipeline's lock events.
    pub(super) const fn cgb_c_after_unit(&mut self) {
        if !self.cgb_c_timing() {
            return;
        }
        let locked = self.d.cpu.vram_read_blocked && self.d.cpu.vram_write_blocked;
        let c = &mut self.d.cgb_c;
        c.vram_lock_age = if locked {
            c.vram_lock_age.saturating_add(1)
        } else {
            0
        };
        c.hblank_age = older(c.hblank_age);
        c.scan_age = older(c.scan_age);
    }

    /// 2 CPU cycles in half dots: how late the CGB-C's CPU sees mode 3's
    /// VRAM lock.
    const fn lock_delay(&self) -> u16 {
        if self.double_speed() { 2 } else { 4 }
    }

    /// VRAM as the CPU sees it locked: 2 cycles after the pipeline.
    pub(in crate::ppu) const fn cgb_c_vram_blocked(&self, pipeline: bool) -> bool {
        if !self.cgb_c_timing() || !self.d.line_clock_valid() {
            return pipeline;
        }
        pipeline && self.d.cgb_c.vram_lock_age > self.lock_delay()
    }

    /// The palettes as the CPU sees them locked: a dot after VRAM, until 2
    /// cycles and 3 half dots after HBlank begins.
    pub(in crate::ppu) fn cgb_c_palettes_blocked(&self, pipeline: bool) -> bool {
        if !self.cgb_c_timing() || !self.d.timing.is_on() {
            return pipeline;
        }
        let c = &self.d.cgb_c;
        if let Some(age) = c.hblank_age
            && self.d.current_line < LINES
        {
            return i64::from(age) <= HBLANK_WAIT + cc(self.double_speed(), 2);
        }
        if !self.d.line_clock_valid() {
            return pipeline;
        }
        pipeline || c.vram_lock_age > self.lock_delay() + 2
    }

    /// OAM as CPU writes see it locked: for the next line only once LY has
    /// changed.
    pub(in crate::ppu) const fn cgb_c_oam_write_blocked(&self, pipeline: bool) -> bool {
        if !self.cgb_c_timing() || !self.d.line_clock_valid() {
            return pipeline;
        }
        pipeline && !matches!(self.d.state(), State::LineOamWriteLock | State::LineLy)
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

    /// The HBlank HDMA period, `offset` CPU cycles ahead: from 3 half dots
    /// after HBlank begins to the line reset; `None` before HBlank.
    pub(crate) fn cgb_c_hdma_period(&self, offset: i64) -> Option<bool> {
        if !self.cgb_c_timing() || !self.d.timing.is_on() {
            return None;
        }
        let (line, h) = self.clock();
        if line >= LINES {
            return Some(false);
        }
        let age = i64::from(self.d.cgb_c.hblank_age?);
        let ahead = cc(self.double_speed(), offset);
        Some(h + ahead < i64::from(LINE_RESET_H) && age + ahead > HBLANK_WAIT)
    }

    /// The line cycle (dots since LY changed) after which the object search
    /// sees an `OBJ_SIZE` write landing now, `units_early` before a read's
    /// point; `-1` if it lands before the line. The search samples entry `i`
    /// at line cycles `2 * i` and `2 * i + 1` (`objects.rs`).
    pub(in crate::ppu) fn cgb_c_size_change_cycle(&self, units_early: i64) -> Option<i64> {
        if matches!(
            self.d.state(),
            State::LineOamWriteLock | State::LineLy | State::OamScanStart
        ) {
            return Some(-1);
        }
        let age = i64::from(self.d.cgb_c.scan_age?);
        // The write reaches the search 2 cycles later.
        let h = age - SCAN_START_TO_LY + units_early + cc(self.double_speed(), 2);
        Some(if h < 0 { -1 } else { (h + 1) / 2 })
    }

    /// HBlank began.
    pub(super) const fn cgb_c_hblank(&mut self) {
        self.d.cgb_c.hblank_age = Some(0);
    }

    /// The line's OAM scan began.
    pub(super) const fn cgb_c_scan_start(&mut self) {
        self.d.cgb_c.scan_age = Some(0);
    }
}
