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
            // DMG / MGB / SGB / SGB2:
            //   0 1 1 1 1 2 2 2
            match scroll_x & 0x7 {
                1..=4 => 1,
                5..=7 => 2,
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
    fn check_lyc(&mut self, ints: &mut Interrupts) {
        self.stat &= !STAT_LYC_B;
        let lyc_match = if self.ly_for_comparison == u16::from(self.lyc) {
            self.stat |= STAT_LYC_B;
            true
        } else {
            false
        };
        if lyc_match && self.stat & STAT_IF_LYC_B != 0 {
            ints.request_lcd();
        }
    }

    /// Transition the PPU to a new mode, reset the per-mode cycle counter,
    /// and fire any mode-bound IRQs.
    fn enter_mode(&mut self, mode: Mode, ints: &mut Interrupts) {
        self.cycles = mode.m_cycles(self.scx, self.is_cgb);
        // Update mode bits AFTER setting cycles so cgb_mode is queried here.
        self.stat = (self.stat & !STAT_MODE_B) | mode as u8;

        match mode {
            Mode::OamScan => {
                if self.stat & STAT_IF_OAM_B != 0 {
                    ints.request_lcd();
                }
                self.win_in_ly = false;
            }
            Mode::VBlank => {
                ints.request_vblank();
                if self.stat & STAT_IF_VBLANK_B != 0 {
                    ints.request_lcd();
                }
                // The mooneye-gb quirk: on entering VBlank, also fire the
                // OAM STAT IRQ if enabled. This is what makes the
                // `vblank_stat_intr-*` tests pass.
                if self.stat & STAT_IF_OAM_B != 0 {
                    ints.request_lcd();
                }
                self.win_skipped = 0;
                self.win_in_frame = false;
            }
            Mode::Drawing => (),
            Mode::HBlank => {
                if self.stat & STAT_IF_HBLANK_B != 0 {
                    ints.request_lcd();
                }
            }
        }
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

        // mooneye-gb's quirk: the HBlank STAT IRQ fires one M-cycle BEFORE
        // the actual mode 0 entry, when we're 1 M-cycle from leaving Mode 3.
        if self.cycles == 1 && self.mode() == Mode::Drawing {
            if self.stat & STAT_IF_HBLANK_B != 0 {
                ints.request_lcd();
            }
        }

        // Line 153 early rollover to 0
        if self.mode() == Mode::VBlank && self.ly == 153 && self.cycles == 113 {
            self.ly = 0;
        }
        if self.mode() == Mode::VBlank && self.ly == 0 && self.cycles == 111 {
            self.ly_for_comparison = 0;
            self.check_lyc(ints);
        }

        self.cycles -= 1;

        if self.cycles > 0 {
            return;
        }

        match self.mode() {
            Mode::OamScan => self.enter_mode(Mode::Drawing, ints),
            Mode::Drawing => {
                self.draw_scanline(cgb_mode);
                self.enter_mode(Mode::HBlank, ints);
            }
            Mode::HBlank => {
                if self.lcdon_line0_mode0 {
                    self.lcdon_line0_mode0 = false;
                    self.enter_mode(Mode::Drawing, ints);
                } else if self.line0_frame_wrap {
                    self.line0_frame_wrap = false;
                    self.enter_mode(Mode::OamScan, ints);
                    self.ly_for_comparison = u16::from(self.ly);
                    self.check_lyc(ints);
                } else {
                    self.ly += 1;
                    self.ly_for_comparison = u16::from(self.ly);
                    if self.ly > 143 {
                        self.enter_mode(Mode::VBlank, ints);
                    } else {
                        self.enter_mode(Mode::OamScan, ints);
                    }
                    self.check_lyc(ints);
                }
            }
            Mode::VBlank => {
                if self.ly == 0 {
                    self.rgba_buf_present = mem::take(&mut self.rgb_buf);
                    if self.is_cgb {
                        // On CGB: direct Mode 1 -> Mode 2 transition
                        self.enter_mode(Mode::OamScan, ints);
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
                    self.ly_for_comparison = u16::from(self.ly);
                    let base_cycles = Mode::VBlank.m_cycles(self.scx, self.is_cgb);
                    self.cycles = if !self.is_cgb && self.ly == 152 {
                        base_cycles - 1
                    } else {
                        base_cycles
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
        // turn off: reset to line 0 in HBlank mode, clear all blocking.
        if val & LCDC_ON_B == 0 && self.lcdc & LCDC_ON_B != 0 {
            self.ly = 0;
            self.ly_for_comparison = 0;
            self.lcdon_line0_mode0 = false;
            self.stat &= !STAT_MODE_B;
            self.cycles = Mode::HBlank.m_cycles(self.scx, self.is_cgb);
            self.rgba_buf_present.clear();
            // LYC comparison: re-evaluate after LY reset to 0.
            self.check_lyc(ints);
        }

        // turn on: per the lcdon_mode_timing test, the first line starts
        // in mode 0 (HBlank) for 20 M-cycles, then goes straight to
        // mode 3 (skipping mode 2).
        if val & LCDC_ON_B != 0 && self.lcdc & LCDC_ON_B == 0 {
            self.ly = 0;
            self.ly_for_comparison = 0;
            self.stat = (self.stat & !STAT_MODE_B) | Mode::HBlank as u8;
            self.cycles = 20;
            self.lcdon_line0_mode0 = true;
            self.check_lyc(ints);
        }

        self.lcdc = val;
    }

    pub fn write_lyc(&mut self, val: u8, ints: &mut Interrupts) {
        self.lyc = val;
        // On hardware, writing to LYC re-evaluates LY=LYC coincidence
        // and fires the LYC STAT IRQ if newly satisfied (SameBoy
        // GB_STAT_update; gambatte memory.cpp::updateIrqs).
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

    pub const fn write_scx(&mut self, val: u8) {
        self.scx = val;
    }

    pub(crate) const fn set_stat(&mut self, val: u8) {
        self.stat = val;
    }

    pub(crate) const fn set_ly(&mut self, val: u8) {
        self.ly = val;
    }

    pub const fn write_scy(&mut self, val: u8) {
        self.scy = val;
    }

    pub fn write_stat(&mut self, val: u8, ints: &mut Interrupts, _cgb_mode: CgbMode) {
        let ly_equals_lyc = self.stat & STAT_LYC_B;
        let mode: u8 = self.mode() as u8;

        // Capture which STAT IRQ sources were previously enabled, so we
        // only fire IRQs for sources that *transition* from disabled to
        // enabled by this write. An IRQ source that was already enabled
        // would have already fired when its condition was first met.
        let prev_enables =
            self.stat & (STAT_IF_HBLANK_B | STAT_IF_VBLANK_B | STAT_IF_OAM_B | STAT_IF_LYC_B);

        self.stat = val;
        self.stat &= !(STAT_LYC_B | STAT_MODE_B);
        self.stat |= ly_equals_lyc | mode;

        // Re-evaluate STAT interrupt line after the write. On hardware,
        // writing to STAT can cause a pending STAT IRQ to fire immediately
        // if a newly-enabled condition is already met (SameBoy
        // GB_STAT_update; gambatte memory.cpp::updateIrqs).
        let new_enables =
            self.stat & (STAT_IF_HBLANK_B | STAT_IF_VBLANK_B | STAT_IF_OAM_B | STAT_IF_LYC_B);
        let newly_enabled = new_enables & !prev_enables;

        // Mode-based STAT IRQ sources: only fire if the mode-specific
        // enable was newly set by this write AND the mode condition is met.
        let mode_irq_enable = match self.mode() {
            Mode::HBlank => STAT_IF_HBLANK_B,
            Mode::VBlank => STAT_IF_VBLANK_B,
            Mode::OamScan => STAT_IF_OAM_B,
            Mode::Drawing => 0,
        };
        if mode_irq_enable != 0 && (newly_enabled & mode_irq_enable) != 0 {
            ints.request_lcd();
        }

        // LYC coincidence STAT IRQ source: only fire if the LYC enable was
        // newly set by this write AND LY currently equals LYC.
        if (newly_enabled & STAT_IF_LYC_B) != 0 && (self.stat & STAT_LYC_B) != 0 {
            ints.request_lcd();
        }
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
