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

const DOTS_UNTIL_ENABLED: i32 = 80;

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

#[expect(
    clippy::arbitrary_source_item_ordering,
    reason = "Order follows the state machine transitions"
)]
#[derive(Clone, Copy, Debug, Default)]
pub enum Mode {
    #[default]
    HBlank = 0,
    VBlank = 1,
    OamScan = 2,
    Drawing = 3,
}

impl Mode {
    pub fn dots(self, scroll_x: u8) -> i32 {
        // Mode timings
        const OAM_SCAN_DOTS: i32 = 80; // Constant
        const DRAWING_DOTS: i32 = 172; // Variable, minimum ammount
        const HBLANK_DOTS: i32 = 204; // Variable, maximum ammount
        const VBLANK_DOTS: i32 = 456; // Constant

        let scroll_adjust = i32::from(scroll_x & 7) * 4;
        match self {
            Self::OamScan => OAM_SCAN_DOTS,
            Self::Drawing => DRAWING_DOTS + scroll_adjust,
            Self::HBlank => HBLANK_DOTS - scroll_adjust,
            Self::VBlank => VBLANK_DOTS,
        }
    }
}

#[expect(clippy::struct_excessive_bools)]
#[derive(Default)]
pub struct Ppu {
    bcp: ColorPalette,
    bgp: u8,
    color_correction: ColorCorrectionMode,
    delay_one_frame: bool,
    enable_timer: i32,
    lcdc: u8,
    ly: u8,
    lyc: u8,
    oam: Oam,
    obp0: u8,
    obp1: u8,
    ocp: ColorPalette,
    opri: bool,
    remaining_dots_in_mode: i32,
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

    const fn check_lyc(&mut self, ints: &mut Interrupts) {
        self.stat &= !STAT_LYC_B;

        if self.ly == self.lyc {
            self.stat |= STAT_LYC_B;
            if self.stat & STAT_IF_LYC_B != 0 {
                ints.request_lcd();
            }
        }
    }

    fn enter_mode(&mut self, mode: Mode, ints: &mut Interrupts) {
        self.set_mode_stat(mode);
        self.remaining_dots_in_mode += self.mode().dots(self.scx);

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
        match self.stat & 3 {
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

    pub fn run(&mut self, dots: i32, ints: &mut Interrupts, cgb_mode: CgbMode) {
        if self.lcdc & LCDC_ON_B == 0 {
            return;
        }

        let mut dots = dots;

        if self.enable_timer > 0 {
            // self.restart_timer = self.restart_timer - dots;
            if self.enable_timer - dots <= 0 {
                self.enable_timer = 0;
                dots -= self.enable_timer;
                let mode = Mode::Drawing;
                self.set_mode_stat(mode);
                self.remaining_dots_in_mode = mode.dots(self.scx);
            } else {
                self.enable_timer -= dots;
                return;
            }
        }

        self.remaining_dots_in_mode -= dots;

        if self.remaining_dots_in_mode < 0 {
            match self.mode() {
                Mode::OamScan => {
                    debug_assert!(self.ly <= 143, "OAM scan, ly = {}", self.ly);
                    self.enter_mode(Mode::Drawing, ints);
                }
                Mode::Drawing => {
                    debug_assert!(self.ly <= 143, "Drawing, ly = {}", self.ly);
                    self.draw_scanline(cgb_mode);
                    self.enter_mode(Mode::HBlank, ints);
                }
                Mode::HBlank => {
                    debug_assert!(self.ly <= 143, "HBlank, ly = {}", self.ly);
                    self.ly += 1;
                    if self.ly > 143 {
                        self.enter_mode(Mode::VBlank, ints);
                    } else {
                        self.enter_mode(Mode::OamScan, ints);
                    }
                    self.check_lyc(ints);
                }
                Mode::VBlank => {
                    debug_assert!(self.ly >= 144 && self.ly <= 153, "VBlank, ly = {}", self.ly);
                    self.ly += 1;
                    if self.ly > 153 {
                        self.ly = 0;
                        if self.delay_one_frame {
                            self.delay_one_frame = false;
                        } else {
                            self.rgba_buf_present = mem::take(&mut self.rgb_buf);
                        }
                        self.enter_mode(Mode::OamScan, ints);
                    } else {
                        self.remaining_dots_in_mode += self.mode().dots(self.scx);
                    }
                    self.check_lyc(ints);
                }
            }
        }
    }

    pub const fn set_color_correction(&mut self, mode: ColorCorrectionMode) {
        self.color_correction = mode;
    }

    const fn set_mode_stat(&mut self, mode: Mode) {
        self.stat = (self.stat & !STAT_MODE_B) | mode as u8;
    }

    pub const fn write_bgp(&mut self, val: u8) {
        self.bgp = val;
    }

    pub fn write_lcdc(&mut self, val: u8, ints: &mut Interrupts) {
        // turn off
        if val & LCDC_ON_B == 0 && self.lcdc & LCDC_ON_B != 0 {
            // FIXME: breaks 'alone in the dark' and the menu fade out in 'Links awakening' among others
            // debug_assert!(
            //     matches!(self.mode(), Mode::VBlank),
            //     "current mode = {:?}, dots = {}, ly = {}",
            //     self.mode(),
            //     self.remaining_dots_in_mode,
            //     self.ly
            // );

            self.ly = 0;
            let mode = Mode::HBlank;
            self.set_mode_stat(mode);
            self.remaining_dots_in_mode = mode.dots(self.scx);
            self.rgba_buf_present.clear();
        }

        // turn on
        if val & LCDC_ON_B != 0 && self.lcdc & LCDC_ON_B == 0 {
            self.ly = 0;
            let mode = Mode::HBlank;
            self.set_mode_stat(mode);
            self.remaining_dots_in_mode = mode.dots(self.scx);
            self.check_lyc(ints);
            self.enable_timer = DOTS_UNTIL_ENABLED;
            self.delay_one_frame = true;
        }

        self.lcdc = val;
    }

    pub const fn write_lyc(&mut self, val: u8) {
        self.lyc = val;
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

    pub const fn write_scy(&mut self, val: u8) {
        self.scy = val;
    }

    pub const fn write_stat(&mut self, val: u8) {
        let ly_equals_lyc = self.stat & STAT_LYC_B;
        let mode: u8 = self.mode() as u8;

        self.stat = val;
        self.stat &= !(STAT_LYC_B | STAT_MODE_B);
        self.stat |= ly_equals_lyc | mode;
    }

    pub const fn write_wx(&mut self, val: u8) {
        self.wx = val;
    }

    pub const fn write_wy(&mut self, val: u8) {
        self.wy = val;
    }
}
