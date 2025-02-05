use crate::interrupts::Interrupts;

use {
    self::color_palette::ColorPalette, self::vram_renderer::VramRenderer, crate::CgbMode,
    rgba_buf::RgbaBuf,
};

pub use vram_renderer::VRAM_PX_HEIGHT;
pub use vram_renderer::VRAM_PX_WIDTH;

mod color_palette;
mod draw;
mod rgba_buf;
mod vram_renderer;

pub const PX_WIDTH: u8 = 160;
pub const PX_HEIGHT: u8 = 144;

// Mode timings
const OAM_SCAN_CYCLES: i32 = 80; // Constant
const DRAWING_CYCLES: i32 = 172; // Variable, minimum ammount
const HBLANK_CYCLES: i32 = 204; // Variable, maximum ammount
const VBLANK_CYCLES: i32 = 456; // Constant

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

// Sizes
pub const OAM_SIZE: u16 = 0xA0;
pub const VRAM_SIZE_GB: u16 = 0x2000;
pub const VRAM_SIZE_CGB: u16 = VRAM_SIZE_GB * 2;

#[derive(Clone, Copy, Debug, Default)]
pub enum Mode {
    #[default]
    HBlank = 0,
    VBlank = 1,
    OamScan = 2,
    Drawing = 3,
}

impl Mode {
    pub(crate) fn cycles(self, scroll_x: u8) -> i32 {
        let scroll_adjust = i32::from(scroll_x & 7) * 4;
        match self {
            Self::OamScan => OAM_SCAN_CYCLES,
            Self::Drawing => DRAWING_CYCLES + scroll_adjust,
            Self::HBlank => HBLANK_CYCLES - scroll_adjust,
            Self::VBlank => VBLANK_CYCLES,
        }
    }
}

#[derive(Debug)]
pub struct Ppu {
    lcdc: u8,
    stat: u8,
    scy: u8,
    scx: u8,
    ly: u8,
    lyc: u8,
    bgp: u8,
    obp0: u8,
    obp1: u8,
    wy: u8,
    wx: u8,
    opri: bool,
    vbk: bool,
    bcp: ColorPalette,
    ocp: ColorPalette,

    vram: [u8; VRAM_SIZE_CGB as usize],
    oam: [u8; OAM_SIZE as usize],
    rgb_buf: RgbaBuf,
    rgba_buf_present: RgbaBuf,
    cycles: i32,
    win_in_frame: bool,
    win_in_ly: bool,
    win_skipped: u8,

    // Debug utils
    vram_renderer: VramRenderer,
}

impl Default for Ppu {
    #[expect(clippy::large_stack_frames)]
    fn default() -> Self {
        Self {
            vram: [0; VRAM_SIZE_CGB as usize],
            oam: [0; OAM_SIZE as usize],
            cycles: Mode::default().cycles(0),
            // Default
            lcdc: Default::default(),
            stat: Mode::default() as u8,
            scy: Default::default(),
            scx: Default::default(),
            ly: Default::default(),
            lyc: Default::default(),
            bgp: Default::default(),
            obp0: Default::default(),
            obp1: Default::default(),
            wy: Default::default(),
            wx: Default::default(),
            opri: Default::default(),
            vbk: Default::default(),
            bcp: ColorPalette::default(),
            ocp: ColorPalette::default(),
            rgb_buf: RgbaBuf::default(),
            rgba_buf_present: RgbaBuf::default(),
            win_in_frame: Default::default(),
            win_in_ly: Default::default(),
            win_skipped: Default::default(),
            vram_renderer: Default::default(),
        }
    }
}

// IO
impl Ppu {
    #[must_use]
    #[inline]
    pub(crate) fn ocp_mut(&mut self) -> &mut ColorPalette {
        &mut self.ocp
    }

    #[must_use]
    #[inline]
    pub(crate) fn bcp_mut(&mut self) -> &mut ColorPalette {
        &mut self.bcp
    }

    #[must_use]
    #[inline]
    pub(crate) const fn ocp(&self) -> &ColorPalette {
        &self.ocp
    }

    #[must_use]
    #[inline]
    pub(crate) const fn bcp(&self) -> &ColorPalette {
        &self.bcp
    }

    #[must_use]
    #[inline]
    pub(crate) const fn read_stat(&self) -> u8 {
        self.stat | 0x80
    }

    #[must_use]
    #[inline]
    pub(crate) const fn read_ly(&self) -> u8 {
        self.ly
    }

    #[inline]
    pub(crate) fn write_opri(&mut self, val: u8) {
        self.opri = val & 1 != 0;
    }

    #[must_use]
    #[inline]
    pub(crate) const fn read_opri(&self) -> u8 {
        self.opri as u8 | 0xFE
    }

    #[inline]
    pub(crate) fn write_vbk(&mut self, val: u8) {
        self.vbk = val & 1 != 0;
    }

    #[must_use]
    #[inline]
    pub(crate) const fn read_vbk(&self) -> u8 {
        (self.vbk as u8) | 0xFE
    }

    #[inline]
    pub(crate) fn write_scx(&mut self, val: u8) {
        self.scx = val;
    }

    #[must_use]
    #[inline]
    pub(crate) const fn read_scx(&self) -> u8 {
        self.scx
    }

    #[inline]
    pub(crate) fn write_scy(&mut self, val: u8) {
        self.scy = val;
    }

    #[must_use]
    #[inline]
    pub(crate) const fn read_scy(&self) -> u8 {
        self.scy
    }

    #[inline]
    pub(crate) fn write_lyc(&mut self, val: u8) {
        self.lyc = val;
    }

    #[must_use]
    #[inline]
    pub(crate) const fn read_lyc(&self) -> u8 {
        self.lyc
    }

    #[inline]
    pub(crate) fn write_bgp(&mut self, val: u8) {
        self.bgp = val;
    }

    #[must_use]
    #[inline]
    pub(crate) const fn read_bgp(&self) -> u8 {
        self.bgp
    }

    #[inline]
    pub(crate) fn write_obp0(&mut self, val: u8) {
        self.obp0 = val;
    }

    #[must_use]
    #[inline]
    pub(crate) const fn read_obp0(&self) -> u8 {
        self.obp0
    }

    #[inline]
    pub(crate) fn write_obp1(&mut self, val: u8) {
        self.obp1 = val;
    }

    #[must_use]
    #[inline]
    pub(crate) const fn read_obp1(&self) -> u8 {
        self.obp1
    }

    #[inline]
    pub(crate) fn write_wy(&mut self, val: u8) {
        self.wy = val;
    }

    #[must_use]
    #[inline]
    pub(crate) const fn read_wy(&self) -> u8 {
        self.wy
    }

    #[inline]
    pub(crate) fn write_wx(&mut self, val: u8) {
        self.wx = val;
    }

    #[must_use]
    #[inline]
    pub(crate) const fn read_wx(&self) -> u8 {
        self.wx
    }

    #[must_use]
    #[inline]
    pub(crate) const fn read_lcdc(&self) -> u8 {
        self.lcdc
    }

    #[inline]
    pub(crate) fn write_lcdc(&mut self, val: u8, ints: &mut Interrupts) {
        // turn off
        if val & LCDC_ON_B == 0 && self.lcdc & LCDC_ON_B != 0 {
            // FIXME: breaks 'alone in the dark' and the menu fade out in 'Links awakening' among others
            // debug_assert!(
            //     matches!(self.mode(), Mode::VBlank),
            //     "current mode = {:?}, cycles = {}, ly = {}",
            //     self.mode(),
            //     self.cycles,
            //     self.ly
            // );

            self.ly = 0;
        }

        // turn on
        if val & LCDC_ON_B != 0 && self.lcdc & LCDC_ON_B == 0 {
            let mode = Mode::HBlank;

            self.set_mode_stat(mode);
            self.cycles = mode.cycles(self.scx);
            self.ly = 0;
            self.check_lyc(ints);
        }

        self.lcdc = val;
    }

    #[inline]
    pub(crate) fn write_stat(&mut self, val: u8) {
        let ly_equals_lyc = self.stat & STAT_LYC_B;
        let mode: u8 = self.mode() as u8;

        self.stat = val;
        self.stat &= !(STAT_LYC_B | STAT_MODE_B);
        self.stat |= ly_equals_lyc | mode;
    }

    #[must_use]
    #[inline]
    pub(crate) const fn read_vram(&self, addr: u16) -> u8 {
        if matches!(self.mode(), Mode::Drawing) {
            0xFF
        } else {
            let bank = self.vbk as u16 * VRAM_SIZE_GB;
            let i = (addr & 0x1FFF) + bank;
            self.vram[i as usize]
        }
    }

    pub(crate) fn write_vram(&mut self, addr: u16, val: u8) {
        if !matches!(self.mode(), Mode::Drawing) {
            let bank = u16::from(self.vbk) * VRAM_SIZE_GB;
            let i = (addr & 0x1FFF) + bank;
            self.vram[i as usize] = val;
        }
    }

    #[must_use]
    #[inline]
    pub(crate) const fn read_oam(&self, addr: u16, dma_on: bool) -> u8 {
        match self.mode() {
            Mode::HBlank | Mode::VBlank if !dma_on => self.oam[(addr & 0xFF) as usize],
            _ => 0xFF,
        }
    }

    #[inline]
    pub(crate) fn write_oam(&mut self, addr: u16, val: u8, dma_active: bool) {
        match self.mode() {
            Mode::HBlank | Mode::VBlank if !dma_active => {
                self.oam[(addr & 0xFF) as usize] = val;
            }
            _ => (),
        };
    }

    #[inline]
    pub(crate) fn write_oam_by_dma(&mut self, addr: u16, val: u8) {
        self.oam[(addr & 0xFF) as usize] = val;
    }
}

// General
impl Ppu {
    pub(crate) fn run(&mut self, cycles: i32, ints: &mut Interrupts, cgb_mode: &CgbMode) {
        if self.lcdc & LCDC_ON_B == 0 {
            return;
        }

        self.cycles -= cycles;

        if self.cycles < 0 {
            match self.mode() {
                Mode::OamScan => {
                    debug_assert!(self.ly <= 143);
                    self.enter_mode(Mode::Drawing, ints);
                }
                Mode::Drawing => {
                    debug_assert!(self.ly <= 143);
                    self.draw_scanline(cgb_mode);
                    self.enter_mode(Mode::HBlank, ints);
                }
                Mode::HBlank => {
                    debug_assert!(self.ly <= 143);
                    self.ly += 1;
                    if self.ly > 143 {
                        self.enter_mode(Mode::VBlank, ints);
                    } else {
                        self.enter_mode(Mode::OamScan, ints);
                    }
                    self.check_lyc(ints);
                }
                Mode::VBlank => {
                    debug_assert!(self.ly >= 144 && self.ly <= 153);
                    self.ly += 1;
                    if self.ly > 153 {
                        self.ly = 0;
                        self.rgba_buf_present = self.rgb_buf.clone();
                        self.enter_mode(Mode::OamScan, ints);
                    } else {
                        self.cycles += self.mode().cycles(self.scx);
                    }
                    self.check_lyc(ints);
                }
            }
        }
    }

    fn check_lyc(&mut self, ints: &mut Interrupts) {
        self.stat &= !STAT_LYC_B;

        if self.ly == self.lyc {
            self.stat |= STAT_LYC_B;
            if self.stat & STAT_IF_LYC_B != 0 {
                ints.req_lcd();
            }
        }
    }

    #[must_use]
    #[inline]
    pub(crate) const fn mode(&self) -> Mode {
        match self.stat & 3 {
            0 => Mode::HBlank,
            1 => Mode::VBlank,
            2 => Mode::OamScan,
            _ => Mode::Drawing,
        }
    }

    #[inline]
    fn set_mode_stat(&mut self, mode: Mode) {
        self.stat = (self.stat & !STAT_MODE_B) | mode as u8;
    }

    fn enter_mode(&mut self, mode: Mode, ints: &mut Interrupts) {
        self.set_mode_stat(mode);
        self.cycles += self.mode().cycles(self.scx);

        match mode {
            Mode::OamScan => {
                if self.stat & STAT_IF_OAM_B != 0 {
                    ints.req_lcd();
                }

                self.win_in_ly = false;
            }
            Mode::VBlank => {
                ints.req_vblank();

                if self.stat & STAT_IF_VBLANK_B != 0 {
                    ints.req_lcd();
                }

                // TODO: why?
                // if self.stat & STAT_IF_OAM_B != 0 {
                //     ints.req_lcd();
                // }

                self.win_skipped = 0;
                self.win_in_frame = false;

                self.vram_renderer.draw_vram(&self.vram);
            }
            Mode::Drawing => (),
            Mode::HBlank => {
                if self.stat & STAT_IF_HBLANK_B != 0 {
                    ints.req_lcd();
                }
            }
        }
    }

    #[must_use]
    #[inline]
    pub(crate) const fn pixel_data_rgba(&self) -> &[u8] {
        self.rgba_buf_present.pixel_data()
    }

    #[must_use]
    #[inline]
    pub(crate) const fn vram_data_rgba(&self) -> &[u8] {
        self.vram_renderer.vram_data_rgba()
    }
}
