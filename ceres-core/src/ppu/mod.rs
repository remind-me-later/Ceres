mod color_palette;
mod draw;
mod fifo;
mod oam;
mod rgba_buf;
mod vram;

use core::mem;

use crate::interrupts::Interrupts;
pub use fifo::PixelFifo;
pub use oam::Oam;
pub use vram::Vram;
use {self::color_palette::ColorPalette, crate::CgbMode, crate::Model, rgba_buf::RgbaBuf};

pub const PX_WIDTH: u8 = 160;
pub const PX_HEIGHT: u8 = 144;

// LCDC bits
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    HBlank = 0,
    VBlank = 1,
    OamScan = 2,
    Drawing = 3,
}

#[expect(clippy::struct_excessive_bools)]
pub struct Ppu {
    pub model: Model,
    bcp: ColorPalette,
    bgp: u8,
    color_correction_mode: ColorCorrectionMode,
    /// Whether this PPU instance runs in CGB/AGB/AGS mode (as opposed to
    /// DMG/MGB/SGB/SGB2 or CGB-in-compat-mode). Used to pick the per-model
    /// scroll-adjustment table.
    is_cgb: bool,
    /// Forward master dot counter within the current scanline (0..456).
    pub line_dot: u16,
    /// The dot coordinate where HBlank was entered on this scanline.
    pub hblank_start_dot: u16,
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
    lcdon_line0: bool,
    line0_frame_wrap: bool,
    /// The PPU's true internal mode. Kept as a field (rather than derived
    /// from the STAT bits) so the CPU-visible mode bits can be decoupled
    /// from the internal state later — SameBoy's `display_state` vs
    /// `GB_IO_STAT`.
    mode: Mode,
    mode_for_interrupt: Option<Mode>,
    stat_line: bool,
    current_vblank_line: u8,
    fifo: fifo::PixelFifo,
    lyc_latched: u8,
}

impl Default for Ppu {
    fn default() -> Self {
        Self {
            model: Model::default(),
            bcp: ColorPalette::default(),
            bgp: 0,
            color_correction_mode: ColorCorrectionMode::default(),
            is_cgb: false,
            line_dot: 0,
            hblank_start_dot: 0,
            lcdc: 0,
            ly: 0,
            ly_for_comparison: 0,
            lyc: 0,
            lyc_latched: 0,
            lcdon_line0_mode0: false,
            lcdon_line0: false,
            line0_frame_wrap: false,
            mode: Mode::HBlank,
            mode_for_interrupt: None,
            stat_line: false,
            current_vblank_line: 0,
            fifo: fifo::PixelFifo::new(),
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
    pub fn new(model: Model) -> Self {
        let mut ppu = Self {
            model,
            ..Self::default()
        };

        if matches!(
            model,
            Model::Cgb0 | Model::CgbA | Model::CgbB | Model::CgbC | Model::CgbD | Model::CgbE
        ) {
            ppu.bcp.init_compat_palette();
            ppu.ocp.init_compat_palette();
        }

        ppu
    }

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
        let mode_signal = match self.mode_for_interrupt {
            Some(Mode::HBlank) => !self.lcdon_line0_mode0 && (self.stat & STAT_IF_HBLANK_B != 0),
            Some(Mode::VBlank) => self.stat & STAT_IF_VBLANK_B != 0,
            Some(Mode::OamScan) => self.stat & STAT_IF_OAM_B != 0,
            Some(Mode::Drawing) | None => match self.mode() {
                Mode::HBlank => {
                    let cgb_hblank_delayed = self.is_cgb && self.line_dot <= self.hblank_start_dot;
                    !self.lcdon_line0_mode0
                        && !cgb_hblank_delayed
                        && (self.stat & STAT_IF_HBLANK_B != 0)
                }
                Mode::VBlank => self.stat & STAT_IF_VBLANK_B != 0,
                Mode::OamScan | Mode::Drawing => false,
            },
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

    /// Transition the PPU to a new mode and fire any mode-bound IRQs.
    fn enter_mode(&mut self, mode: Mode, ints: &mut Interrupts, cgb_mode: CgbMode) {
        self.is_cgb = matches!(cgb_mode, CgbMode::Cgb | CgbMode::Compat);
        self.mode_for_interrupt = None;
        self.mode = mode;
        self.stat = (self.stat & !STAT_MODE_B) | mode as u8;

        match mode {
            Mode::OamScan => {
                self.win_in_ly = false;
                self.ly_for_comparison = u16::from(self.ly);
                self.check_lyc(ints);
            }
            Mode::Drawing => {
                // The sprite scan runs at mode-2 END, not entry: hardware
                // reads OAM incrementally through mode 2 (dots 4-84), so a
                // mode-2 ISR that writes OAM (e.g. the intr_2_mode0_timing_sprites
                // setup) is still seen by the not-yet-scanned entries.
                // Scanning at mode-3 entry sees every mode-2 write.
                let sprite_height = if self.lcdc & 0x04 != 0 { 16 } else { 8 };
                self.fifo.scan_sprites(&self.oam, self.ly, sprite_height);
                let obj_enabled = self.lcdc & 0x02 != 0 || self.is_cgb;
                self.fifo.start_drawing(
                    self.scx,
                    !self.is_cgb && self.lcdon_line0_mode0,
                    self.is_cgb,
                    obj_enabled,
                    self.model,
                    self.wx,
                    self.lcdc,
                    self.wy,
                    self.ly,
                );
            }
            Mode::VBlank => {
                self.current_vblank_line = 144;
                self.ly = 144;
                self.ly_for_comparison = 144;
                self.win_skipped = 0;
                self.win_in_frame = false;
                self.fifo.reset_window_frame();
            }
            Mode::HBlank => (),
        }

        self.update_stat_line(ints);
        self.check_lyc(ints);
    }

    #[must_use]
    pub const fn mode(&self) -> Mode {
        self.mode
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
        if self.is_cgb {
            if matches!(self.mode(), Mode::VBlank)
                && self.current_vblank_line == 153
                && self.line_dot == 448
            {
                145
            } else if matches!(self.mode(), Mode::HBlank)
                && !self.lcdon_line0_mode0
                && self.line_dot >= 449
                && self.ly < 143
            {
                self.ly + 1
            } else {
                self.ly
            }
        } else {
            self.ly
        }
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
        if self.is_cgb
            && matches!(self.mode, Mode::Drawing)
            && (self.scx & 3 == 3)
            && self.fifo.position() >= 156
        {
            (self.stat & !STAT_MODE_B) | (Mode::HBlank as u8) | 0x80
        } else {
            self.stat | 0x80
        }
    }

    #[must_use]
    pub const fn read_wx(&self) -> u8 {
        self.wx
    }

    #[must_use]
    pub const fn read_wy(&self) -> u8 {
        self.wy
    }

    /// Step the pixel FIFO one dot and write the produced pixel (if any)
    /// into the framebuffer.
    fn step_fifo_dot(&mut self, cgb_mode: CgbMode) {
        if let Some((lx, bg_px, sprite_px)) = self.fifo.step_dot(
            &self.vram,
            self.ly,
            self.wx,
            self.wy,
            self.scx,
            self.scy,
            self.lcdc,
            cgb_mode == CgbMode::Cgb,
        ) {
            let rgb = self.resolve_fifo_pixel(bg_px, sprite_px, cgb_mode);
            if self.ly < 144 {
                let idx = u32::from(self.ly) * u32::from(PX_WIDTH) + u32::from(lx);
                self.rgb_buf.set_px(idx, rgb);
            }
        }
    }

    /// Advance the PPU by one T-cycle (dot). Mode 3's length is owned by
    /// the pixel FIFO: the Drawing arm steps the FIFO one dot per call and
    /// leaves for HBlank as soon as the scanline is complete, so mid-line
    /// register writes land at exact dots and sprite/window stalls extend
    /// the line naturally. All modes are clocked forward dot-by-dot against
    /// `line_dot` (0..456).
    pub fn tick_t_cycle(&mut self, ints: &mut Interrupts, cgb_mode: CgbMode) {
        self.is_cgb = matches!(cgb_mode, CgbMode::Cgb | CgbMode::Compat);
        if self.lcdc & LCDC_ON_B == 0 {
            return;
        }

        let dot = self.line_dot;
        let line_len = if self.lcdon_line0 && !self.is_cgb {
            452
        } else {
            456
        };
        // Hardware scanline transition pipeline phases:
        // - early_ly_inc_dot (line_len - 8): LY increment on line 143 (CGB) or DMG with (scx & 3) != 0; CGB LYC latching.
        // - early_mode2_stat_dot (line_len - 4): CGB Mode 2 STAT IRQ / Mode 2 STAT bits; DMG early LY increment if (scx & 3) == 0.
        // - line_end_dot (line_len - 1): Scanline wrap to dot 0; DMG VBlank IRQ on line 143.
        let early_ly_inc_dot = line_len - 8;
        let early_mode2_stat_dot = line_len - 4;
        let line_end_dot = line_len - 1;

        // Mid-scanline comparator / glitch events:
        match self.mode() {
            Mode::OamScan => {
                if dot == 4 {
                    self.ly_for_comparison = u16::from(self.ly);
                    self.check_lyc(ints);
                }
                if self.is_cgb
                    && self.ly == 0
                    && dot == 0
                    && self.stat & STAT_IF_OAM_B != 0
                    && !self.stat_line
                {
                    ints.request_lcd();
                    self.stat_line = true;
                }
                if !self.is_cgb
                    && self.ly == 0
                    && dot == 1
                    && self.stat & STAT_IF_OAM_B != 0
                    && !self.stat_line
                {
                    ints.request_lcd();
                    self.stat_line = true;
                }
            }
            Mode::HBlank => {
                if self.is_cgb && dot == self.hblank_start_dot + 1 {
                    self.update_stat_line(ints);
                }
                if (self.ly == 143 || (self.ly == 144 && self.mode() == Mode::HBlank))
                    && self.ly_for_comparison != u16::MAX
                {
                    if !self.is_cgb {
                        if dot == early_mode2_stat_dot {
                            self.ly = 144;
                            self.ly_for_comparison = 143;
                            self.check_lyc(ints);
                            if self.stat & STAT_IF_OAM_B != 0 && !self.stat_line {
                                ints.request_lcd();
                                self.stat_line = true;
                            }
                        } else if dot == line_end_dot {
                            self.stat = (self.stat & !STAT_MODE_B) | Mode::VBlank as u8;
                            self.ly_for_comparison = 144;
                            self.check_lyc(ints);
                            ints.request_vblank();
                            if self.stat & STAT_IF_VBLANK_B != 0 && !self.stat_line {
                                ints.request_lcd();
                                self.stat_line = true;
                            }
                        }
                    } else if dot == early_ly_inc_dot {
                        self.ly = 144;
                        self.ly_for_comparison = 143;
                        self.check_lyc(ints);
                        if self.stat & STAT_IF_OAM_B != 0 && !self.stat_line {
                            ints.request_lcd();
                            self.stat_line = true;
                        }
                    } else if dot == early_mode2_stat_dot {
                        self.stat = (self.stat & !STAT_MODE_B) | Mode::VBlank as u8;
                        self.ly_for_comparison = 144;
                        self.check_lyc(ints);
                        ints.request_vblank();
                        if self.stat & STAT_IF_VBLANK_B != 0 && !self.stat_line {
                            ints.request_lcd();
                            self.stat_line = true;
                        }
                    }
                } else if dot == early_ly_inc_dot && !self.lcdon_line0_mode0 {
                    if self.is_cgb && self.ly < 143 {
                        self.lyc_latched = self.lyc;
                        self.ly_for_comparison = u16::from(self.ly);
                        if self.ly_for_comparison == u16::from(self.lyc) {
                            self.stat |= STAT_LYC_B;
                        } else {
                            self.stat &= !STAT_LYC_B;
                        }
                    } else if !self.is_cgb && self.ly < 143 && (self.scx & 3) != 0 {
                        self.ly += 1;
                        self.ly_for_comparison = u16::MAX;
                        self.check_lyc(ints);
                    }
                } else if dot == early_mode2_stat_dot && !self.lcdon_line0_mode0 {
                    if self.ly < 143 {
                        if self.is_cgb {
                            self.stat = (self.stat & !STAT_MODE_B) | Mode::OamScan as u8;
                            self.ly_for_comparison = u16::from(self.ly + 1);
                            if self.ly_for_comparison == u16::from(self.lyc) {
                                self.stat |= STAT_LYC_B;
                            } else {
                                self.stat &= !STAT_LYC_B;
                            }
                            if self.lyc_latched == self.ly + 1 {
                                if (self.stat & STAT_IF_LYC_B) != 0 && !self.stat_line {
                                    ints.request_lcd();
                                    self.stat_line = true;
                                }
                            }
                            if self.stat & STAT_IF_OAM_B != 0 && !self.stat_line {
                                ints.request_lcd();
                                self.stat_line = true;
                            }
                        } else if (self.scx & 3) == 0 {
                            self.ly += 1;
                            self.ly_for_comparison = u16::MAX;
                            self.check_lyc(ints);
                        }
                    }
                }

                if !self.is_cgb
                    && dot == 449
                    && !self.lcdon_line0_mode0
                    && (self.ly < 143 || (self.ly == 143 && self.ly_for_comparison == u16::MAX))
                    && self.stat & STAT_IF_OAM_B != 0
                    && !self.stat_line
                {
                    ints.request_lcd();
                    self.stat_line = true;
                }
            }
            Mode::VBlank => {
                if self.current_vblank_line == 153 {
                    // Line 153 timing phases (SameBoy display.c:2217):
                    if self.is_cgb {
                        if dot == 0 {
                            self.ly = 0;
                            self.lyc_latched = self.lyc;
                            self.ly_for_comparison = 153;
                            if self.ly_for_comparison == u16::from(self.lyc) {
                                self.stat |= STAT_LYC_B;
                            } else {
                                self.stat &= !STAT_LYC_B;
                            }
                        } else if dot == 4 {
                            self.ly = 0;
                            self.ly_for_comparison = 0;
                            if self.lyc_latched == 0 {
                                if (self.stat & STAT_IF_LYC_B) != 0 && !self.stat_line {
                                    ints.request_lcd();
                                    self.stat_line = true;
                                }
                            }
                            if self.ly_for_comparison == u16::from(self.lyc) {
                                self.stat |= STAT_LYC_B;
                            } else {
                                self.stat &= !STAT_LYC_B;
                            }
                        } else if dot == 452 {
                            self.stat = (self.stat & !STAT_MODE_B) | Mode::OamScan as u8;
                        }
                    } else if dot == 4 {
                        self.ly = 0;
                        self.ly_for_comparison = u16::MAX;
                        self.check_lyc(ints);
                    } else if dot == 8 {
                        self.ly = 0;
                        self.ly_for_comparison = 0;
                        self.check_lyc(ints);
                    } else if dot == 452 {
                        self.stat = (self.stat & !STAT_MODE_B) | Mode::HBlank as u8;
                        self.mode = Mode::HBlank;
                        self.line0_frame_wrap = true;
                        self.ly_for_comparison = 0;
                        self.check_lyc(ints);
                    }
                } else if self.current_vblank_line >= 144 {
                    if dot == 4 {
                        self.ly_for_comparison = u16::from(self.ly);
                        self.check_lyc(ints);
                    } else if dot == 448 {
                        if self.is_cgb {
                            if self.current_vblank_line < 152 {
                                self.ly = self.current_vblank_line + 1;
                            } else {
                                self.ly = 153;
                                self.lyc_latched = self.lyc;
                                self.ly_for_comparison = 152;
                                if self.ly_for_comparison == u16::from(self.lyc) {
                                    self.stat |= STAT_LYC_B;
                                } else {
                                    self.stat &= !STAT_LYC_B;
                                }
                            }
                        }
                    } else if dot == 452 {
                        if self.current_vblank_line < 152 {
                            if self.is_cgb {
                                self.ly_for_comparison = u16::from(self.current_vblank_line + 1);
                                self.check_lyc(ints);
                            } else {
                                self.ly = self.current_vblank_line + 1;
                                self.ly_for_comparison = u16::MAX;
                                self.check_lyc(ints);
                            }
                        } else if self.is_cgb {
                            self.ly_for_comparison = 153;
                            if self.lyc_latched == 153 || self.lyc == 153 {
                                if (self.stat & STAT_IF_LYC_B) != 0 && !self.stat_line {
                                    ints.request_lcd();
                                    self.stat_line = true;
                                }
                            }
                            if self.ly_for_comparison == u16::from(self.lyc) {
                                self.stat |= STAT_LYC_B;
                            } else {
                                self.stat &= !STAT_LYC_B;
                            }
                        } else {
                            self.ly = 153;
                            self.ly_for_comparison = u16::MAX;
                            self.check_lyc(ints);
                        }
                    }
                }
            }
            Mode::Drawing => {
                if !self.fifo.line_done() {
                    self.step_fifo_dot(cgb_mode);
                }

                if self.fifo.line_done() || dot >= 450 {
                    while self.fifo.position() < 160 {
                        self.step_fifo_dot(cgb_mode);
                    }
                    self.hblank_start_dot = dot + 1;
                    self.enter_mode(Mode::HBlank, ints, cgb_mode);
                }
            }
        }

        // Mode transitions and scanline wrap:
        if self.mode() == Mode::OamScan && dot == 79 {
            self.enter_mode(Mode::Drawing, ints, cgb_mode);
        } else if self.lcdon_line0_mode0 && dot == (if !self.is_cgb { 75 } else { 79 }) {
            self.enter_mode(Mode::Drawing, ints, cgb_mode);
            self.lcdon_line0_mode0 = false;
        }

        if dot == line_end_dot {
            self.line_dot = 0;
            match self.mode() {
                Mode::HBlank => {
                    self.lcdon_line0 = false;
                    if self.line0_frame_wrap {
                        self.line0_frame_wrap = false;
                        self.current_vblank_line = 0;
                        self.ly = 0;
                        self.rgba_buf_present = mem::take(&mut self.rgb_buf);
                        self.enter_mode(Mode::OamScan, ints, cgb_mode);
                        self.ly_for_comparison = 0;
                        self.check_lyc(ints);
                    } else if (self.is_cgb && self.ly == 143) || self.ly >= 144 {
                        self.enter_mode(Mode::VBlank, ints, cgb_mode);
                    } else {
                        if self.is_cgb {
                            self.ly += 1;
                        }
                        self.enter_mode(Mode::OamScan, ints, cgb_mode);
                    }
                }
                Mode::VBlank => {
                    if self.current_vblank_line >= 153 {
                        self.current_vblank_line = 0;
                        self.ly = 0;
                        self.rgba_buf_present = mem::take(&mut self.rgb_buf);
                        self.enter_mode(Mode::OamScan, ints, cgb_mode);
                        self.ly_for_comparison = 0;
                        self.check_lyc(ints);
                    } else {
                        self.current_vblank_line += 1;
                        self.ly = if !self.is_cgb && self.current_vblank_line == 153 {
                            0
                        } else {
                            self.current_vblank_line
                        };
                        self.ly_for_comparison = if !self.is_cgb && self.current_vblank_line == 153
                        {
                            153
                        } else {
                            u16::from(self.ly)
                        };
                        if !(self.is_cgb && self.current_vblank_line == 153) {
                            self.check_lyc(ints);
                        }
                    }
                }
                _ => (),
            }
        } else {
            self.line_dot += 1;
        }
    }

    pub const fn set_color_correction_mode(&mut self, mode: ColorCorrectionMode) {
        self.color_correction_mode = mode;
    }

    pub fn write_lcdc(&mut self, val: u8, ints: &mut Interrupts, is_cgb: bool) {
        self.is_cgb = is_cgb;
        let was_on = self.lcdc & LCDC_ON_B != 0;
        let is_on = val & LCDC_ON_B != 0;
        self.lcdc = val;

        // turn off: reset to line 0 in HBlank mode, clear all blocking.
        if !is_on && was_on {
            self.ly = 0;
            self.current_vblank_line = 0;
            self.ly_for_comparison = 0;
            self.lcdon_line0_mode0 = false;
            self.lcdon_line0 = false;
            self.stat &= !STAT_MODE_B;
            self.mode = Mode::HBlank;
            self.stat_line = false;
            self.line_dot = 0;
            self.hblank_start_dot = 0;
            self.rgba_buf_present.clear();
            // LYC comparison: re-evaluate after LY reset to 0.
            self.check_lyc(ints);
        }

        // turn on: per the lcdon_mode_timing test, the first line starts
        // in mode 0 (HBlank) for 76 dots on DMG (Mode 1 on CGB), then goes straight to
        // mode 3 (skipping mode 2).
        if is_on && !was_on {
            self.ly = 0;
            self.current_vblank_line = 0;
            self.ly_for_comparison = 0;
            self.stat = (self.stat & !STAT_MODE_B) | Mode::HBlank as u8;
            self.mode = Mode::HBlank;
            self.line_dot = 0;
            self.hblank_start_dot = 0;
            self.lcdon_line0_mode0 = true;
            self.lcdon_line0 = true;
            self.check_lyc(ints);
        }
    }

    pub fn write_lyc(&mut self, val: u8, ints: &mut Interrupts) {
        self.lyc = val;
        if self.is_cgb {
            if self.mode() == Mode::HBlank && self.line_dot >= 444 {
                return;
            }
            if self.current_vblank_line == 152 && self.line_dot >= 452 {
                return;
            }
            if self.current_vblank_line == 153 && self.line_dot < 8 {
                return;
            }
        }
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
        self.scx = val;
        self.fifo.set_scx(val, self.is_cgb);
    }

    pub const fn set_stat(&mut self, val: u8) {
        self.stat = val;
    }

    /// Force the PPU line/mode state (`remaining_dots` remaining in `mode`). Test
    /// hook for post-boot state injection.
    pub const fn set_line_mode(&mut self, line: u8, mode: Mode, remaining_dots: i32) {
        self.ly = line;
        self.current_vblank_line = line;
        self.ly_for_comparison = line as u16;
        self.stat = (self.stat & !STAT_MODE_B) | mode as u8;
        self.mode = mode;
        self.line_dot = if remaining_dots <= 456 && remaining_dots >= 0 {
            (456 - remaining_dots) as u16
        } else {
            0
        };
        self.hblank_start_dot = 0;
        self.lcdon_line0_mode0 = false;
        self.lcdon_line0 = false;
        self.line0_frame_wrap = false;
    }

    pub const fn write_scy(&mut self, val: u8) {
        self.scy = val;
    }

    pub fn write_stat(&mut self, val: u8, ints: &mut Interrupts, is_cgb: bool) {
        let prev_stat = self.stat;
        let ly_equals_lyc = self.stat & STAT_LYC_B;
        let mode = self.stat & STAT_MODE_B;

        let was_line_high = self.stat_line;
        self.stat = (val & !0x07) | ly_equals_lyc | mode;

        if !is_cgb && self.lcdc & LCDC_ON_B != 0 && !was_line_high {
            let lyc_glitch = (prev_stat & STAT_IF_LYC_B == 0)
                && (val & STAT_IF_LYC_B != 0)
                && (self.stat & STAT_LYC_B != 0);

            if self.mode() == Mode::HBlank || self.mode() == Mode::VBlank || lyc_glitch {
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
