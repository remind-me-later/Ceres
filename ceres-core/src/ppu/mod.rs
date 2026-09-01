mod color_palette;
mod draw;
mod fifo;
mod oam;
mod rgba_buf;
mod vram;

use core::mem;

use crate::interrupts::Interrupts;
pub use oam::Oam;
pub use vram::Vram;
use {self::color_palette::ColorPalette, crate::CgbMode, crate::Model, rgba_buf::RgbaBuf};

pub const PX_WIDTH: u8 = 160;
pub const PX_HEIGHT: u8 = 144;

/// T-cycles (dots) per M-cycle.
const DOTS_PER_M: i32 = 4;
/// Maximum Mode 3 length in dots: a full line is 456 dots and Mode 2 takes
/// 80, so a scanline can never spend more than this drawing. Used as a
/// watchdog only — the real transition fires when the FIFO completes the
/// scanline.
const MODE3_MAX_DOTS: i32 = 456 - 20 * DOTS_PER_M;

// LCDC bits
const LCDC_ON_B: u8 = 0x80;

// STAT bits
const STAT_MODE_B: u8 = 0x3;
const STAT_LYC_B: u8 = 0x4;
const STAT_IF_HBLANK_B: u8 = 0x8;
const STAT_IF_VBLANK_B: u8 = 0x10;
const STAT_IF_OAM_B: u8 = 0x20;
const STAT_IF_LYC_B: u8 = 0x40;

// TEMPORARY sweep knob: dots remaining in HBlank when the DMG mode-2
// (OAM) STAT IRQ fires. Swept against the mooneye intr_2 family; old
// value 4, best 7. (fold into the ladder before commit)
const OAM_IRQ_AT: i32 = 7;

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

impl Mode {
    /// M-cycles (1 M-cycle = 4 T-cycles) for each mode. Inspired by
    /// mooneye-gb but with the original Ceres constants for the per-mode
    /// lengths. `scroll_x` only affects Mode 3 / Mode 0 split, not VBlank.
    /// The scroll adjustment differs between DMG/MGB/SGB/SGB2 and
    /// CGB/AGB/AGS (see SameBoy's display.c).
    const fn m_cycles(self, scroll_x: u8, model: Model) -> i32 {
        const OAM_M_CYCLES: i32 = 20;
        const VBLANK_M_CYCLES: i32 = 114;
        match self {
            Self::OamScan => OAM_M_CYCLES,
            Self::Drawing => {
                let adjust = match scroll_x & 0x7 {
                    1..=4 => 1,
                    5..=7 => 2,
                    _ => 0,
                };
                let base = match model {
                    Model::Cgb0
                    | Model::CgbA
                    | Model::CgbB
                    | Model::CgbC
                    | Model::CgbD
                    | Model::CgbE
                    | Model::Agb => 42,
                    _ => 43,
                };
                base + adjust
            }
            Self::HBlank => {
                let adjust = match scroll_x & 0x7 {
                    1..=4 => 1,
                    5..=7 => 2,
                    _ => 0,
                };
                let base = match model {
                    Model::Cgb0
                    | Model::CgbA
                    | Model::CgbB
                    | Model::CgbC
                    | Model::CgbD
                    | Model::CgbE
                    | Model::Agb => 52,
                    _ => 51,
                };
                base - adjust
            }
            Self::VBlank => VBLANK_M_CYCLES,
        }
    }
}

#[expect(clippy::struct_excessive_bools)]
pub struct Ppu {
    pub(crate) model: Model,
    bcp: ColorPalette,
    bgp: u8,
    color_correction_mode: ColorCorrectionMode,
    /// Whether this PPU instance runs in CGB/AGB/AGS mode (as opposed to
    /// DMG/MGB/SGB/SGB2 or CGB-in-compat-mode). Used to pick the per-model
    /// scroll-adjustment table.
    is_cgb: bool,
    /// Dots (T-cycles) remaining until the PPU transitions to the next
    /// mode (or fires a mode-bound IRQ). In Mode 3 this is only a
    /// watchdog: the FIFO decides when the line ends.
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
    /// The PPU's true internal mode. Kept as a field (rather than derived
    /// from the STAT bits) so the CPU-visible mode bits can be decoupled
    /// from the internal state later — SameBoy's `display_state` vs
    /// `GB_IO_STAT`.
    mode: Mode,
    mode_for_interrupt: Option<Mode>,
    stat_line: bool,
    sprite_penalty: i32,
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
            cycles: Mode::HBlank.m_cycles(0, Model::default()) * DOTS_PER_M,
            lcdc: 0,
            ly: 0,
            ly_for_comparison: 0,
            lyc: 0,
            lyc_latched: 0,
            lcdon_line0_mode0: false,
            line0_frame_wrap: false,
            mode: Mode::HBlank,
            mode_for_interrupt: None,
            stat_line: false,
            sprite_penalty: 0,
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
            cycles: Mode::HBlank.m_cycles(0, model) * DOTS_PER_M,
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
                    let cgb_hblank_delayed = self.is_cgb
                        && self.cycles
                            >= (Mode::HBlank.m_cycles(self.scx, self.model) * DOTS_PER_M
                                - self.sprite_penalty);
                    !self.lcdon_line0_mode0
                        && !cgb_hblank_delayed
                        && (self.stat & STAT_IF_HBLANK_B != 0)
                }
                Mode::VBlank => self.stat & STAT_IF_VBLANK_B != 0,
                Mode::OamScan => !self.is_cgb && (self.stat & STAT_IF_OAM_B != 0),
                _ => false,
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

    /// Transition the PPU to a new mode, reset the per-mode cycle counter,
    /// and fire any mode-bound IRQs.
    fn enter_mode(&mut self, mode: Mode, ints: &mut Interrupts, cgb_mode: CgbMode) {
        self.is_cgb = matches!(cgb_mode, CgbMode::Cgb | CgbMode::Compat);
        if mode != Mode::HBlank {
            // Entering HBlank keeps the penalty measured when the FIFO
            // finished the previous scanline (it sizes this HBlank).
            self.sprite_penalty = 0;
        }

        let base_cycles = mode.m_cycles(self.scx, self.model) * DOTS_PER_M;
        self.cycles = match mode {
            // Mode 3's length is owned by the pixel FIFO; `cycles` only
            // arms the watchdog against a stalled FIFO.
            Mode::Drawing => MODE3_MAX_DOTS,
            Mode::HBlank => (base_cycles - self.sprite_penalty).max(DOTS_PER_M),
            _ => base_cycles,
        };
        self.mode_for_interrupt = None;
        self.mode = mode;
        // Update mode bits AFTER setting cycles so cgb_mode is queried here.
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
                self.fifo.start_drawing(self.scx, self.lcdon_line0_mode0);
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
            if self.current_vblank_line == 153 && self.cycles == 114 * DOTS_PER_M {
                153
            } else if (self.stat & STAT_MODE_B) == 1
                && self.current_vblank_line == 144
                && self.cycles == 2 * DOTS_PER_M
            {
                145
            } else if (self.stat & STAT_MODE_B) == 0 && self.cycles == DOTS_PER_M && self.ly < 143 {
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
    /// the line naturally. All other modes use `cycles` as a dot countdown
    /// (4 dots per M-cycle, so events previously calibrated in M-cycles
    /// keep firing at the same absolute dots).
    pub fn tick_t_cycle(&mut self, ints: &mut Interrupts, cgb_mode: CgbMode) {
        // Cache whether we're running in CGB native mode so per-model
        // timing decisions can be made without threading CgbMode
        // through every internal call.
        self.is_cgb = matches!(cgb_mode, CgbMode::Cgb | CgbMode::Compat);
        if self.lcdc & LCDC_ON_B == 0 {
            return;
        }

        // Mid-scanline comparator / glitch events:
        match self.mode() {
            Mode::OamScan => {
                if self.cycles == DOTS_PER_M * 19 {
                    self.ly_for_comparison = u16::from(self.ly);
                    self.check_lyc(ints);
                }
            }
            Mode::HBlank => {
                let base_hblank =
                    Mode::HBlank.m_cycles(self.scx, self.model) * DOTS_PER_M - self.sprite_penalty;
                if self.is_cgb && self.cycles == base_hblank - DOTS_PER_M {
                    self.update_stat_line(ints);
                }
                if (self.ly == 143 || (self.ly == 144 && self.mode() == Mode::HBlank))
                    && self.ly_for_comparison != u16::MAX
                {
                    if !self.is_cgb {
                        if self.cycles == 2 * DOTS_PER_M {
                            self.ly = 144;
                            self.ly_for_comparison = 143;
                            self.check_lyc(ints);
                            if self.stat & STAT_IF_OAM_B != 0 && !self.stat_line {
                                ints.request_lcd();
                                self.stat_line = true;
                            }
                        } else if self.cycles == DOTS_PER_M {
                            self.ly_for_comparison = 144;
                            self.check_lyc(ints);
                            ints.request_vblank();
                            if self.stat & STAT_IF_VBLANK_B != 0 && !self.stat_line {
                                ints.request_lcd();
                                self.stat_line = true;
                            }
                        }
                    } else if self.cycles == 2 * DOTS_PER_M {
                        self.ly = 144;
                        self.ly_for_comparison = 143;
                        self.check_lyc(ints);
                    } else if self.cycles == DOTS_PER_M {
                        self.ly_for_comparison = 144;
                        self.check_lyc(ints);
                        ints.request_vblank();
                        if self.stat & STAT_IF_VBLANK_B != 0 && !self.stat_line {
                            ints.request_lcd();
                            self.stat_line = true;
                        }
                    }
                } else if self.cycles == 2 * DOTS_PER_M && !self.lcdon_line0_mode0 {
                    if !self.is_cgb {
                        if self.ly < 143 {
                            self.ly_for_comparison = u16::MAX;
                            self.check_lyc(ints);
                        }
                    } else if self.ly < 143 {
                        self.lyc_latched = self.lyc;
                        self.ly_for_comparison = u16::from(self.ly);
                        if self.ly_for_comparison == u16::from(self.lyc) {
                            self.stat |= STAT_LYC_B;
                        } else {
                            self.stat &= !STAT_LYC_B;
                        }
                    }
                } else if self.cycles == DOTS_PER_M && !self.lcdon_line0_mode0 {
                    if self.ly <= 143 {
                        if self.is_cgb {
                            if self.lyc_latched == self.ly + 1 {
                                if (self.stat & STAT_IF_LYC_B) != 0 && !self.stat_line {
                                    ints.request_lcd();
                                    self.stat_line = true;
                                }
                            }
                            self.ly_for_comparison = u16::from(self.ly + 1);
                            if self.ly_for_comparison == u16::from(self.lyc) {
                                self.stat |= STAT_LYC_B;
                            } else {
                                self.stat &= !STAT_LYC_B;
                            }
                        } else {
                            if self.ly < 143 {
                                self.ly += 1;
                            }
                            self.ly_for_comparison = u16::from(self.ly);
                            self.check_lyc(ints);
                        }
                    }
                }

                // DMG mode-2 (OAM) STAT IRQ. Fired `OAM_IRQ_AT` dots before
                // HBlank ends (swept against the mooneye intr_2 family; 7
                // is the empirically best dot on our line phase). Gated on
                // ly != 0: hardware gives line 0 no mode-2 interrupt
                // (SameBoy display.c:1778-1787).
                if !self.is_cgb
                    && self.cycles == OAM_IRQ_AT
                    && !self.lcdon_line0_mode0
                    && self.ly != 0
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
                        if self.cycles == 114 * DOTS_PER_M {
                            self.ly = 0;
                            self.lyc_latched = self.lyc;
                            self.ly_for_comparison = 153;
                            if self.ly_for_comparison == u16::from(self.lyc) {
                                self.stat |= STAT_LYC_B;
                            } else {
                                self.stat &= !STAT_LYC_B;
                            }
                        } else if self.cycles == 113 * DOTS_PER_M {
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
                        }
                    } else if self.cycles == 113 * DOTS_PER_M {
                        self.ly = 0;
                        self.ly_for_comparison = u16::MAX;
                        self.check_lyc(ints);
                    } else if self.cycles == 112 * DOTS_PER_M {
                        self.ly = 0;
                        self.ly_for_comparison = 0;
                        self.check_lyc(ints);
                    }
                } else if self.current_vblank_line >= 144 {
                    let base_cycles = Mode::VBlank.m_cycles(self.scx, self.model) * DOTS_PER_M;
                    if self.cycles == base_cycles - DOTS_PER_M {
                        self.ly_for_comparison = u16::from(self.ly);
                        self.check_lyc(ints);
                    } else if self.cycles == 2 * DOTS_PER_M {
                        if self.current_vblank_line < 152 {
                            self.ly = self.current_vblank_line + 1;
                            if !self.is_cgb {
                                self.ly_for_comparison = u16::MAX;
                                self.check_lyc(ints);
                            }
                        } else {
                            self.ly = 153;
                            if self.is_cgb {
                                self.lyc_latched = self.lyc;
                                self.ly_for_comparison = 152;
                                if self.ly_for_comparison == u16::from(self.lyc) {
                                    self.stat |= STAT_LYC_B;
                                } else {
                                    self.stat &= !STAT_LYC_B;
                                }
                            } else {
                                self.ly_for_comparison = u16::MAX;
                                self.check_lyc(ints);
                            }
                        }
                    } else if self.cycles == DOTS_PER_M {
                        if self.current_vblank_line < 152 {
                            self.ly_for_comparison = u16::from(self.current_vblank_line + 1);
                            self.check_lyc(ints);
                        } else if self.is_cgb {
                            self.ly_for_comparison = 153;
                            if self.lyc_latched == 153 {
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
                            self.ly_for_comparison = 153;
                            self.check_lyc(ints);
                        }
                    }
                }
            }
            Mode::Drawing => {
                if !self.fifo.line_done() {
                    self.step_fifo_dot(cgb_mode);
                }

                if self.fifo.line_done() {
                    // The FIFO completed the scanline: Mode 3 ends at this
                    // dot. Record the measured length (dots since mode-3
                    // entry) as the sprite/window penalty relative to the
                    // SCX-adjusted base, then let the mode-expiry block
                    // below enter HBlank; HBlank absorbs the remainder so
                    // the line still totals 456 dots.
                    let mode3_dots = MODE3_MAX_DOTS - self.cycles + 1;
                    let base3 = Mode::Drawing.m_cycles(self.scx, self.model) * DOTS_PER_M;
                    self.sprite_penalty = mode3_dots - base3;
                    self.cycles = 1;
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
                // Watchdog: the FIFO should have completed the line by now
                // (it never legitimately needs all 376 dots). Finish it so
                // every scanline still reaches HBlank even if a PPU bug
                // stalls the FIFO.
                while !self.fifo.line_done() {
                    self.step_fifo_dot(cgb_mode);
                }
                self.enter_mode(Mode::HBlank, ints, cgb_mode);
            }
            Mode::HBlank => {
                if self.lcdon_line0_mode0 {
                    self.enter_mode(Mode::Drawing, ints, cgb_mode);
                    self.lcdon_line0_mode0 = false;
                } else if self.line0_frame_wrap {
                    self.line0_frame_wrap = false;
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
                if self.lcdon_line0_mode0 {
                    self.lcdon_line0_mode0 = false;
                    self.enter_mode(Mode::Drawing, ints, cgb_mode);
                } else if self.current_vblank_line >= 153 {
                    self.current_vblank_line = 0;
                    self.ly = 0;
                    self.rgba_buf_present = mem::take(&mut self.rgb_buf);
                    if self.is_cgb {
                        self.enter_mode(Mode::OamScan, ints, cgb_mode);
                        self.ly_for_comparison = 0;
                        self.check_lyc(ints);
                    } else {
                        self.stat = (self.stat & !STAT_MODE_B) | Mode::HBlank as u8;
                        self.mode = Mode::HBlank;
                        self.cycles = DOTS_PER_M;
                        self.line0_frame_wrap = true;
                        self.ly_for_comparison = 0;
                        self.check_lyc(ints);
                    }
                } else {
                    self.current_vblank_line += 1;
                    self.ly = if !self.is_cgb && self.current_vblank_line == 153 {
                        0
                    } else {
                        self.current_vblank_line
                    };
                    let base_cycles = Mode::VBlank.m_cycles(self.scx, self.model) * DOTS_PER_M;
                    self.cycles = if !self.is_cgb && self.current_vblank_line == 153 {
                        base_cycles - DOTS_PER_M
                    } else {
                        base_cycles
                    };
                    self.ly_for_comparison = if !self.is_cgb && self.current_vblank_line == 153 {
                        153
                    } else {
                        u16::from(self.ly)
                    };
                    self.check_lyc(ints);
                }
            }
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
            self.stat &= !STAT_MODE_B;
            self.mode = Mode::HBlank;
            self.stat_line = false;
            self.cycles = Mode::HBlank.m_cycles(self.scx, self.model) * DOTS_PER_M;
            self.rgba_buf_present.clear();
            // LYC comparison: re-evaluate after LY reset to 0.
            self.check_lyc(ints);
        }

        // turn on: per the lcdon_mode_timing test, the first line starts
        // in mode 0 (HBlank) for 20 M-cycles on DMG (Mode 1 on CGB), then goes straight to
        // mode 3 (skipping mode 2).
        if is_on && !was_on {
            self.ly = 0;
            self.current_vblank_line = 0;
            self.ly_for_comparison = 0;
            self.stat = (self.stat & !STAT_MODE_B) | Mode::HBlank as u8;
            self.mode = Mode::HBlank;
            self.cycles = 20 * DOTS_PER_M;
            self.lcdon_line0_mode0 = true;
            self.check_lyc(ints);
        }
    }

    pub fn write_lyc(&mut self, val: u8, ints: &mut Interrupts) {
        self.lyc = val;
        if self.is_cgb {
            if self.mode() == Mode::HBlank
                && (self.cycles == 2 * DOTS_PER_M || self.cycles == DOTS_PER_M)
            {
                return;
            }
            if self.current_vblank_line == 152
                && (self.cycles == 2 * DOTS_PER_M || self.cycles == DOTS_PER_M)
            {
                return;
            }
            if self.current_vblank_line == 153
                && (self.cycles == 114 * DOTS_PER_M || self.cycles == 113 * DOTS_PER_M)
            {
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
        // Mode 3's length is FIFO-owned: an SCX write takes effect through
        // the FIFO's fetch/discard state, never by re-timing the mode.
        self.scx = val;
        self.fifo.set_scx(val);
    }

    pub(crate) const fn set_stat(&mut self, val: u8) {
        self.stat = val;
    }

    /// Force the PPU line/mode state (dots remaining in `mode`). Test
    /// hook for post-boot state injection.
    pub(crate) const fn set_line_mode(&mut self, line: u8, mode: Mode, cycles: i32) {
        self.ly = line;
        self.current_vblank_line = line;
        self.ly_for_comparison = line as u16;
        self.stat = (self.stat & !STAT_MODE_B) | mode as u8;
        self.mode = mode;
        self.cycles = cycles;
        self.lcdon_line0_mode0 = false;
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
