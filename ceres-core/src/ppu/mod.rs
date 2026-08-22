pub mod fifo;
mod color_palette;
mod draw;
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
        let vram_m_cycles: i32 = 43;
        const VBLANK_M_CYCLES: i32 = 114;
        match self {
            Self::OamScan => OAM_M_CYCLES,
            Self::Drawing => {
                let adjust = match model {
                    Model::Cgb0
                    | Model::CgbA
                    | Model::CgbB
                    | Model::CgbC
                    | Model::CgbD
                    | Model::CgbE
                    | Model::Agb => match scroll_x & 0x7 {
                        3..=6 => 1,
                        7 => 2,
                        _ => 0,
                    },
                    _ => match scroll_x & 0x7 {
                        4..=7 => 1,
                        _ => 0,
                    },
                };
                vram_m_cycles + adjust
            }
            Self::HBlank => {
                let adjust = match model {
                    Model::Cgb0
                    | Model::CgbA
                    | Model::CgbB
                    | Model::CgbC
                    | Model::CgbD
                    | Model::CgbE
                    | Model::Agb => match scroll_x & 0x7 {
                        3..=6 => 1,
                        7 => 2,
                        _ => 0,
                    },
                    _ => match scroll_x & 0x7 {
                        1..=4 => 1,
                        5..=7 => 2,
                        _ => 0,
                    },
                };
                51 - adjust
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
            cycles: Mode::HBlank.m_cycles(0, Model::default()),
            lcdc: 0,
            ly: 0,
            ly_for_comparison: 0,
            lyc: 0,
            lyc_latched: 0,
            lcdon_line0_mode0: false,
            line0_frame_wrap: false,
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
        Self {
            model,
            cycles: Mode::HBlank.m_cycles(0, model),
            ..Self::default()
        }
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
                Mode::HBlank => !self.lcdon_line0_mode0 && (self.stat & STAT_IF_HBLANK_B != 0),
                Mode::VBlank => self.stat & STAT_IF_VBLANK_B != 0,
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

    fn sprite_penalty_m_cycles(&self, cgb_mode: CgbMode) -> i32 {
        if self.lcdc & LCDC_OBJ_B == 0 && cgb_mode == CgbMode::Dmg {
            return 0;
        }

        let height: u8 = if self.lcdc & LCDC_OBJL_B == 0 { 8 } else { 16 };
        let bytes = self.oam.bytes();
        let mut visible_sprites: [u8; 10] = [0; 10];
        let mut count = 0;

        for i in 0..40 {
            let offset = i * 4;
            let y = bytes[offset];
            let x = bytes[offset + 1];

            let ly_plus_16 = u16::from(self.ly) + 16;
            let y_u16 = u16::from(y);

            if ly_plus_16 >= y_u16 && ly_plus_16 < y_u16 + u16::from(height) {
                visible_sprites[count] = x;
                count += 1;
                if count == 10 {
                    break;
                }
            }
        }

        if count == 0 {
            return 0;
        }

        if matches!(cgb_mode, CgbMode::Dmg) || self.opri {
            visible_sprites[..count].sort_unstable();
        }

        let scx_fine = (self.scx & 7) as i32;
        let mut total_t_cycles = 0;
        let mut last_tile_x = -1;
        let mut unique_tiles = 0;
        let mut prev_t = -1;
        for &x in &visible_sprites[..count] {
            if x < 168 {
                let t = (x / 8) as i32;
                if t != prev_t {
                    unique_tiles += 1;
                    prev_t = t;
                }
            }
        }

        let mut num_tiles = 0;
        let mut boundary_seen = false;
        let mut tile_initial_offset = -1;
        let mut tile_repeated_boundary_applied = false;

        for &x in &visible_sprites[..count] {
            if x >= 168 {
                continue;
            }

            total_t_cycles += 6;

            let tile_x = (x / 8) as i32;
            let offset = (x & 7) as i32;

            if tile_x != last_tile_x {
                let prev_boundary = boundary_seen;
                boundary_seen = false;
                tile_initial_offset = offset;
                if scx_fine == 0 {
                    match offset {
                        0 => total_t_cycles += 4,
                        1 => total_t_cycles += 3,
                        2 => total_t_cycles += 2,
                        3 => total_t_cycles += 1,
                        _ => {}
                    }
                } else if x < 8 {
                    if x == 0 {
                        total_t_cycles += if num_tiles == 0 { 6 } else { 3 };
                    } else if offset >= 8 - scx_fine {
                        total_t_cycles += if num_tiles == 0 {
                            if unique_tiles >= 10 && scx_fine >= 4 && offset >= 6 { 5 } else if offset == 8 - scx_fine || offset < 7 { 6 } else { 5 }
                        } else if scx_fine > 1 && offset == 8 - scx_fine {
                            6
                        } else {
                            5
                        };
                        boundary_seen = true;
                    } else if offset == 1 || (scx_fine >= 4 && offset <= 3) {
                        total_t_cycles += 4;
                    } else if offset == 2 {
                        total_t_cycles += if num_tiles == 0 { 3 } else { 2 };
                    } else if offset == 3 {
                        total_t_cycles += 2;
                    } else {
                        total_t_cycles += if scx_fine == 1 { 0 } else { 2 };
                    }
                } else {
                    match offset {
                        0 => {
                            if !prev_boundary {
                                total_t_cycles += if num_tiles == 0 {
                                    if unique_tiles >= 10 && scx_fine > 1 { 5 } else { 4 }
                                } else if unique_tiles >= 10 {
                                    if scx_fine <= 1 { 4 } else { (5 - scx_fine).max(0) }
                                } else {
                                    if scx_fine <= 1 { 4 } else { (6 - scx_fine).max(1) }
                                };
                            }
                        }
                        1 => total_t_cycles += if num_tiles == 0 { 4 } else if unique_tiles >= 10 { (4 - scx_fine).max(0) } else if scx_fine <= 1 || scx_fine >= 4 { 3 } else { (4 - scx_fine).max(0) },
                        2 => total_t_cycles += if num_tiles == 0 { 3 } else if unique_tiles >= 10 { (3 - scx_fine).max(0) } else if scx_fine >= 4 { 3 } else if scx_fine <= 1 { 2 } else { (3 - scx_fine).max(0) },
                        3 => total_t_cycles += if num_tiles == 0 { if scx_fine >= 4 && count >= 10 { 4 } else { 2 } } else if unique_tiles >= 10 { (2 - scx_fine).max(0) } else if scx_fine >= 4 { 2 } else if scx_fine <= 1 { 1 } else { (2 - scx_fine).max(0) },
                        _ => {
                            if offset >= 8 - scx_fine {
                                total_t_cycles += if num_tiles == 0 {
                                    if offset == 8 - scx_fine || offset < 7 { 6 } else { 5 }
                                } else if unique_tiles >= 10 {
                                    let base_delay = match offset - (8 - scx_fine) {
                                        0 => 5,
                                        1 => 4,
                                        2 => 3,
                                        _ => 2,
                                    };
                                    let tile_boost = if num_tiles <= 3 && (scx_fine >= 4 || (scx_fine > 1 && offset == 8 - scx_fine)) { 1 } else { 0 };
                                    base_delay + tile_boost
                                } else if tile_initial_offset == 0 && scx_fine >= 4 {
                                    0
                                } else if scx_fine > 1 && (offset == 8 - scx_fine || (scx_fine >= 4 && offset <= 5)) {
                                    6
                                } else {
                                    5
                                };
                                boundary_seen = true;
                            } else if num_tiles == 0 {
                                total_t_cycles += if scx_fine == 1 { 0 } else { 2 };
                            }
                        }
                    }
                }
                if scx_fine == 0 && num_tiles > 0 && offset < 5 {
                    total_t_cycles += 1;
                }
                num_tiles += 1;
                last_tile_x = tile_x;
            } else if scx_fine > 0 {
                if !boundary_seen && offset >= 8 - scx_fine {
                    total_t_cycles += match tile_initial_offset {
                        0 if num_tiles > 1 && scx_fine >= 4 => 2,
                        1 if scx_fine >= 4 => 4,
                        2 if scx_fine >= 4 => 5,
                        3..=4 if scx_fine == 2 => 4,
                        _ => if offset == 8 - scx_fine {
                            if scx_fine > 1 { 6 } else { 5 }
                        } else {
                            5
                        },
                    };
                    boundary_seen = true;
                } else if boundary_seen && (offset == 8 - scx_fine || (scx_fine >= 4 && (offset <= 5 || offset == tile_initial_offset + 1))) {
                    if tile_initial_offset == 0 && num_tiles > 1 && scx_fine >= 4 {
                        total_t_cycles += 2;
                    } else if scx_fine >= 3 && count >= 10 && !tile_repeated_boundary_applied && tile_initial_offset >= 8 - scx_fine - (if scx_fine >= 4 { 1 } else { 0 }) {
                        total_t_cycles += 2;
                        tile_repeated_boundary_applied = true;
                    }
                }
            }
        }

        let base_scroll_adjust = if !self.is_cgb && (self.scx & 7) >= 4 { 1 } else { 0 };
        let all_in_same_tile = count > 0 && visible_sprites[..count].iter().all(|&x| (x / 8) == (visible_sprites[0] / 8));
        let all_in_tile_0 = visible_sprites[..count].iter().all(|&x| x < 8);
        let all_at_zero = count > 0 && visible_sprites[0] == 0 && (all_in_tile_0 || visible_sprites[count - 1] == 0);
        let scx_adjust = if !self.is_cgb && all_at_zero && !boundary_seen && count % 2 == 0 {
            match scx_fine {
                3 => 2,
                4 => 3,
                _ => 0,
            }
        } else {
            0
        };
        let use_plus_one = scx_fine == 0 || count <= 1 || (scx_fine >= 4 && (visible_sprites[0] == 0 || all_in_same_tile) && !boundary_seen);
        if use_plus_one {
            (((total_t_cycles + scx_adjust + 1) / 4) - base_scroll_adjust).max(0)
        } else {
            (((total_t_cycles + scx_adjust) / 4) - base_scroll_adjust).max(0)
        }
    }

    /// Transition the PPU to a new mode, reset the per-mode cycle counter,
    /// and fire any mode-bound IRQs.
    fn enter_mode(&mut self, mode: Mode, ints: &mut Interrupts, cgb_mode: CgbMode) {
        self.is_cgb = matches!(cgb_mode, CgbMode::Cgb | CgbMode::Compat);
        if mode == Mode::Drawing {
            self.sprite_penalty = self.sprite_penalty_m_cycles(cgb_mode);
        } else if mode != Mode::HBlank {
            self.sprite_penalty = 0;
        }

        let base_cycles = mode.m_cycles(self.scx, self.model);
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
                self.fifo.start_scanline(
                    &self.oam,
                    self.ly,
                    self.scx,
                    self.scy,
                    self.lcdc,
                    self.is_cgb,
                    self.opri,
                );
            }
            Mode::VBlank => {
                self.current_vblank_line = 144;
                self.ly = 144;
                self.ly_for_comparison = 144;
                ints.request_vblank();
                if !self.is_cgb && (self.stat & STAT_IF_OAM_B != 0) && !self.stat_line {
                    ints.request_lcd();
                    self.stat_line = true;
                }
                self.win_skipped = 0;
                self.win_in_frame = false;
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
        if self.is_cgb && self.current_vblank_line == 153 && self.cycles == 114 {
            153
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

    /// Advance the PPU by one M-cycle (4 T-cycles). This matches
    /// mooneye-gb's `emulate()`: each call consumes one M-cycle of
    /// the current mode, fires any pending IRQs, and switches modes
    /// when the per-mode cycle budget runs out.
    pub fn tick_m_cycle(&mut self, ints: &mut Interrupts, cgb_mode: CgbMode) {
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
                if self.cycles == 19 {
                    self.ly_for_comparison = u16::from(self.ly);
                    self.check_lyc(ints);
                }
            }
            Mode::HBlank => {
                if (self.ly == 143 || (self.ly == 144 && self.mode() == Mode::HBlank)) && self.ly_for_comparison != u16::MAX {
                    if !self.is_cgb {
                        if self.cycles == 2 {
                            self.ly = 144;
                            self.ly_for_comparison = 143;
                            self.check_lyc(ints);
                            if self.stat & STAT_IF_OAM_B != 0 && !self.stat_line {
                                ints.request_lcd();
                                self.stat_line = true;
                            }
                        } else if self.cycles == 1 {
                            self.ly_for_comparison = 144;
                            self.check_lyc(ints);
                        }
                    } else if self.cycles == 2 {
                        self.ly = 144;
                        self.ly_for_comparison = 143;
                        self.check_lyc(ints);
                    } else if self.cycles == 1 {
                        if !self.is_cgb {
                            self.ly_for_comparison = 144;
                            self.check_lyc(ints);
                        }
                        if self.stat & STAT_IF_OAM_B != 0 && !self.stat_line {
                            ints.request_lcd();
                            self.stat_line = true;
                        }
                    }
                } else if self.cycles == 2 && !self.lcdon_line0_mode0 {
                    if !self.is_cgb {
                        if self.ly < 143 {
                            self.ly += 1;
                            self.ly_for_comparison = u16::MAX;
                            self.check_lyc(ints);
                        }
                        if self.ly == 1 || self.ly == 143 {
                            if self.stat & STAT_IF_OAM_B != 0 && !self.stat_line {
                                ints.request_lcd();
                                self.stat_line = true;
                            }
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
                } else if self.cycles == 1 && !self.lcdon_line0_mode0 {
                    if self.is_cgb {
                        if self.ly < 143 {
                            self.ly += 1;
                            self.ly_for_comparison = u16::from(self.ly);
                            if self.lyc_latched == self.ly {
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
                            if self.stat & STAT_IF_OAM_B != 0 && !self.stat_line {
                                ints.request_lcd();
                                self.stat_line = true;
                            }
                        }
                    } else if self.ly <= 143 {
                        self.ly_for_comparison = u16::from(self.ly);
                        self.check_lyc(ints);
                        if self.stat & STAT_IF_OAM_B != 0 && !self.stat_line {
                            ints.request_lcd();
                            self.stat_line = true;
                        }
                    }
                }
            }
            Mode::VBlank => {
                if self.current_vblank_line == 153 {
                    // Line 153 timing phases (SameBoy display.c:2217):
                    if self.is_cgb {
                        if self.cycles == 114 {
                            self.ly = 0;
                            self.lyc_latched = self.lyc;
                            self.ly_for_comparison = 153;
                            if self.ly_for_comparison == u16::from(self.lyc) {
                                self.stat |= STAT_LYC_B;
                            } else {
                                self.stat &= !STAT_LYC_B;
                            }
                        } else if self.cycles == 113 {
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
                    } else if self.cycles == 113 {
                        self.ly = 0;
                        self.ly_for_comparison = u16::MAX;
                        self.check_lyc(ints);
                    } else if self.cycles == 112 {
                        self.ly = 0;
                        self.ly_for_comparison = 0;
                        self.check_lyc(ints);
                    }
                } else if self.current_vblank_line >= 144 {
                    let base_cycles = Mode::VBlank.m_cycles(self.scx, self.model);
                    if self.cycles == base_cycles - 1 {
                        self.ly_for_comparison = u16::from(self.ly);
                        self.check_lyc(ints);
                    } else if self.cycles == 3 {
                        if !self.is_cgb && self.current_vblank_line < 152 {
                            self.ly = self.current_vblank_line + 1;
                        }
                    } else if self.cycles == 2 {
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
                    } else if self.cycles == 1 {
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
                for _ in 0..4 {
                    if let Some((lx, bg_px, sprite_px)) = self.fifo.step_dot(
                        &self.vram,
                        self.ly,
                        self.wx,
                        self.wy,
                        self.scx,
                        self.lcdc,
                        self.is_cgb,
                    ) {
                        let rgb = self.resolve_fifo_pixel(bg_px, sprite_px, cgb_mode);
                        if self.ly < 144 {
                            let idx = u32::from(self.ly) * 160 + u32::from(lx);
                            self.rgb_buf.set_px(idx, rgb);
                        }
                    }
                }

                if !self.is_cgb && self.cycles == 1 && (self.scx & 7) == 0 {
                    // Mode 0 HBlank STAT IRQ fires 1 M-cycle BEFORE Mode 0 begins on DMG when SCX % 8 == 0
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
                if self.fifo.lx < 160 {
                    self.draw_scanline(cgb_mode);
                }
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
                } else if self.ly >= 144 {
                    self.enter_mode(Mode::VBlank, ints, cgb_mode);
                } else {
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
                        self.cycles = 1;
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
                    let base_cycles = Mode::VBlank.m_cycles(self.scx, self.model);
                    self.cycles = if !self.is_cgb && self.current_vblank_line == 153 {
                        base_cycles - 1
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
            self.stat_line = false;
            self.cycles = Mode::HBlank.m_cycles(self.scx, self.model);
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
            self.cycles = 20;
            self.lcdon_line0_mode0 = true;
            self.check_lyc(ints);
        }
    }

    pub fn write_lyc(&mut self, val: u8, ints: &mut Interrupts) {
        self.lyc = val;
        if self.is_cgb {
            if self.mode() == Mode::HBlank && (self.cycles == 2 || self.cycles == 1) {
                return;
            }
            if self.current_vblank_line == 152 && (self.cycles == 2 || self.cycles == 1) {
                return;
            }
            if self.current_vblank_line == 153 && (self.cycles == 114 || self.cycles == 113) {
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
        if self.mode() == Mode::Drawing {
            let old_mode3 = Mode::Drawing.m_cycles(self.scx, self.model);
            let new_mode3 = Mode::Drawing.m_cycles(val, self.model);
            self.cycles += new_mode3 - old_mode3;
        }
        self.scx = val;
    }

    pub(crate) const fn set_stat(&mut self, val: u8) {
        self.stat = val;
    }

    pub(crate) const fn set_line_mode(&mut self, line: u8, mode: Mode, cycles: i32) {
        self.ly = line;
        self.current_vblank_line = line;
        self.ly_for_comparison = line as u16;
        self.stat = (self.stat & !STAT_MODE_B) | mode as u8;
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

            if self.mode() == Mode::HBlank
                || self.mode() == Mode::VBlank
                || lyc_glitch
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
