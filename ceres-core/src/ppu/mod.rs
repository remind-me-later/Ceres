mod color_palette;
mod draw;
mod oam;
mod rgba_buf;
mod vram;

use core::mem;

use crate::interrupts::Interrupts;
pub use oam::Oam;
pub use vram::Vram;
use {self::color_palette::ColorPalette, crate::CgbMode, rgba_buf::RgbaBuf};

pub const PX_WIDTH: u8 = 160;
pub const PX_HEIGHT: u8 = 144;

// LCDC bits
const LCDC_BG_B: u8 = 0x1;
const LCDC_OBJ_B: u8 = 0x2;
const LCDC_OBJL_B: u8 = 0x4;
const LCDC_BG_AREA: u8 = 0x8;
const LCDC_BG_SIGNED: u8 = 0x10;
const LCDC_WIN_B: u8 = 0x20;
const LCDC_WIN_AREA: u8 = 0x40;
const LCDC_ON_B: u8 = 0x80;

// STAT bits
const STAT_MODE_B: u8 = 0x3;
const STAT_LYC_B: u8 = 0x4;
const STAT_IF_HBLANK_B: u8 = 0x8;
const STAT_IF_VBLANK_B: u8 = 0x10;
const STAT_IF_OAM_B: u8 = 0x20;
const STAT_IF_LYC_B: u8 = 0x40;

#[non_exhaustive]
#[derive(Clone, Copy, Default)]
pub enum ColorCorrectionMode {
    CorrectCurves,
    Disabled,
    LowContrast,
    #[default]
    ModernBalanced,
    ModernBoostContrast,
    ReduceContrast,
}

/// PPU mode (matches the STAT register mode bits 0-1 and the
/// mooneye-gb state-machine mode names).
#[expect(
    clippy::arbitrary_source_item_ordering,
    reason = "Order follows the state machine transitions"
)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    HBlank = 0,
    VBlank = 1,
    OamScan = 2,
    Drawing = 3,
}

impl Mode {
    /// M-cycles (1 M-cycle = 4 T-cycles) for each mode. Inspired by
    /// mooneye-gb but with the original Ceres constants for the per-mode
    /// lengths. `scroll_x` only affects Mode 3 / Mode 0 split, not VBlank.
    /// The scroll adjustment differs between DMG/MGB/SGB/SGB2 and
    /// CGB/AGB/AGS (see SameBoy's display.c).
    const fn m_cycles(self, scroll_x: u8, cgb_mode: bool) -> i32 {
        // mooneye-gb "scroll adjustment": how many extra M-cycles Mode 3
        // gets and Mode 0 loses, based on the low 3 bits of SCX.
        let scroll_adjust: i32 = if cgb_mode {
            // CGB / AGB / AGS (verified against the wilbertpol-mooneye-gb
            // tests `hblank_ly_scx_timing-C`, `intr_2_mode0_scx1_timing`,
            // and SameBoy's display.c):
            //   0 0 0 1 1 1 1 2
            match scroll_x & 0x7 {
                3..=6 => 1,
                7 => 2,
                _ => 0,
            }
        } else {
            // DMG / MGB / SGB / SGB2
            match scroll_x & 0x7 {
                4..=7 => 1,
                _ => 0,
            }
        };
        // Original Ceres scanline constants, kept to maintain compatibility
        // with tests that were passing under the previous (batch-driven)
        // PPU. Total per line still = 114 M-cycles = 456 T-cycles.
        const OAM_M_CYCLES: i32 = 20;
        let vram_m_cycles: i32 = if cgb_mode { 43 } else { 43 };
        let hblank_m_cycles: i32 = 114 - OAM_M_CYCLES - vram_m_cycles;
        const VBLANK_M_CYCLES: i32 = 114;
        match self {
            Self::OamScan => OAM_M_CYCLES,
            Self::Drawing => vram_m_cycles + scroll_adjust,
            Self::HBlank => hblank_m_cycles - scroll_adjust,
            Self::VBlank => VBLANK_M_CYCLES,
        }
    }
}

#[expect(clippy::struct_excessive_bools)]
pub struct Ppu {
    bcp: ColorPalette,
    bgp: u8,
    color_correction_mode: ColorCorrectionMode,
    /// Whether this PPU instance runs in CGB/AGB/AGS mode (as opposed to
    /// DMG/MGB/SGB/SGB2 or CGB-in-compat-mode). Used to pick the per-model
    /// scroll-adjustment table.
    is_cgb: bool,
    /// M-cycles remaining until the PPU transitions to the next mode
    /// (or fires a mode-bound IRQ). Mirrors mooneye-gb's `cycles`.
    cycles: i32,
    lcdc: u8,
    ly: u8,
    /// LY value used for LYC coincidence comparison. Separate from `ly`
    /// because the real PPU updates the LYC comparator a few T-cycles
    /// after LY increments (SameBoy's `ly_for_comparison`).
    ly_for_comparison: u16,
    lyc: u8,
    oam: Oam,
    obp0: u8,
    obp1: u8,
    ocp: ColorPalette,
    opri: bool,
    rgb_buf: RgbaBuf,
    rgba_buf_present: RgbaBuf,
    scx: u8,
    scy: u8,
    stat: u8,
    vram: Vram,
    win_in_frame: bool,
    win_in_ly: bool,
    win_skipped: u8,
    wx: u8,
    wy: u8,
    lcdon_line0_mode0: bool,
    line0_frame_wrap: bool,
    mode_for_interrupt: Option<Mode>,
    stat_line: bool,
    sprite_penalty: i32,
}

impl Default for Ppu {
    fn default() -> Self {
        Self {
            bcp: ColorPalette::default(),
            bgp: 0,
            color_correction_mode: ColorCorrectionMode::default(),
            is_cgb: false,
            cycles: Mode::HBlank.m_cycles(0, false),
            lcdc: 0,
            ly: 0,
            ly_for_comparison: 0,
            lyc: 0,
            lcdon_line0_mode0: false,
            line0_frame_wrap: false,
            mode_for_interrupt: None,
            stat_line: false,
            sprite_penalty: 0,
            oam: Oam::default(),
            obp0: 0,
            obp1: 0,
            ocp: ColorPalette::default(),
            opri: false,
            rgb_buf: RgbaBuf::default(),
            rgba_buf_present: RgbaBuf::default(),
            scx: 0,
            scy: 0,
            stat: Mode::HBlank as u8,
            vram: Vram::default(),
            win_in_frame: false,
            win_in_ly: false,
            win_skipped: 0,
            wx: 0,
            wy: 0,
        }
    }
}

// IO
impl Ppu {
    #[must_use]
    pub const fn bcp(&self) -> &ColorPalette {
        &self.bcp
    }

    #[must_use]
    pub const fn bcp_mut(&mut self) -> &mut ColorPalette {
        &mut self.bcp
    }

    /// Re-evaluate LY=LYC coincidence and fire LYC STAT IRQ on rising edge.
    /// Uses `ly_for_comparison` (separate from `ly`) so the LYC comparator
    /// can update a few T-cycles after `ly` increments, matching SameBoy.
    fn update_stat_line(&mut self, ints: &mut Interrupts) {
        if self.lcdc & LCDC_ON_B == 0 {
            self.stat_line = false;
            return;
        }

        let lyc_signal = (self.stat & STAT_IF_LYC_B != 0) && (self.stat & STAT_LYC_B != 0);
        let effective_mode = self.mode_for_interrupt.unwrap_or(self.mode());
        let mode_signal = match effective_mode {
            Mode::HBlank => !self.lcdon_line0_mode0 && (self.stat & STAT_IF_HBLANK_B != 0),
            Mode::VBlank => self.stat & STAT_IF_VBLANK_B != 0,
            Mode::OamScan => self.stat & STAT_IF_OAM_B != 0,
            Mode::Drawing => false,
        };

        let new_line = lyc_signal || mode_signal;
        if new_line && !self.stat_line {
            ints.request_lcd();
        }
        self.stat_line = new_line;
    }

    fn check_lyc(&mut self, ints: &mut Interrupts) {
        if self.lcdc & LCDC_ON_B == 0 {
            return;
        }

        if self.ly_for_comparison == u16::from(self.lyc) {
            self.stat |= STAT_LYC_B;
        } else {
            self.stat &= !STAT_LYC_B;
        }

        self.update_stat_line(ints);
    }

    fn sprite_penalty_m_cycles(&self, cgb_mode: CgbMode) -> i32 {
        if self.lcdc & LCDC_OBJ_B == 0 {
            return 0;
        }

        let height = if self.lcdc & LCDC_OBJL_B == 0 { 8 } else { 16 };
        let (objs, count) = self.objs_in_ly(height, cgb_mode);
        if count == 0 {
            return 0;
        }

        let mut total_t_cycles = 0;
        let mut last_tile_x = -1;

        for obj in &objs[..count as usize] {
            let x = obj.x;
            if x == 0 || x >= 168 {
                continue;
            }

            // Base sprite fetch penalty: 6 T-cycles (2 T OAM + 4 T VRAM)
            total_t_cycles += 6;

            // Fetcher alignment penalty (0..5 T-cycles for mid-tile sprites)
            let tile_x = (x.wrapping_add(self.scx) / 8) as i32;
            if tile_x != last_tile_x {
                let scroll_offset = (x.wrapping_add(self.scx) & 7) as i32;
                if scroll_offset > 0 && scroll_offset <= 5 {
                    total_t_cycles += scroll_offset;
                }
                last_tile_x = tile_x;
            }
        }

        (total_t_cycles + 2) / 4
    }

    /// Transition the PPU to a new mode, reset the per-mode cycle counter,
    /// and fire any mode-bound IRQs.
    fn enter_mode(&mut self, mode: Mode, ints: &mut Interrupts, cgb_mode: CgbMode) {
        if mode == Mode::Drawing {
            self.sprite_penalty = self.sprite_penalty_m_cycles(cgb_mode);
        } else if mode != Mode::HBlank {
            self.sprite_penalty = 0;
        }

        let base_cycles = mode.m_cycles(self.scx, self.is_cgb);
        self.cycles = match mode {
            Mode::Drawing => base_cycles + self.sprite_penalty,
            Mode::HBlank => (base_cycles - self.sprite_penalty).max(1),
            _ => base_cycles,
        };
        self.mode_for_interrupt = None;
        // Update mode bits AFTER setting cycles so cgb_mode is queried here.
        self.stat = (self.stat & !STAT_MODE_B) | mode as u8;

        match mode {
            Mode::OamScan => {
                self.win_in_ly = false;
                self.ly_for_comparison = u16::from(self.ly);
                self.check_lyc(ints);
            }
            Mode::VBlank => {
                self.ly = 144;
                self.ly_for_comparison = 144;
                ints.request_vblank();
                self.win_skipped = 0;
                self.win_in_frame = false;

                // DMG quirk: entering VBlank triggers the OAM STAT interrupt (SameBoy display.c:2173)
                if !self.is_cgb && (self.stat & STAT_IF_OAM_B != 0) && !self.stat_line {
                    ints.request_lcd();
                }
            }
            Mode::Drawing | Mode::HBlank => (),
        }

        self.update_stat_line(ints);
        self.check_lyc(ints);
    }

    #[must_use]
    pub const fn mode(&self) -> Mode {
        match self.stat & STAT_MODE_B {
            0 => Mode::HBlank,
            1 => Mode::VBlank,
            2 => Mode::OamScan,
            _ => Mode::Drawing,
        }
    }

    #[must_use]
    pub const fn ocp(&self) -> &ColorPalette {
        &self.ocp
    }

    #[must_use]
    pub const fn ocp_mut(&mut self) -> &mut ColorPalette {
        &mut self.ocp
    }

    #[must_use]
    pub const fn pixel_data_rgba(&self) -> &[u8] {
        self.rgba_buf_present.pixel_data()
    }

    #[must_use]
    pub const fn read_bgp(&self) -> u8 {
        self.bgp
    }

    #[must_use]
    pub const fn read_lcdc(&self) -> u8 {
        self.lcdc
    }

    #[must_use]
    pub const fn read_ly(&self) -> u8 {
        self.ly
    }

    #[must_use]
    pub const fn read_lyc(&self) -> u8 {
        self.lyc
    }

    #[must_use]
    pub const fn read_obp0(&self) -> u8 {
        self.obp0
    }

    #[must_use]
    pub const fn read_obp1(&self) -> u8 {
        self.obp1
    }

    #[must_use]
    pub const fn read_opri(&self) -> u8 {
        self.opri as u8 | 0xFE
    }

    #[must_use]
    pub const fn read_scx(&self) -> u8 {
        self.scx
    }

    #[must_use]
    pub const fn read_scy(&self) -> u8 {
        self.scy
    }

    #[must_use]
    pub const fn read_stat(&self) -> u8 {
        self.stat | 0x80
    }

    #[must_use]
    pub const fn read_wx(&self) -> u8 {
        self.wx
    }

    #[must_use]
    pub const fn read_wy(&self) -> u8 {
        self.wy
    }

    /// Advance the PPU by one M-cycle (4 T-cycles). This matches
    /// mooneye-gb's `emulate()`: each call consumes one M-cycle of
    /// the current mode, fires any pending IRQs, and switches modes
    /// when the per-mode cycle budget runs out.
    pub fn tick_m_cycle(&mut self, ints: &mut Interrupts, cgb_mode: CgbMode) {
        // Cache whether we're running in CGB native mode so per-model
        // timing decisions can be made without threading CgbMode
        // through every internal call.
        self.is_cgb = matches!(cgb_mode, CgbMode::Cgb);
        if self.lcdc & LCDC_ON_B == 0 {
            return;
        }


        // Mid-scanline comparator / glitch events:
        match self.mode() {
            Mode::OamScan => {
                if self.cycles == 19 {
                    self.ly_for_comparison = u16::from(self.ly);
                    self.check_lyc(ints);
                }
            }
            Mode::HBlank => {
                if self.cycles == 1 && !self.lcdon_line0_mode0 {
                    // Mode 2 STAT IRQ fires 1 M-cycle BEFORE Mode 2 begins (SameBoy line 1780)
                    if self.stat & STAT_IF_OAM_B != 0 && !self.stat_line {
                        ints.request_lcd();
                        self.stat_line = true;
                    }
                }
            }
            Mode::VBlank => {
                if self.ly == 153 {
                    // Line 153 timing phases (SameBoy display.c:2217):
                    // self.cycles = 114: LY=153, ly_for_comparison = MAX
                    // self.cycles = 113: LY=0,   ly_for_comparison = 153
                    // self.cycles = 112: LY=0,   ly_for_comparison = MAX
                    // self.cycles = 111: LY=0,   ly_for_comparison = 0
                    if self.cycles == 113 {
                        self.ly = 0;
                        self.ly_for_comparison = 153;
                        self.check_lyc(ints);
                    } else if self.cycles == 112 {
                        self.ly = 0;
                        self.ly_for_comparison = u16::MAX;
                        self.check_lyc(ints);
                    }
                } else if self.ly >= 144 {
                    let base_cycles = Mode::VBlank.m_cycles(self.scx, self.is_cgb);
                    if self.cycles == base_cycles - 1 {
                        self.ly_for_comparison = u16::from(self.ly);
                        self.check_lyc(ints);
                    }
                }
            }
            Mode::Drawing => {
                if self.cycles == 1 {
                    // Mode 0 HBlank STAT IRQ fires 1 M-cycle BEFORE Mode 0 begins (mooneye-gb ppu.rs:326)
                    if self.stat & STAT_IF_HBLANK_B != 0 && !self.stat_line {
                        ints.request_lcd();
                        self.stat_line = true;
                    }
                }
            }
        }

        self.cycles -= 1;

        if self.cycles > 0 {
            return;
        }

        match self.mode() {
            Mode::OamScan => self.enter_mode(Mode::Drawing, ints, cgb_mode),
            Mode::Drawing => {
                self.draw_scanline(cgb_mode);
                self.enter_mode(Mode::HBlank, ints, cgb_mode);
            }
            Mode::HBlank => {
                if self.lcdon_line0_mode0 {
                    self.lcdon_line0_mode0 = false;
                    self.enter_mode(Mode::Drawing, ints, cgb_mode);
                } else if self.line0_frame_wrap {
                    self.line0_frame_wrap = false;
                    self.enter_mode(Mode::OamScan, ints, cgb_mode);
                    self.ly_for_comparison = 0;
                    self.check_lyc(ints);
                } else {
                    self.ly += 1;
                    if self.ly > 143 {
                        self.enter_mode(Mode::VBlank, ints, cgb_mode);
                    } else {
                        self.enter_mode(Mode::OamScan, ints, cgb_mode);
                    }
                }
            }
            Mode::VBlank => {
                if self.ly >= 153 || self.ly < 144 {
                    self.ly = 0;
                    self.rgba_buf_present = mem::take(&mut self.rgb_buf);
                    if self.is_cgb {
                        // On CGB: direct Mode 1 -> Mode 2 transition
                        self.enter_mode(Mode::OamScan, ints, cgb_mode);
                        self.ly_for_comparison = 0;
                        self.check_lyc(ints);
                    } else {
                        // On DMG/MGB: 1-M-cycle Mode 0 glitch on line 0
                        self.stat = (self.stat & !STAT_MODE_B) | Mode::HBlank as u8;
                        self.cycles = 1;
                        self.line0_frame_wrap = true;
                    }
                } else {
                    self.ly += 1;
                    let base_cycles = Mode::VBlank.m_cycles(self.scx, self.is_cgb);
                    self.cycles = if !self.is_cgb && self.ly == 152 {
                        base_cycles - 1
                    } else {
                        base_cycles
                    };
                    self.ly_for_comparison = if self.ly == 153 {
                        u16::MAX
                    } else {
                        u16::MAX
                    };
                    self.check_lyc(ints);
                }
            }
        }
    }

    pub const fn set_color_correction_mode(&mut self, mode: ColorCorrectionMode) {
        self.color_correction_mode = mode;
    }

    pub fn write_lcdc(&mut self, val: u8, ints: &mut Interrupts) {
        let was_on = self.lcdc & LCDC_ON_B != 0;
        let is_on = val & LCDC_ON_B != 0;
        self.lcdc = val;

        // turn off: reset to line 0 in HBlank mode, clear all blocking.
        if !is_on && was_on {
            self.ly = 0;
            self.ly_for_comparison = 0;
            self.lcdon_line0_mode0 = false;
            self.stat &= !STAT_MODE_B;
            self.stat_line = false;
            self.cycles = Mode::HBlank.m_cycles(self.scx, self.is_cgb);
            self.rgba_buf_present.clear();
            // LYC comparison: re-evaluate after LY reset to 0.
            self.check_lyc(ints);
        }

        // turn on: per the lcdon_mode_timing test, the first line starts
        // in mode 0 (HBlank) for 20 M-cycles, then goes straight to
        // mode 3 (skipping mode 2).
        if is_on && !was_on {
            self.ly = 0;
            self.ly_for_comparison = 0;
            self.stat = (self.stat & !STAT_MODE_B) | Mode::HBlank as u8;
            self.cycles = 20;
            self.lcdon_line0_mode0 = true;
            self.check_lyc(ints);
        }
    }

    pub fn write_lyc(&mut self, val: u8, ints: &mut Interrupts) {
        self.lyc = val;
        self.check_lyc(ints);
    }

    pub const fn write_bgp(&mut self, val: u8) {
        self.bgp = val;
    }

    pub const fn write_obp0(&mut self, val: u8) {
        self.obp0 = val;
    }

    pub const fn write_obp1(&mut self, val: u8) {
        self.obp1 = val;
    }

    pub const fn write_opri(&mut self, val: u8) {
        self.opri = val & 1 != 0;
    }

    pub fn write_scx(&mut self, val: u8) {
        if self.mode() == Mode::Drawing {
            let old_mode3 = Mode::Drawing.m_cycles(self.scx, self.is_cgb);
            let new_mode3 = Mode::Drawing.m_cycles(val, self.is_cgb);
            self.cycles += new_mode3 - old_mode3;
        }
        self.scx = val;
    }

    pub(crate) const fn set_stat(&mut self, val: u8) {
        self.stat = val;
    }

    pub(crate) const fn set_ly(&mut self, val: u8) {
        self.ly = val;
        self.ly_for_comparison = val as u16;
    }

    pub(crate) const fn set_cycles(&mut self, val: i32) {
        self.cycles = val;
    }

    pub const fn write_scy(&mut self, val: u8) {
        self.scy = val;
    }

    pub fn write_stat(&mut self, val: u8, ints: &mut Interrupts, is_cgb: bool) {
        let ly_equals_lyc = self.stat & STAT_LYC_B;
        let mode = self.stat & STAT_MODE_B;

        self.stat = (val & !0x07) | ly_equals_lyc | mode;

        if !is_cgb && self.lcdc & LCDC_ON_B != 0 && !self.stat_line {
            // DMG STAT write glitch: writing to STAT while in Mode 0, Mode 1, or when LY=LYC is active
            // pulses the STAT line high if it was previously low.
            if self.mode() == Mode::HBlank
                || self.mode() == Mode::VBlank
                || (self.stat & STAT_LYC_B != 0)
            {
                ints.request_lcd();
                self.stat_line = true;
            }
        }

        self.update_stat_line(ints);
    }

    pub const fn write_wx(&mut self, val: u8) {
        self.wx = val;
    }

    pub const fn write_wy(&mut self, val: u8) {
        self.wy = val;
    }

    /// Scanline renderer: CGB palettes are always accessible to the CPU.
    /// Kept as a no-op stub for the restored API so callers still compile.
    #[inline]
    #[must_use]
    pub const fn is_cgb_palettes_accessible(&self) -> bool {
        true
    }

    /// Scanline renderer has no STOP-mode state to track. Stub for API.
    #[inline]
    pub const fn enter_stop_mode(&mut self) {}

    /// Scanline renderer has no STOP-mode state to track. Stub for API.
    #[inline]
    pub const fn leave_stop_mode(&mut self) {}
}
