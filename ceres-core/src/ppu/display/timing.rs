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
//! It drives the STAT interrupt and the LY and STAT reads of the DMG family.
//! On the CGB, `gstat.rs` (up to revision C) and SameBoy's model (`stat.rs`)
//! still do; there it runs as a shadow (the `debug` feature records its
//! interrupts in the PPU trace, and `Gb::set_timing_live` lets it drive the
//! interrupt and the reads instead). Live, it passes the CGB suites too:
//! gambatte's 3023 CGB-C checks, SameSuite, Mooneye, AGE.
//!
//! The revisions share the structure and differ in a table of phases
//! (`Phases`) and of CPU access delays (`Delays`). The CGB's were fitted to
//! the tests; `doc/cgb-hardware-tests.md` lists what hardware should
//! confirm.
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
    /// A written LYC and the half dots until it reaches the comparison.
    lyc_pending: Option<(u8, u8)>,
    /// Written enables and the half dots until they reach the line (CGB).
    stat_pending: Option<(u8, u8)>,
    /// The line rose in the last unit.
    unit_edge: bool,
    /// The unit drives the interrupt on the CGB too (an experiment:
    /// `Gb::set_timing_live`).
    live: bool,
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
            unit_edge: false,
            live: false,
        }
    }
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
            _ => {}
        }
        // No line reset as LY becomes 144: HBlank holds into VBlank.
        if self.h == self.phases.hblank_clear && self.line != LAST_VISIBLE_LINE {
            self.hblank = false;
        }
        let sample = self.h & 7 == LY_MATCH_PHASE;
        let mut rose = false;
        if let Some((lyc, left)) = self.lyc_pending {
            // The CGB's comparison takes a new LYC only at a sample.
            if left <= 1 && (sample || !self.phases.cgb) {
                self.lyc = lyc;
                self.lyc_pending = None;
                self.ly_match = self.ly == self.lyc;
                rose |= self.update_line();
            } else {
                self.lyc_pending = Some((lyc, if left > 1 { left - 1 } else { 1 }));
            }
        }
        if sample {
            self.ly_match = self.ly == self.lyc;
        }
        if self.h == self.phases.line153_ly_reset && self.line == LAST_LINE {
            self.ly = 0;
        }
        rose |= self.update_line();
        // VBlank is latched on the same clock edge as LY=LYC, a gate later:
        // LY=LYC dropping as VBlank starts leaves the line low in between.
        // Written enables (CGB) land with it: the line sees both at once.
        if let Some((enables, left)) = self.stat_pending {
            if left <= 1 {
                self.enables = enables;
                self.stat_pending = None;
            } else {
                self.stat_pending = Some((enables, left - 1));
            }
        }
        if self.h == VBLANK_H {
            match self.line {
                LAST_VISIBLE_LINE => self.vblank = true,
                LAST_LINE => self.vblank = false,
                _ => {}
            }
        }
        rose |= self.update_line();
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
        let t = self.d.timing.ahead(delay);
        if self.d.timing.phases.cgb && delay > 0 {
            let before = self.d.timing.ahead(delay - 1);
            if before.ly != t.ly {
                return before.ly & t.ly;
            }
        }
        t.ly
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
        let flag_view = t.ahead(delays.flag_read);
        let holds = t.phases.cgb && (self.double_speed() || t.phases.flag_holds);
        let lyc_flag = flag_view.ly_match && (holds || flag_view.ly == flag_view.lyc);
        let mode = t.ahead(delays.mode_read).read_mode(stat & STAT_MODE_B);
        enables | mode | if lyc_flag { STAT_LYC_B } else { 0 }
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
    #[cfg_attr(
        not(feature = "debug"),
        expect(
            clippy::missing_const_for_fn,
            clippy::needless_pass_by_ref_mut,
            reason = "The debug build traces it"
        )
    )]
    fn timing_rose(&mut self, ints: &mut Interrupts) {
        #[cfg(feature = "debug")]
        self.trace_shadow_lcd_interrupt();
        if self.timing_live() {
            ints.request_lcd();
        }
    }

    /// Whether the interrupt line rose in the last unit.
    #[cfg(feature = "debug")]
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
    #[cfg(feature = "debug")]
    pub(in crate::ppu) const fn timing_position(&self) -> (u8, i32) {
        (self.d.timing.line, self.d.timing.h)
    }

    /// The unit drives the STAT interrupt and the reads: on the DMG family,
    /// and on the CGB with `Gb::set_timing_live`.
    pub(in crate::ppu) const fn timing_live(&self) -> bool {
        !self.is_cgb_hardware() || self.d.timing.live
    }

    /// See `Gb::set_timing_live`.
    #[cfg(feature = "debug")]
    pub(crate) const fn set_timing_live(&mut self, live: bool) {
        self.d.timing.live = live;
    }

    /// A STAT write. On the DMG the write briefly sets every enable: any
    /// active source raises the interrupt (with the LCD off, a set LY=LYC).
    /// The CGB has no such glitch, and its enables reach the line later.
    pub(in crate::ppu) fn timing_write_stat(&mut self, val: u8, ints: &mut Interrupts) {
        let delay = self.delays().stat_write;
        let t = &mut self.d.timing;
        let enables = val & STAT_ENABLES;
        // A write still on its way lands now.
        if let Some((pending, _)) = t.stat_pending.take() {
            t.enables = pending;
        }
        if t.on && delay > 0 {
            t.stat_pending = Some((enables, delay));
            return;
        }
        let glitch = !t.phases.cgb && t.line_for(STAT_ENABLES) && !t.stat_line;
        t.stat_line |= glitch;
        t.enables = enables;
        if t.update_line() || glitch {
            self.timing_rose(ints);
        }
    }

    /// An LYC write: it reaches the comparison `Delays::lyc_write` half dots
    /// later, which then follows it at once on the DMG, and at its next
    /// sample on the CGB.
    pub(in crate::ppu) const fn timing_write_lyc(&mut self, val: u8) {
        let delay = self.delays().lyc_write;
        let t = &mut self.d.timing;
        if t.on {
            t.lyc_pending = Some((val, delay));
        } else {
            t.lyc = val;
            t.lyc_pending = None;
        }
    }
}
