//! The CGB's STAT interrupt, after gambatte's event model.
//!
//! Each source raises the interrupt as a separate event at a fixed point of
//! the line (mode 0 at the end of mode 3), and an event is suppressed when
//! the line is already held by another source. The registers the events see
//! are snapshots that a late write does not reach (gambatte's `LycIrq` and
//! `MStatIrqEvent`).
//!
//! Times are in half dots on gambatte's clock: `h` counts from the point
//! where LY changes, which a CPU read sees 19 units after the line starts
//! on `line_clock`.

use {
    super::{LAST_LINE, LINE_LENGTH, LINES, LINES_PER_FRAME, State, model_ge_cgb_d},
    crate::{
        interrupts::Interrupts,
        ppu::{
            LCDC_WIN_EN_B, Ppu, STAT_IF_HBLANK_B, STAT_IF_LYC_B, STAT_IF_OAM_B, STAT_IF_VBLANK_B,
        },
    },
};

/// Dots per line and per frame.
const LINE: i64 = LINE_LENGTH as i64;
const FRAME: i64 = LINES_PER_FRAME as i64 * LINE;
/// Half dots per line and per frame.
const LINE_H: i64 = 2 * LINE;
const FRAME_H: i64 = 2 * FRAME;
const DISABLED: u64 = u64::MAX;

const LAST_VISIBLE_LINE: u8 = LINES - 1;

/// Line cycles (dots) of the mode 2 events: 4 dots before the line, 2
/// before line 0.
const M2_LINE_CYCLE: i64 = 452;
const M2_LINE_CYCLE_LY0: i64 = 454;
const LAST_M2_FC: i64 = LAST_VISIBLE_LINE as i64 * LINE + M2_LINE_CYCLE;

/// The line after `ly` (the last one wraps to 0).
const fn next_ly(ly: u8) -> u8 {
    if ly == LAST_LINE { 0 } else { ly + 1 }
}
const LY0_M2_FC: i64 = LAST_LINE as i64 * LINE + M2_LINE_CYCLE_LY0;
/// The VBlank event: 2 dots before line 144.
const M1_FC: i64 = LINES as i64 * LINE - 2;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Pending {
    after: u64,
    val: u8,
}

/// A register snapshot that writes reach only if no event comes first:
/// a write is in place for the events after `Pending::after`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct LateReg {
    val: u8,
    pending: [Option<Pending>; 4],
}

impl LateReg {
    /// A write at `now` that events up to `after` do not see.
    fn write(&mut self, now: u64, after: u64, val: u8) {
        // Earlier writes whose window closed without an event are in place.
        self.settle(now);
        if let Some(slot) = self.pending.iter_mut().find(|p| p.is_none()) {
            *slot = Some(Pending { after, val });
        } else {
            self.val = val;
        }
    }

    /// The writes in place for an event at `now`.
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

    /// An event at `now` takes the writes in place and then `val`.
    fn event(&mut self, now: u64) -> u8 {
        self.settle(now);
        let seen = self.val;
        self.pending = [None; 4];
        seen
    }

    const fn set(&mut self, val: u8) {
        self.val = val;
        self.pending = [None; 4];
    }
}

#[expect(
    clippy::struct_excessive_bools,
    reason = "Independent event flags, as in gambatte"
)]
#[derive(Clone, Debug)]
pub struct GStat {
    /// The clock: half dots since the LCD was turned on, and the position
    /// in the line.
    g: u64,
    ly: u8,
    h: i64,

    /// STAT as the LCD sees it (`statReg_`).
    stat: u8,
    cgb: bool,

    // `LycIrq`.
    lyc_time: u64,
    lyc_src: u8,
    lyc_stat_src: u8,
    lyc_reg: u8,
    lyc_stat: u8,

    // `MStatIrqEvent`.
    m_lyc: LateReg,
    m_stat: LateReg,

    m2_time: u64,
    oneshot_time: u64,
    /// The mode 0 event is scheduled.
    m0_scheduled: bool,
    /// Mode 3 ended in the last unit: the mode 0 event follows.
    m0_pending: bool,
    /// The mode 0 event of the current line has passed.
    m0_done: bool,
    /// Mode 3 of the current line is over.
    hblank: bool,
    /// `h` when HBlank began.
    hblank_h: i64,
    /// WY and LCDC as gambatte's window checks see them.
    wy: LateReg,
    lcdc: LateReg,
    /// The LCDC write in progress went through `gstat_write_lcdc`.
    lcdc_tracked: bool,
    lcd_on: bool,
}

impl Default for GStat {
    fn default() -> Self {
        Self {
            g: 0,
            ly: 0,
            h: 0,
            stat: 0,
            cgb: true,
            lyc_time: DISABLED,
            lyc_src: 0,
            lyc_stat_src: 0,
            lyc_reg: 0,
            lyc_stat: 0,
            m_lyc: LateReg::default(),
            m_stat: LateReg::default(),
            m2_time: DISABLED,
            oneshot_time: DISABLED,
            m0_scheduled: false,
            m0_pending: false,
            m0_done: false,
            hblank: false,
            hblank_h: 0,
            wy: LateReg::default(),
            lcdc: LateReg::default(),
            lcdc_tracked: false,
            lcd_on: false,
        }
    }
}

impl GStat {
    /// `n` CPU cycles on gambatte's clock in half dots.
    const fn cc(ds: bool, n: i64) -> i64 {
        if ds { n } else { 2 * n }
    }

    /// gambatte's `lyCounter.time() - cc`, in CPU cycles.
    const fn time_to_next_ly(&self, ds: bool) -> i64 {
        let left = LINE_H - self.h;
        if ds { left } else { left / 2 }
    }

    /// The line cycle (dots) of the clock.
    const fn line_cycles(&self, ds: bool) -> i64 {
        LINE - (self.time_to_next_ly(ds) >> ds as u32)
    }

    const fn frame_pos(&self) -> i64 {
        self.ly as i64 * LINE_H + self.h
    }

    /// The time of the next frame cycle `fc` (dots), after now.
    const fn next_frame_cycle(&self, fc: i64) -> u64 {
        let mut delta = (2 * fc - self.frame_pos()).rem_euclid(FRAME_H);
        if delta == 0 {
            delta = FRAME_H;
        }
        self.g + delta.cast_unsigned()
    }

    /// The time of the next line cycle `lc` (dots), after now.
    const fn next_line_cycle(&self, lc: i64) -> u64 {
        let mut delta = (2 * lc - self.h).rem_euclid(LINE_H);
        if delta == 0 {
            delta = LINE_H;
        }
        self.g + delta.cast_unsigned()
    }

    /// Time from now to `t`, in CPU cycles.
    const fn until(&self, t: u64, ds: bool) -> i64 {
        if t == DISABLED {
            return i64::MAX;
        }
        let d = t.cast_signed() - self.g.cast_signed();
        if ds { d } else { d / 2 }
    }

    fn schedule_lyc(&self, stat: u8, lyc: u8) -> u64 {
        if stat & STAT_IF_LYC_B != 0 && lyc < LINES_PER_FRAME {
            self.next_frame_cycle(if lyc == 0 {
                i64::from(LAST_LINE) * LINE + 6
            } else {
                i64::from(lyc) * LINE - 2
            })
        } else {
            DISABLED
        }
    }

    /// `LycIrq::regChange` (CGB).
    fn lyc_reg_change(&mut self, stat: u8, lyc: u8, ds: bool) {
        let src_time = self.schedule_lyc(stat, lyc);
        self.lyc_stat_src = stat;
        self.lyc_src = lyc;
        self.lyc_time = self.lyc_time.min(src_time);
        let left = self.until(self.lyc_time, ds);
        if !self.cgb {
            if left > 4 || src_time != self.lyc_time {
                self.lyc_reg = lyc;
            }
            self.lyc_stat = stat;
            return;
        }
        if left > 6 + 4 * i64::from(ds) || (src_time != self.lyc_time && left > 2) {
            self.lyc_reg = lyc;
        }
        if left > 2 {
            self.lyc_stat = stat;
        }
    }

    fn lyc_reschedule(&mut self) {
        self.lyc_time = self
            .schedule_lyc(self.lyc_stat, self.lyc_reg)
            .min(self.schedule_lyc(self.lyc_stat_src, self.lyc_src));
    }

    /// gambatte's `mode2IrqSchedule`.
    fn schedule_m2(&self, stat: u8, ds: bool) -> u64 {
        if stat & STAT_IF_OAM_B == 0 {
            return DISABLED;
        }
        let fc = i64::from(self.ly) * LINE + self.line_cycles(ds);
        if (LAST_M2_FC..LY0_M2_FC).contains(&fc) || stat & STAT_IF_HBLANK_B != 0 {
            self.next_frame_cycle(LY0_M2_FC)
        } else {
            self.next_line_cycle(M2_LINE_CYCLE)
        }
    }

    /// The line LYC is compared with and the cycles until that changes
    /// (gambatte's `getLycCmpLy`).
    const fn lyc_cmp(&self, ds: bool) -> (u8, i64) {
        let ds_n = ds as i64;
        let line_time = LINE << ds_n;
        let left = self.time_to_next_ly(ds);
        if self.ly == LAST_LINE {
            let left = left - (line_time - 6 - 6 * ds_n);
            if left <= 0 {
                (0, left + line_time)
            } else {
                (LAST_LINE, left)
            }
        } else {
            let left = left - (2 + 2 * ds_n);
            if left <= 0 {
                (self.ly + 1, left + line_time)
            } else {
                (self.ly, left)
            }
        }
    }

    /// The mode 0 event of the current line is still to come.
    const fn m0_ahead(&self) -> bool {
        self.m0_scheduled && self.ly < LINES && !self.m0_done
    }

    fn stat_change_triggers_m0_lyc_or_m1(
        &self,
        old: u8,
        data: u8,
        lycperiod: bool,
        ds: bool,
    ) -> bool {
        let ly = self.ly;
        let left = self.time_to_next_ly(ds);
        let ds_n = i64::from(ds);
        // 2 dots before line 144: the VBlank event.
        if ly < LINES - 1 || (ly == LINES - 1 && left > 2 * (1 + ds_n)) {
            let close = if ly < LINES - 1 {
                4 + 4 * ds_n
            } else {
                4 + 2 * ds_n
            };
            if self.m0_ahead() || left <= close {
                return lycperiod && data & STAT_IF_LYC_B != 0;
            }
            if old & STAT_IF_HBLANK_B != 0 {
                return false;
            }
            return data & STAT_IF_HBLANK_B != 0 || (lycperiod && data & STAT_IF_LYC_B != 0);
        }
        if old & STAT_IF_VBLANK_B != 0 && (ly < LAST_LINE || left > 3 + 3 * ds_n) {
            return false;
        }
        (data & STAT_IF_VBLANK_B != 0 && (ly < LAST_LINE || left > 4 + 2 * ds_n))
            || (lycperiod && data & STAT_IF_LYC_B != 0)
    }

    fn stat_change_triggers_m2(&self, old: u8, data: u8, ds: bool) -> bool {
        if old & STAT_IF_OAM_B != 0 || data & (STAT_IF_OAM_B | STAT_IF_HBLANK_B) != STAT_IF_OAM_B {
            return false;
        }
        let left = self.time_to_next_ly(ds);
        let ds_n = i64::from(ds);
        match self.ly {
            0..LAST_VISIBLE_LINE => left <= (LINE - M2_LINE_CYCLE) * (1 + ds_n) && left > 2,
            LAST_VISIBLE_LINE => left <= (LINE - M2_LINE_CYCLE) * (1 + ds_n) && left > 4 + 2 * ds_n,
            LAST_LINE => left <= (LINE - M2_LINE_CYCLE_LY0) * (1 + ds_n) && left > 2,
            _ => false,
        }
    }

    /// gambatte's `statChangeTriggersStatIrqDmg`: on the DMG any STAT write
    /// raises the interrupt in HBlank, VBlank or with LY=LYC, unless the line
    /// was already held.
    const fn stat_change_triggers_dmg(&self, old: u8, ds: bool) -> bool {
        let lyc = self.lyc_cmp(ds).0 == self.lyc_reg;
        if self.ly < LINES {
            if !self.m0_done {
                return lyc && old & STAT_IF_LYC_B == 0;
            }
            return old & STAT_IF_HBLANK_B == 0 && !(lyc && old & STAT_IF_LYC_B != 0);
        }
        old & STAT_IF_VBLANK_B == 0 && !(lyc && old & STAT_IF_LYC_B != 0)
    }

    fn stat_change_triggers(&self, old: u8, data: u8, ds: bool) -> bool {
        if !self.cgb {
            return self.stat_change_triggers_dmg(old, ds);
        }
        if data & !old & (STAT_IF_LYC_B | STAT_IF_OAM_B | STAT_IF_VBLANK_B | STAT_IF_HBLANK_B) == 0
        {
            return false;
        }
        let (cmp_ly, cmp_left) = self.lyc_cmp(ds);
        let lycperiod = cmp_ly == self.lyc_reg && cmp_left > 2;
        if lycperiod && old & STAT_IF_LYC_B != 0 {
            return false;
        }
        self.stat_change_triggers_m0_lyc_or_m1(old, data, lycperiod, ds)
            || self.stat_change_triggers_m2(old, data, ds)
    }

    fn lyc_change_blocked_by_m0_or_m1(&self, data: u8, ds: bool) -> bool {
        if self.ly < LINES {
            return self.stat & STAT_IF_HBLANK_B != 0 && !self.m0_ahead() && data == self.ly;
        }
        self.stat & STAT_IF_VBLANK_B != 0
            && !(self.ly == LAST_LINE
                && self.time_to_next_ly(ds) <= 2 + 2 * i64::from(ds) + 2 * i64::from(self.cgb))
    }

    fn lyc_change_triggers(&self, old: u8, data: u8, ds: bool) -> bool {
        if self.stat & STAT_IF_LYC_B == 0
            || data >= LINES_PER_FRAME
            || self.lyc_change_blocked_by_m0_or_m1(data, ds)
        {
            return false;
        }
        let (mut cmp_ly, cmp_left) = self.lyc_cmp(ds);
        let cgb2 = 2 * i64::from(self.cgb);
        if cmp_left <= 4 + 4 * i64::from(ds) + cgb2 {
            if old == cmp_ly && cmp_left > cgb2 {
                // LY and LYC change together: the flag never goes low.
                return false;
            }
            cmp_ly = next_ly(cmp_ly);
        }
        data == cmp_ly
    }
}

impl Ppu {
    /// The CGB follows gambatte's model (up to revision C).
    #[must_use]
    pub const fn gambatte_stat(&self) -> bool {
        self.hw_cgb() && !model_ge_cgb_d(self.model)
    }

    /// The STAT interrupt and LY follow gambatte's model (its CGB, and its
    /// DMG, a DMG-B).
    #[must_use]
    pub fn gambatte_irq(&self) -> bool {
        self.gambatte_stat() || self.model == crate::Model::DmgB
    }

    /// Advances gambatte's clock by a unit and runs the events that are due.
    pub(super) fn gstat_unit(&mut self, ints: &mut Interrupts) {
        if !self.gambatte_irq() {
            return;
        }
        self.d.gstat.cgb = self.hw_cgb();
        let ds = self.double_speed();
        let g = &mut self.d.gstat;
        g.g += 1;
        g.h += 1;
        if g.h == LINE_H {
            g.h = 0;
            g.ly = next_ly(g.ly);
            g.m0_done = false;
            g.hblank = false;
        }
        if !g.lcd_on {
            return;
        }
        if self.hw_cgb() {
            self.gstat_window_checks();
        }
        self.gstat_events(ints, ds);
    }

    /// The window checks at the end of a line (gambatte's `weMaster`): at
    /// line cycle 450 against the line, at 454 against the next one.
    fn gstat_window_checks(&mut self) {
        let g = &mut self.d.gstat;
        let ly = match g.h {
            900 => g.ly,
            908 if g.ly == LAST_LINE => 0,
            908 => g.ly + 1,
            _ => return,
        };
        g.wy.settle(g.g);
        g.lcdc.settle(g.g);
        if ly != 0 && g.lcdc.val & LCDC_WIN_EN_B != 0 && g.wy.val == ly {
            self.d.window.wy_triggered = true;
        }
    }

    /// The STAT interrupt events due in this unit.
    fn gstat_events(&mut self, ints: &mut Interrupts, ds: bool) {
        let g = &mut self.d.gstat;
        if g.g == g.oneshot_time {
            g.oneshot_time = DISABLED;
            ints.request_lcd();
        }

        // VBlank (gambatte's mode 1 event).
        if i64::from(g.ly) * LINE_H + g.h == 2 * M1_FC {
            let m_stat = g.m_stat.event(g.g);
            let flag =
                g.stat & STAT_IF_VBLANK_B != 0 && m_stat & (STAT_IF_OAM_B | STAT_IF_HBLANK_B) == 0;
            g.m_stat.set(g.stat);
            if flag {
                ints.request_lcd();
            }
        }

        if g.g == g.lyc_time {
            let cmp_ly = next_ly(g.ly);
            let blocked = if g.lyc_reg <= LINES && g.lyc_reg > 0 {
                g.lyc_stat & STAT_IF_OAM_B != 0
            } else {
                g.lyc_stat & STAT_IF_VBLANK_B != 0
            };
            let flag = (g.lyc_stat | g.lyc_stat_src) & STAT_IF_LYC_B != 0
                && g.lyc_reg == cmp_ly
                && !blocked;
            g.lyc_reg = g.lyc_src;
            g.lyc_stat = g.lyc_stat_src;
            g.lyc_time = g.schedule_lyc(g.lyc_stat, g.lyc_reg);
            if flag {
                ints.request_lcd();
            }
        }

        if g.g == g.m2_time {
            let m_stat = g.m_stat.event(g.g);
            let m_lyc = g.m_lyc.event(g.g);
            let ly = if g.time_to_next_ly(ds) < 16 {
                next_ly(g.ly)
            } else {
                g.ly
            };
            let blocked_by_m1 = ly == 0 && m_stat & STAT_IF_VBLANK_B != 0;
            let blocked_by_lyc = m_stat & STAT_IF_LYC_B != 0 && ly.saturating_sub(1) == m_lyc;
            g.m_lyc.set(g.lyc_reg);
            g.m_stat.set(g.stat);
            let next = if g.stat & STAT_IF_HBLANK_B != 0 {
                FRAME
            } else if ly == 0 {
                LINE - (M2_LINE_CYCLE_LY0 - M2_LINE_CYCLE)
            } else if ly == LINES {
                LINE + LINE * (i64::from(LINES_PER_FRAME) - i64::from(LINES) - 1)
                    + M2_LINE_CYCLE_LY0
                    - M2_LINE_CYCLE
            } else {
                LINE
            };
            g.m2_time += (2 * next).cast_unsigned();
            if !blocked_by_m1 && !blocked_by_lyc {
                ints.request_lcd();
            }
        }

        if g.m0_pending {
            g.m0_pending = false;
            g.m0_done = true;
            if g.m0_scheduled {
                let m_stat = g.m_stat.event(g.g);
                let m_lyc = g.m_lyc.event(g.g);
                let flag = (g.stat | m_stat) & STAT_IF_HBLANK_B != 0
                    && (m_stat & STAT_IF_LYC_B == 0 || g.ly != m_lyc);
                g.m_lyc.set(g.lyc_reg);
                g.m_stat.set(g.stat);
                g.m0_scheduled = g.stat & STAT_IF_HBLANK_B != 0;
                if flag {
                    ints.request_lcd();
                }
            }
        }
    }

    /// The CGB's VRAM and palette locks at the start of mode 3 follow the
    /// line clock: `Some(false)` before line cycle `threshold`, `Some(true)`
    /// after it while the PPU is still in its object search; `None` leaves
    /// it to the PPU's own lock.
    pub(in crate::ppu) fn gstat_mode3_lock(&self, threshold: i64) -> Option<bool> {
        let g = &self.d.gstat;
        if !self.gambatte_stat() || !g.lcd_on || !self.d.line_clock_valid() || g.ly >= LINES {
            return None;
        }
        let ds = self.double_speed();
        if g.line_cycles(ds) + i64::from(ds) < threshold {
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

    /// The CGB's OAM lock at the end of a line follows the line clock:
    /// `Some(blocked)` in the last dots of the line, unlocked before them
    /// once mode 3 is over, `None` elsewhere.
    pub(in crate::ppu) fn gstat_oam_lock(&self, write: bool) -> Option<bool> {
        let g = &self.d.gstat;
        if !self.gambatte_stat() || !g.lcd_on || !self.d.line_clock_valid() {
            return None;
        }
        let ds = self.double_speed();
        let left = if write { 4 } else { 4 - i64::from(ds) };
        if g.line_cycles(ds) + left >= LINE {
            Some(!(LAST_VISIBLE_LINE..LAST_LINE).contains(&g.ly))
        } else if g.ly >= LINES || g.hblank {
            Some(false)
        } else {
            None
        }
    }

    /// The line cycle (gambatte's clock, relative to `line`) after which the
    /// object search sees an OBJ_SIZE write landing now, `units_early` before
    /// a read's point; `-1` if it lands before the line.
    pub(in crate::ppu) fn gstat_size_change_cycle(
        &self,
        line: u8,
        units_early: i64,
    ) -> Option<i64> {
        let g = &self.d.gstat;
        let ds = self.double_speed();
        let mut h = g.h + units_early + GStat::cc(ds, 2);
        if g.ly != line {
            // Only a write in the last dots of the line before counts.
            if next_ly(g.ly) != line {
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

    /// A WY write: gambatte's window checks see it 2 cycles later.
    pub(in crate::ppu) fn gstat_write_wy(&mut self, val: u8) {
        let ds = self.double_speed();
        let g = &mut self.d.gstat;
        if g.lcd_on {
            g.wy.write(g.g, g.g + GStat::cc(ds, 2).cast_unsigned(), val);
        } else {
            g.wy.set(val);
        }
    }

    /// An LCDC write `units_early` before a read's point: gambatte's window
    /// checks see it 2 cycles after that point.
    pub fn gstat_write_lcdc(&mut self, val: u8, units_early: i64) {
        let ds = self.double_speed();
        let g = &mut self.d.gstat;
        let after = g.g.cast_signed() + units_early + GStat::cc(ds, 2);
        g.lcdc.write(g.g, after.cast_unsigned(), val);
        g.lcdc_tracked = true;
    }

    /// LCDC writes that do not go through `gstat_write_lcdc` land at once.
    pub(in crate::ppu) const fn d_lcdc_tracked(&self) -> bool {
        self.d.gstat.lcdc_tracked
    }

    /// The LCDC write is complete.
    pub const fn gstat_lcdc_write_done(&mut self) {
        self.d.gstat.lcdc_tracked = false;
    }

    pub(in crate::ppu) const fn gstat_set_lcdc(&mut self, val: u8) {
        self.d.gstat.lcdc.set(val);
    }

    /// An HBlank HDMA enabled now starts with the current HBlank (gambatte's
    /// `isHdmaPeriod(cc + 4)`); `None` before mode 0 began.
    pub fn gstat_hdma_enable_in_hblank(&self) -> Option<bool> {
        self.gstat_hdma_period(4)
    }

    /// gambatte's `isHdmaPeriod(cc + offset)`; `None` before mode 0 began.
    pub fn gstat_hdma_period(&self, offset: i64) -> Option<bool> {
        let g = &self.d.gstat;
        if !self.gambatte_stat() || !g.lcd_on {
            return None;
        }
        if g.ly >= LINES {
            return Some(false);
        }
        if !g.hblank {
            return None;
        }
        let ds = self.double_speed();
        let at = g.h + GStat::cc(ds, offset);
        Some(at + GStat::cc(ds, 3 + 3 * i64::from(ds)) < LINE_H && at >= g.hblank_h + 3)
    }

    /// HBlank began.
    pub(super) const fn gstat_hblank(&mut self) {
        self.d.gstat.hblank = true;
        self.d.gstat.hblank_h = self.d.gstat.h;
    }

    /// The CGB-C's palettes become accessible 2 cycles after mode 0 begins
    /// on gambatte's clock (`m0Time + 2`); `None` outside HBlank.
    pub(in crate::ppu) const fn gstat_palettes_unlocked(&self) -> Option<bool> {
        let g = &self.d.gstat;
        if !self.gambatte_stat() || !g.lcd_on || !g.hblank || g.ly >= LINES {
            return None;
        }
        // Mode 0 begins (`m0Time`) 3 units after HBlankStart.
        Some(g.h >= g.hblank_h + 3 + GStat::cc(self.double_speed(), 2))
    }

    /// Mode 3 ended: the mode 0 event follows in the next unit.
    pub(super) const fn gstat_mode3_end(&mut self) {
        if !self.d.gstat.m0_done {
            self.d.gstat.m0_pending = true;
        }
    }

    /// The LCD was turned on: gambatte's clock starts at line 0.
    pub(in crate::ppu) fn gstat_lcd_on(&mut self) {
        let ds = self.double_speed();
        let g = &mut self.d.gstat;
        g.lcd_on = true;
        g.ly = 0;
        g.h = 0;
        g.m0_done = false;
        g.m0_pending = false;
        g.oneshot_time = DISABLED;
        g.lyc_stat = g.lyc_stat_src;
        g.lyc_reg = g.lyc_src;
        g.m_lyc.set(g.lyc_reg);
        g.lyc_reschedule();
        g.m2_time = g.schedule_m2(g.stat, ds);
        g.m0_scheduled = g.stat & STAT_IF_HBLANK_B != 0;
    }

    pub(in crate::ppu) const fn gstat_lcd_off(&mut self) {
        let g = &mut self.d.gstat;
        g.lcd_on = false;
        g.lyc_time = DISABLED;
        g.m2_time = DISABLED;
        g.oneshot_time = DISABLED;
        g.m0_scheduled = false;
        g.m0_pending = false;
    }

    /// Checks the clock against the start of a line on `line_clock`.
    pub(super) const fn gstat_line_end(&mut self, line: u8) {
        let g = &mut self.d.gstat;
        g.ly = line;
        g.h = LINE_H - 19;
    }

    /// A STAT write (gambatte's `lcdstatChange`).
    pub(in crate::ppu) fn gstat_write_stat(&mut self, data: u8, ints: &mut Interrupts) {
        let ds = self.double_speed();
        let g = &mut self.d.gstat;
        let old = g.stat;
        g.stat = data;
        g.lyc_reg_change(data, g.lyc_src, ds);
        if g.lcd_on {
            if data & STAT_IF_HBLANK_B != 0 {
                g.m0_scheduled = true;
            }
            g.m2_time = g.schedule_m2(data, ds);
            if g.stat_change_triggers(old, data, ds) {
                ints.request_lcd();
            }
            let late = GStat::cc(ds, 2 * i64::from(g.cgb));
            g.m_stat.write(g.g, g.g + late.cast_unsigned(), data);
        } else {
            g.m_stat.set(data);
        }
    }

    /// An LYC write (gambatte's `lycRegChange`).
    pub(in crate::ppu) fn gstat_write_lyc(&mut self, data: u8, ints: &mut Interrupts) {
        let ds = self.double_speed();
        let g = &mut self.d.gstat;
        let old = g.lyc_reg;
        if data == old {
            return;
        }
        g.lyc_reg_change(g.lyc_stat_src, data, ds);
        if !g.lcd_on {
            g.m_lyc.set(data);
            return;
        }
        g.m_lyc.write(
            g.g,
            g.g + GStat::cc(ds, 5 * i64::from(g.cgb) + 1 - i64::from(ds)).cast_unsigned(),
            data,
        );
        if g.lyc_change_triggers(old, data, ds) {
            if ds || !g.cgb {
                ints.request_lcd();
            } else {
                g.oneshot_time = g.g + GStat::cc(ds, 5).cast_unsigned();
            }
        }
    }
}
