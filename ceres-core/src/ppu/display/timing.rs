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
//! It drives the STAT interrupt and the LY and STAT reads of every model.
//! The revisions share the structure and differ in a table of phases
//! (`Phases`) and of CPU access delays (`Delays`). The CGB's were fitted to
//! the CGB test suites (gambatte's CGB-C checks, SameSuite, Mooneye, AGE);
//! `doc/cgb-hardware-tests.md` lists what hardware should confirm.
//!
//! Times are in half dots on a line clock (`h`, 912 per line) that the
//! pixel pipeline resyncs at every line end; `h = 0` is where LY changes for
//! a CPU read (gambatte's clock, which its tests are written against).

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

/// A register write on its way to the timing unit.
#[derive(Clone, Copy)]
struct PendingWrite {
    value: u8,
    half_dots_left: u8,
}

impl PendingWrite {
    /// The write a half dot later. It stays at 1 while it waits for
    /// something else (the CGB's LY=LYC sample).
    const fn ticked(self) -> Self {
        Self {
            half_dots_left: if self.half_dots_left > 1 {
                self.half_dots_left - 1
            } else {
                1
            },
            ..self
        }
    }
}

/// Where the LCD's clock is.
#[derive(Clone, Copy)]
pub(in crate::ppu) struct ClockPosition {
    pub line: u8,
    /// The half dot within the line.
    pub half_dot: i32,
}
/// The line-end pulse rises 8 half dots (an M-cycle) before the line and
/// lasts an M-cycle.
pub(super) const LINE_END_H: i32 = LINE_H - 8;
/// The line reset that clears the HBlank gate, just after the pulse.
pub(super) const LINE_RESET_H: i32 = LINE_END_H + 2;
/// VBlank is latched half an M-cycle after the pulse.
pub(super) const VBLANK_H: i32 = LINE_END_H + 4;
/// LY=LYC is sampled once per M-cycle, at this phase.
const LY_MATCH_PHASE: i32 = 4;
/// The half dots from an LCDC write that turns the LCD off to the sources of
/// the interrupt seeing it.
const LCD_OFF_LATE: u8 = 3;

const LAST_VISIBLE_LINE: u8 = LINES - 1;

/// The enables that STAT's bits 3-6 hold.
const STAT_ENABLES: u8 = STAT_IF_HBLANK_B | STAT_IF_VBLANK_B | STAT_IF_OAM_B | STAT_IF_LYC_B;

/// Where a revision's signals sit on the clock. The structure is the same
/// on every model; the CGB's phases were fitted to the CGB test suites (see
/// `doc/cgb-hardware-tests.md` for what hardware should confirm).
#[derive(Clone, Copy)]
struct Phases {
    /// The CGB: no STAT write glitch, writes and reads at the CGB's delays
    /// (`Ppu::delays`), and a written LYC takes effect at an LY=LYC sample.
    cgb: bool,
    /// The line reset clears the HBlank gate.
    hblank_clear: i32,
    /// LY resets to 0 early on line 153, after the LY=LYC sample there.
    line153_ly_reset: i32,
    /// The OAM scan (mode 2) as reads see it, on the delayed clock.
    scan_start: i32,
    scan_end: i32,
    /// Reads see mode 1 from this point of line 143 (the DMG only from line
    /// 144)...
    mode1_start: i32,
    /// ...until this point of line 153.
    mode1_end: i32,
    /// At normal speed the LY=LYC flag holds until the next LY=LYC sample
    /// instead of dropping as LY changes (it does in double speed on every
    /// CGB).
    flag_holds: bool,
}

const DMG_PHASES: Phases = Phases {
    cgb: false,
    hblank_clear: LINE_RESET_H,
    line153_ly_reset: 4,
    scan_start: LINE_RESET_H + 4,
    scan_end: 2 * 77 + 4,
    mode1_start: LINE_H,
    mode1_end: VBLANK_H,
    flag_holds: false,
};

/// The CGB up to revision C (gambatte's tests): the HBlank gate clears and
/// the OAM scan reads a half dot off the DMG's.
const CGB_C_PHASES: Phases = Phases {
    cgb: true,
    hblank_clear: LINE_RESET_H + 1,
    scan_start: LINE_RESET_H + 3,
    scan_end: 2 * 77 + 3,
    mode1_start: LINE_H - 1,
    ..DMG_PHASES
};

/// The CGB from revision D (Mooneye's and AGE's CGB-E tests): line 153's LY
/// reset a dot later, mode 1 longer and the LY=LYC flag held.
const CGB_E_PHASES: Phases = Phases {
    line153_ly_reset: 6,
    mode1_end: VBLANK_H + 2,
    flag_holds: true,
    ..CGB_C_PHASES
};

#[expect(
    clippy::struct_excessive_bools,
    reason = "Independent signals of the circuit"
)]
#[derive(Clone)]
pub(super) struct LcdTiming {
    phases: Phases,
    on: bool,
    /// The first line after the LCD was turned on, which has no OAM scan.
    first_line: bool,
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
    /// A written LYC on its way to the comparison.
    lyc_pending: Option<PendingWrite>,
    /// Written enables on their way to the line (CGB).
    stat_pending: Option<PendingWrite>,
    /// An input of the line changed outside a unit without the line being
    /// re-evaluated.
    dirty: bool,
}

impl Default for LcdTiming {
    fn default() -> Self {
        Self {
            phases: DMG_PHASES,
            on: false,
            first_line: false,
            line: 0,
            h: 0,
            ly: 0,
            line_end: false,
            vblank: false,
            hblank: false,
            hblank_pending: false,
            ly_match: false,
            enables: 0,
            lyc: 0,
            stat_line: false,
            lyc_pending: None,
            stat_pending: None,
            dirty: false,
        }
    }
}

impl LcdTiming {
    /// Where the clock is.
    pub(super) const fn position(&self) -> ClockPosition {
        ClockPosition {
            line: self.line,
            half_dot: self.h,
        }
    }

    /// The LCD is on.
    pub(super) const fn is_on(&self) -> bool {
        self.on
    }

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
            self.first_line = false;
            self.line = if self.line == LAST_LINE {
                0
            } else {
                self.line + 1
            };
        }
        if !self.on {
            return false;
        }

        // The line is re-evaluated only when one of its inputs changed: in
        // most half dots none does.
        let mut changed = self.dirty;
        self.dirty = false;
        if self.hblank_pending {
            self.hblank_pending = false;
            self.hblank = true;
            changed = true;
        }
        match self.h {
            0 => {
                self.line_end = false;
                changed = true;
            }
            LINE_END_H => {
                self.line_end = true;
                self.ly = if self.line == LAST_LINE {
                    0
                } else {
                    self.line + 1
                };
                changed = true;
            }
            _ => {}
        }
        // No line reset as LY becomes 144: HBlank holds into VBlank.
        if self.h == self.phases.hblank_clear && self.line != LAST_VISIBLE_LINE && self.hblank {
            self.hblank = false;
            changed = true;
        }
        let sample = self.h & 7 == LY_MATCH_PHASE;
        let mut rose = false;
        if let Some(write) = self.lyc_pending {
            // The CGB's comparison takes a new LYC only at a sample.
            if write.half_dots_left <= 1 && (sample || !self.phases.cgb) {
                self.lyc = write.value;
                self.lyc_pending = None;
                self.ly_match = self.ly == self.lyc;
                rose |= self.update_line();
            } else {
                self.lyc_pending = Some(write.ticked());
            }
        }
        if sample && self.ly_match != (self.ly == self.lyc) {
            self.ly_match = !self.ly_match;
            changed = true;
        }
        if self.h == self.phases.line153_ly_reset && self.line == LAST_LINE {
            self.ly = 0;
        }
        if changed {
            rose |= self.update_line();
        }
        // VBlank is latched on the same clock edge as LY=LYC, a gate later:
        // LY=LYC dropping as VBlank starts leaves the line low in between.
        // Written enables (CGB) land with it: the line sees both at once.
        changed = false;
        if let Some(write) = self.stat_pending {
            if write.half_dots_left <= 1 {
                self.enables = write.value;
                self.stat_pending = None;
                changed = true;
            } else {
                self.stat_pending = Some(write.ticked());
            }
        }
        if self.h == VBLANK_H {
            match self.line {
                LAST_VISIBLE_LINE => {
                    self.vblank = true;
                    changed = true;
                }
                LAST_LINE => {
                    self.vblank = false;
                    changed = true;
                }
                _ => {}
            }
        }
        if changed {
            rose |= self.update_line();
        }
        rose
    }
}

/// How many half dots after the unit's clock a CPU access lands: writes
/// reach the unit that much later, reads see it that much ahead.
struct Delays {
    stat_write: u8,
    /// On the CGB, at the first LY=LYC sample after it.
    lyc_write: u8,
    ly_read: u8,
    flag_read: u8,
    mode_read: u8,
}

const DMG_DELAYS: Delays = Delays {
    stat_write: 0,
    lyc_write: 3,
    ly_read: 4,
    flag_read: 3,
    mode_read: 4,
};

const CGB_DELAYS: Delays = Delays {
    stat_write: 4,
    lyc_write: 6,
    flag_read: 1,
    ..DMG_DELAYS
};

const CGB_DOUBLE_SPEED_DELAYS: Delays = Delays {
    stat_write: 3,
    lyc_write: 4,
    ly_read: 2,
    flag_read: 2,
    mode_read: 3,
};

impl LcdTiming {
    /// STAT's mode as a read sees it; `pipeline` is the mode the drawing
    /// pipeline is in, which tells mode 3 from mode 0.
    const fn read_mode(&self, pipeline: u8) -> u8 {
        let p = &self.phases;
        let next_visible = self.line < LAST_VISIBLE_LINE || self.line == LAST_LINE;
        let scanning = (self.h >= p.scan_start && next_visible)
            || (self.h < p.scan_end && self.line < LINES && !self.first_line);
        let visible =
            self.line < LINES && !(self.line == LAST_VISIBLE_LINE && self.h >= p.scan_start);
        let vblank = (self.vblank && (self.line >= LINES || self.h >= p.mode1_start))
            || (self.line == LAST_LINE && self.h >= VBLANK_H && self.h < p.mode1_end);
        if vblank {
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
    /// The CPU's access delays for the model and speed.
    const fn delays(&self) -> Delays {
        if !self.d.timing.phases.cgb {
            DMG_DELAYS
        } else if self.double_speed() {
            CGB_DOUBLE_SPEED_DELAYS
        } else {
            CGB_DELAYS
        }
    }

    /// LY as a CPU read sees it. On the CGB a read in the half dot where LY
    /// changes sees the counter rippling: the bits that fall have fallen,
    /// the one that rises has not risen yet.
    pub(in crate::ppu) fn timing_read_ly(&self) -> u8 {
        let delay = self.delays().ly_read;
        let mut t = self.d.timing.clone();
        let mut before = t.ly;
        for _ in 0..delay {
            before = t.ly;
            t.unit();
        }
        if self.d.timing.phases.cgb && delay > 0 && before != t.ly {
            before & t.ly
        } else {
            t.ly
        }
    }

    /// STAT's mode and LY=LYC flag as a CPU read sees them, over `stat`
    /// (the enables and the pipeline's mode). The flag comes from a latch
    /// (`RUPO`) that follows the synced comparison, and on the DMG drops as
    /// soon as LY changes.
    pub(in crate::ppu) fn timing_read_stat(&self, stat: u8) -> u8 {
        let enables = stat & !(STAT_MODE_B | STAT_LYC_B);
        let t = &self.d.timing;
        if !t.on {
            // With the LCD off the comparison clock stops: the flag keeps
            // its last value whatever LY and LYC are, and the mode reads 0.
            return enables | if t.ly_match { STAT_LYC_B } else { 0 };
        }
        let delays = self.delays();
        let holds = t.phases.cgb && (self.double_speed() || t.phases.flag_holds);
        // One run ahead serves both views.
        let mut t = t.clone();
        let (mut lyc_flag, mut mode) = (false, 0);
        for k in 0..=delays.flag_read.max(delays.mode_read) {
            if k > 0 {
                t.unit();
            }
            if k == delays.flag_read {
                lyc_flag = t.ly_match && (holds || t.ly == t.lyc);
            }
            if k == delays.mode_read {
                mode = t.read_mode(stat & STAT_MODE_B);
            }
        }
        enables | mode | if lyc_flag { STAT_LYC_B } else { 0 }
    }

    /// Advances the unit by a half dot.
    pub(super) const fn timing_unit(&mut self, ints: &mut Interrupts) {
        if self.d.timing.unit() {
            ints.request_lcd();
        }
    }

    /// Mode 3 ended: the HBlank gate rises in the next unit.
    pub(super) const fn timing_mode3_end(&mut self) {
        self.d.timing.hblank_pending = true;
    }

    /// The LCD was turned on: the line clock starts at line 0.
    pub(in crate::ppu) const fn timing_lcd_on(&mut self, ints: &mut Interrupts) {
        let phases = if !self.is_cgb_hardware() {
            DMG_PHASES
        } else if super::model_ge_cgb_d(self.model) {
            CGB_E_PHASES
        } else {
            CGB_C_PHASES
        };
        let t = &mut self.d.timing;
        t.phases = phases;
        t.on = true;
        t.first_line = true;
        t.line = 0;
        t.h = 0;
        t.ly = 0;
        t.line_end = false;
        t.vblank = false;
        t.hblank = false;
        t.hblank_pending = false;
        t.ly_match = t.lyc == 0;
        if t.update_line() {
            ints.request_lcd();
        }
    }

    /// The LCD was turned off: LY=LYC keeps its last value. The LCD's reset
    /// reaches the sources of the interrupt a moment after the write lands
    /// here: the clock runs on for those half dots, and an interrupt that
    /// rises in them is requested (in the netlist a line-end pulse 2 half
    /// dots after the write still requests line 0x45's mode 2 interrupt:
    /// the reset follows it by 156 ns; `hdl/check/lcd_off_line_start.asm`).
    pub(in crate::ppu) const fn timing_lcd_off(&mut self, ints: &mut Interrupts) {
        let mut late = LCD_OFF_LATE;
        while late > 0 {
            self.timing_unit(ints);
            late -= 1;
        }
        let t = &mut self.d.timing;
        t.on = false;
        t.ly = 0;
        t.line_end = false;
        t.vblank = false;
        t.hblank = false;
        t.hblank_pending = false;
        t.update_line();
    }

    /// Resyncs the clock with the pixel pipeline at a line end.
    pub(super) const fn timing_line_end(&mut self, line: u8) {
        self.d.timing.line = line;
        self.d.timing.h = LINE_H - 19;
    }

    /// Where the unit's clock is.
    #[cfg(feature = "debug")]
    pub(in crate::ppu) const fn timing_position(&self) -> ClockPosition {
        self.d.timing.position()
    }

    /// A STAT write. On the DMG the write briefly sets every enable: any
    /// active source raises the interrupt (with the LCD off, a set LY=LYC).
    /// The CGB has no such glitch, and its enables reach the line later.
    pub(in crate::ppu) const fn timing_write_stat(&mut self, val: u8, ints: &mut Interrupts) {
        let delay = self.delays().stat_write;
        let t = &mut self.d.timing;
        let enables = val & STAT_ENABLES;
        // A write still on its way lands now.
        if let Some(write) = t.stat_pending.take() {
            t.enables = write.value;
            t.dirty = true;
        }
        if t.on && delay > 0 {
            t.stat_pending = Some(PendingWrite {
                value: enables,
                half_dots_left: delay,
            });
            return;
        }
        let glitch = !t.phases.cgb && t.line_for(STAT_ENABLES) && !t.stat_line;
        t.stat_line |= glitch;
        t.enables = enables;
        if t.update_line() || glitch {
            ints.request_lcd();
        }
    }

    /// An LYC write: it reaches the comparison `Delays::lyc_write` half dots
    /// later, which then follows it at once on the DMG, and at its next
    /// sample on the CGB.
    pub(in crate::ppu) const fn timing_write_lyc(&mut self, val: u8) {
        let delay = self.delays().lyc_write;
        let t = &mut self.d.timing;
        if t.on {
            t.lyc_pending = Some(PendingWrite {
                value: val,
                half_dots_left: delay,
            });
        } else {
            t.lyc = val;
            t.lyc_pending = None;
        }
    }
}
