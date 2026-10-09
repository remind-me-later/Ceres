//! The line and frame state machine.
//!
//! The display sleeps for a number of dots between the points where
//! something observable happens. [`State`] names the code that runs when a
//! sleep ends; its values are SameBoy's `display_state` numbers, so a trace
//! can still be compared against SameBoy. The values above 199 are not
//! sleeps but loop heads the machine passes through.

use {
    super::{
        LINE_LENGTH, LINES, MODE2_LENGTH, mode3::Mode3Flow, model_ge_cgb_d, stat::MODE_VBLANK_ENTRY,
    },
    crate::{
        interrupts::Interrupts,
        ppu::{Ppu, STAT_IF_OAM_B, STAT_MODE_B, oam_bug::NO_ROW},
    },
};

/// Dots from HBlankStart to the HBlank HDMA request.
const HBLANK_HDMA_DELAY: u8 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum State {
    // The first line after the LCD is turned on: no OAM scan.
    LcdOn = 0,
    FirstLine = 23,
    FirstLineOamLock = 2,
    FirstLineMode3 = 34,
    FirstLinePalettesLock = 37,
    FirstLineVramLock = 38,

    // Lines 0..=143: the OAM scan (mode 2).
    LineOamWriteLock = 35,
    LineLy = 6,
    OamScanStart = 7,
    OamScanNext = 201,
    OamScanObject = 8,
    Mode3PalettesLock = 10,
    Mode3Start = 32,

    // Drawing (mode 3): the loop and the points it resumes at.
    Mode3 = 200,
    Mode3WindowDelay = 42,
    Mode3ObjectWait = 27,
    Mode3ObjectFetch = 41,
    Mode3ObjectAttributes = 20,
    Mode3ObjectLow = 39,
    Mode3ObjectHigh = 40,
    Mode3Pixel = 21,

    // HBlank (mode 0) and the end of the line.
    HBlankStart = 22,
    HBlankHdma = 33,
    HBlankPalettesUnlock = 36,
    LineEnd = 11,
    NextLine = 31,
    /// A line longer than 456 dots: mode 3 is cut off.
    Mode3Abort = 9,
    LongLineEnd = 43,
    /// The line after an aborted one starts straight in mode 3.
    AbortedLineStart = 28,

    // Lines 144..=152: VBlank (mode 1).
    VBlankLine = 210,
    VBlankLy = 26,
    VBlankLyCompare = 12,
    VBlankLineRest = 24,
    VBlankLineEnd = 13,

    // Line 153, where LY reads 0 early.
    Line153 = 220,
    Line153Ly = 19,
    Line153LyZero = 14,
    Line153Compare = 15,
    Line153CompareZero = 16,
    /// Writing to LYC during this period on a CGB has side effects.
    Line153LycGlitch = 29,
    FrameEnd = 17,
}

impl Ppu {
    /// Runs the code of the states whose sleep ended, until the next sleep.
    pub(super) fn step_state_machine(&mut self, ints: &mut Interrupts) {
        // Sleeping: `wait` dots remain, resume when it reaches zero.
        if self.d.wait > 0 {
            self.d.wait -= 1;
            if self.d.wait > 0 {
                return;
            }
        }

        let mut state = self.d.state;
        loop {
            let next = match state {
                State::LcdOn
                | State::FirstLine
                | State::FirstLineOamLock
                | State::FirstLineMode3
                | State::FirstLinePalettesLock
                | State::FirstLineVramLock => self.first_line(ints, state),

                State::LineOamWriteLock
                | State::LineLy
                | State::OamScanStart
                | State::OamScanNext
                | State::OamScanObject
                | State::Mode3PalettesLock
                | State::Mode3Start => self.oam_scan(ints, state),

                State::Mode3
                | State::Mode3WindowDelay
                | State::Mode3ObjectWait
                | State::Mode3ObjectFetch
                | State::Mode3ObjectAttributes
                | State::Mode3ObjectLow
                | State::Mode3ObjectHigh
                | State::Mode3Pixel => {
                    if matches!(self.mode3(ints, state), Mode3Flow::Done) {
                        self.mode3_done();
                    }
                    None
                }

                State::HBlankStart
                | State::HBlankHdma
                | State::HBlankPalettesUnlock
                | State::LineEnd
                | State::NextLine
                | State::Mode3Abort
                | State::LongLineEnd
                | State::AbortedLineStart => self.hblank(ints, state),

                State::VBlankLine
                | State::VBlankLy
                | State::VBlankLyCompare
                | State::VBlankLineRest
                | State::VBlankLineEnd => self.vblank(ints, state),

                State::Line153
                | State::Line153Ly
                | State::Line153LyZero
                | State::Line153Compare
                | State::Line153CompareZero
                | State::Line153LycGlitch
                | State::FrameEnd => self.line_153(ints, state),
            };
            match next {
                Some(next) => state = next,
                None => return,
            }
        }
    }

    /// Starts a visible line (lines 0..=143).
    pub(super) fn line_start(&mut self) {
        self.wy_check();
        self.d.cpu.oam_write_blocked = self.hw_cgb() && !self.double_speed();
        self.d.objs.accessed_oam_row = 0;
        self.sleep(State::LineOamWriteLock, 2);
    }

    /// The first line after the LCD is turned on: it has no OAM scan, and is
    /// shorter.
    fn first_line(&mut self, ints: &mut Interrupts, state: State) -> Option<State> {
        match state {
            State::LcdOn => {
                if !self.hw_cgb() {
                    self.sleep(State::FirstLine, 1);
                    return None;
                }
                Some(State::FirstLine)
            }
            State::FirstLine => {
                self.d.current_line = 0;
                self.d.window.line = 0xFF;
                self.d.window.wy_triggered = false;
                self.d.position_in_line = 240;
                self.d.line_has_fractional_scrolling = false;
                self.d.irq.ly_for_comparison = 0;
                self.stat &= !STAT_MODE_B;
                self.d.irq.mode_for_interrupt = -1;
                self.d.cpu.unlock_all();
                self.d.cfl = MODE2_LENGTH - 4;
                self.d.line_clock = 0;
                self.stat_update(ints);
                self.sleep(State::FirstLineOamLock, MODE2_LENGTH - 4);
                None
            }
            State::FirstLineOamLock => {
                self.d.cpu.oam_write_blocked = true;
                self.d.cfl += 2;
                self.stat_update(ints);
                self.sleep(State::FirstLineMode3, 2);
                None
            }
            State::FirstLineMode3 => {
                self.d.objs.count = 0;
                self.d.objs.found = 0;
                // Mode 0 is shorter on the first line 0.
                self.d.cfl += 8;
                self.stat = (self.stat & !STAT_MODE_B) | 3;
                self.d.irq.mode_for_interrupt = 3;
                self.d.cpu.oam_write_blocked = true;
                self.d.cpu.oam_read_blocked = true;
                let vram_blocked = self.double_speed() || !self.hw_cgb();
                self.d.cpu.vram_read_blocked = vram_blocked;
                self.d.cpu.vram_write_blocked = vram_blocked;
                self.d.cfl += 2;
                self.sleep(State::FirstLinePalettesLock, 2);
                None
            }
            State::FirstLinePalettesLock => {
                self.d.cpu.cgb_palettes_blocked = true;
                self.d.cfl += 3;
                self.sleep(State::FirstLineVramLock, 3);
                None
            }
            State::FirstLineVramLock => {
                self.d.cpu.vram_read_blocked = true;
                self.d.cpu.vram_write_blocked = true;
                self.d.window.wx_triggered = false;
                self.mode3_start();
                Some(State::Mode3)
            }
            _ => unreachable!(),
        }
    }

    /// Lines 0..=143 up to mode 3: LY changes and the OAM scan.
    fn oam_scan(&mut self, ints: &mut Interrupts, state: State) -> Option<State> {
        match state {
            State::LineOamWriteLock => {
                self.d.cpu.oam_write_blocked = self.hw_cgb();
                self.sleep(State::LineLy, 1);
                None
            }
            State::LineLy => {
                self.ly = self.d.current_line;
                self.d.cpu.oam_read_blocked = !self.double_speed() || model_ge_cgb_d(self.model);
                self.d.irq.ly_for_comparison = if self.d.current_line != 0 { -1 } else { 0 };
                // The OAM STAT interrupt occurs 1 T-cycle before STAT
                // actually changes, except on line 0.
                if self.d.current_line != 0 {
                    self.d.irq.mode_for_interrupt = 2;
                    self.stat &= !STAT_MODE_B;
                } else if !self.hw_cgb() {
                    self.stat &= !STAT_MODE_B;
                } else {
                    // CGB line 0: STAT keeps its mode bits.
                }
                self.stat_update(ints);
                self.sleep(State::OamScanStart, 1);
                None
            }
            State::OamScanStart => {
                self.d.cpu.oam_read_blocked = true;
                self.d.cpu.oam_write_blocked = true;
                self.stat = (self.stat & !STAT_MODE_B) | 2;
                self.d.irq.ly_for_comparison = i32::from(self.d.current_line);
                self.wy_check();
                if self.d.current_line == 0 && !self.hw_cgb() {
                    // The DMG's mode 2 condition of line 0 comes one dot after
                    // the other lines'.
                    self.stat_update(ints);
                    self.d.irq.line0_pulse = 1;
                } else {
                    self.d.irq.mode_for_interrupt = 2;
                    self.stat_update(ints);
                    self.d.irq.mode_for_interrupt = -1;
                    self.stat_update(ints);
                }
                self.d.objs.count = 0;
                self.d.objs.found = 0;
                self.d.objs.index = 0;
                Some(State::OamScanNext)
            }
            State::OamScanNext => {
                // The CGB adds the object before the sleep.
                if self.hw_cgb() {
                    self.add_object_from_index(self.d.objs.index);
                }
                self.sleep(State::OamScanObject, 2);
                None
            }
            State::OamScanObject => {
                if !self.hw_cgb() {
                    self.add_object_from_index(self.d.objs.index);
                    self.d.objs.accessed_oam_row = (self.d.objs.index & !1) * 4 + 8;
                }
                if self.d.objs.index == 37 {
                    self.d.cpu.vram_read_blocked = !self.hw_cgb();
                    self.d.cpu.vram_write_blocked = false;
                    self.d.cpu.cgb_palettes_blocked = false;
                    self.d.cpu.oam_write_blocked = self.hw_cgb();
                }
                self.d.objs.index += 1;
                if self.d.objs.index < 40 {
                    return Some(State::OamScanNext);
                }
                self.d.cfl = MODE2_LENGTH + 4;
                self.d.objs.accessed_oam_row = NO_ROW;
                self.d.objs.found = self.d.objs.count;
                self.stat = (self.stat & !STAT_MODE_B) | 3;
                self.d.irq.mode_for_interrupt = 3;
                self.d.cpu.vram_read_blocked = true;
                self.d.cpu.vram_write_blocked = true;
                self.d.cpu.cgb_palettes_blocked = false;
                self.d.cpu.oam_write_blocked = true;
                self.d.cpu.oam_read_blocked = true;
                self.stat_update(ints);
                self.d.cfl += 3;
                self.sleep(State::Mode3PalettesLock, 3);
                None
            }
            State::Mode3PalettesLock => {
                self.d.cpu.cgb_palettes_blocked = true;
                self.d.cfl += 2;
                self.sleep(State::Mode3Start, 2);
                None
            }
            State::Mode3Start => {
                self.mode3_start();
                Some(State::Mode3)
            }
            _ => unreachable!(),
        }
    }

    /// HBlank and the end of the visible lines, including the lines cut off
    /// in mode 3.
    fn hblank(&mut self, ints: &mut Interrupts, state: State) -> Option<State> {
        match state {
            State::HBlankStart => {
                self.stat &= !STAT_MODE_B;
                self.d.irq.mode_for_interrupt = 0;
                self.d.cpu.unlock_oam_vram();
                self.d.hblank_hdma_delay = HBLANK_HDMA_DELAY;
                self.stat_update(ints);
                self.d.cfl += 2;
                self.sleep(State::HBlankHdma, 2);
                None
            }
            State::HBlankHdma => {
                self.d.cpu.cgb_palettes_blocked = !self.double_speed();
                self.d.cfl += 2;
                self.sleep(State::HBlankPalettesUnlock, 2);
                None
            }
            State::HBlankPalettesUnlock => {
                self.d.cpu.cgb_palettes_blocked = false;
                if self.d.cfl > LINE_LENGTH - 2 {
                    self.d.cfl = 0;
                    self.sleep(State::LongLineEnd, LINE_LENGTH);
                } else {
                    let rest = LINE_LENGTH - self.d.cfl - 2;
                    self.d.cfl = 0;
                    self.sleep(State::LineEnd, rest);
                }
                None
            }
            State::LongLineEnd | State::Mode3Abort => {
                self.fill_desynced_line();
                self.d.objs.count = self.d.objs.found;
                self.d.current_line += 1;
                self.wy_check();
                self.d.cfl = 0;
                if self.d.current_line != LINES {
                    self.d.cfl = 2;
                    self.sleep(State::AbortedLineStart, 2);
                    return None;
                }
                if (156..240).contains(&self.d.position_in_line) {
                    self.d.irq.delayed_glitch_hblank_interrupt = true;
                }
                self.d.position_in_line = 240;
                self.d.line_has_fractional_scrolling = false;
                Some(State::VBlankLine)
            }
            State::AbortedLineStart => {
                self.ly = self.d.current_line;
                if (156..240).contains(&self.d.position_in_line) {
                    self.d.irq.delayed_glitch_hblank_interrupt = true;
                }
                self.stat_update(ints);
                self.d.position_in_line = 241;
                self.mode3_start();
                Some(State::Mode3)
            }
            State::LineEnd => {
                self.d.cfl = 0;
                self.d.line_clock = 0;
                self.sleep(State::NextLine, 2);
                None
            }
            State::NextLine => {
                self.d.current_line += 1;
                if self.d.current_line < LINES {
                    self.line_start();
                    return None;
                }
                Some(State::VBlankLine)
            }
            _ => unreachable!(),
        }
    }

    /// Lines 144..=152.
    fn vblank(&mut self, ints: &mut Interrupts, state: State) -> Option<State> {
        match state {
            State::VBlankLine => {
                self.d.irq.ly_for_comparison = -1;
                self.stat_update(ints);
                self.sleep(State::VBlankLy, 2);
                None
            }
            State::VBlankLy => {
                self.ly = self.d.current_line;
                if self.d.current_line == LINES {
                    if !self.d.irq.stat_interrupt_line && self.stat & STAT_IF_OAM_B != 0 {
                        ints.request_lcd();
                    }
                    // Until the VBlank condition takes over, the line is held
                    // by the HBlank or the OAM one (see `stat_update`).
                    self.d.irq.mode_for_interrupt = MODE_VBLANK_ENTRY;
                    self.d.irq.entry_stat = self.stat;
                }
                self.sleep(State::VBlankLyCompare, 2);
                None
            }
            State::VBlankLyCompare => {
                if self.d.irq.delayed_glitch_hblank_interrupt {
                    self.d.irq.delayed_glitch_hblank_interrupt = false;
                    self.d.irq.mode_for_interrupt = 0;
                }
                self.d.irq.ly_for_comparison = i32::from(self.d.current_line);
                self.stat_update(ints);
                self.sleep(State::VBlankLineRest, 1);
                None
            }
            State::VBlankLineRest => {
                if self.d.current_line == LINES {
                    // Entering VBlank also triggers the OAM interrupt.
                    self.stat &= !STAT_MODE_B;
                    self.stat |= 1;
                    ints.request_vblank();
                    if !self.d.irq.stat_interrupt_line && self.stat & STAT_IF_OAM_B != 0 {
                        ints.request_lcd();
                    }
                    self.d.irq.mode_for_interrupt = 1;
                    self.stat_update(ints);
                    self.present_frame();
                }
                self.sleep(State::VBlankLineEnd, LINE_LENGTH - 5);
                None
            }
            State::VBlankLineEnd => {
                self.d.current_line += 1;
                if self.d.current_line < 153 {
                    Some(State::VBlankLine)
                } else {
                    Some(State::Line153)
                }
            }
            _ => unreachable!(),
        }
    }

    /// Line 153: LY reads 0 for most of it, at a revision specific time.
    fn line_153(&mut self, ints: &mut Interrupts, state: State) -> Option<State> {
        let cgb_d = model_ge_cgb_d(self.model);
        match state {
            State::Line153 => {
                self.d.irq.ly_for_comparison = -1;
                self.stat_update(ints);
                self.sleep(State::Line153Ly, 2);
            }
            State::Line153Ly => {
                self.ly = 153;
                self.sleep(State::Line153LyZero, if cgb_d { 2 } else { 4 });
            }
            State::Line153LyZero => {
                if !cgb_d && !self.double_speed() {
                    self.ly = 0;
                }
                self.d.irq.ly_for_comparison = 153;
                self.stat_update(ints);
                self.sleep(State::Line153Compare, if cgb_d { 4 } else { 2 });
            }
            State::Line153Compare => {
                self.ly = 0;
                self.d.irq.ly_for_comparison = if cgb_d || self.double_speed() {
                    153
                } else {
                    -1
                };
                self.stat_update(ints);
                // The DMG compares with line 0 one dot later.
                self.sleep(State::Line153CompareZero, if self.hw_cgb() { 4 } else { 5 });
            }
            State::Line153CompareZero => {
                self.d.irq.ly_for_comparison = 0;
                self.stat_update(ints);
                self.sleep(State::Line153LycGlitch, if self.hw_cgb() { 12 } else { 11 });
            }
            State::Line153LycGlitch => self.sleep(State::FrameEnd, LINE_LENGTH - 24),
            State::FrameEnd => {
                self.d.current_line = 0;
                self.d.window.wy_triggered = false;
                self.d.window.line0_wy_countdown = if self.hw_cgb() { 7 } else { 6 };
                self.line_start();
            }
            _ => unreachable!(),
        }
        None
    }
}
