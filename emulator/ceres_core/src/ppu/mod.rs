use crate::interrupts::Interrupts;

use {self::color_palette::ColorPalette, crate::CMode, rgba_buf::RgbaBuf};

mod color_palette;
mod draw;
mod rgba_buf;

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
const OAM_SIZE: u16 = 0x100;
const VRAM_SIZE_GB: u16 = 0x2000;
const VRAM_SIZE_CGB: u16 = VRAM_SIZE_GB * 2;

#[derive(Clone, Copy)]
pub enum Mode {
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
    opri: u8,
    vbk: bool,
    bcp: ColorPalette,
    ocp: ColorPalette,

    vram: [u8; VRAM_SIZE_CGB as usize],
    oam: [u8; OAM_SIZE as usize],
    rgb_buf: RgbaBuf,
    rgba_buf_present: RgbaBuf,
    ppu_cycles: i32,
    ppu_win_in_frame: bool,
    ppu_win_in_ly: bool,
    ppu_win_skipped: u8,
}

impl Default for Ppu {
    fn default() -> Self {
        Self {
            vram: [0; VRAM_SIZE_CGB as usize],
            oam: [0; OAM_SIZE as usize],
            ppu_cycles: Mode::OamScan.cycles(0),
            // Default
            lcdc: Default::default(),
            stat: STAT_LYC_B | Mode::OamScan as u8,
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
            ppu_win_in_frame: Default::default(),
            ppu_win_in_ly: Default::default(),
            ppu_win_skipped: Default::default(),
        }
    }
}

// IO
impl Ppu {
    pub(crate) fn ocp_mut(&mut self) -> &mut ColorPalette {
        &mut self.ocp
    }

    pub(crate) fn bcp_mut(&mut self) -> &mut ColorPalette {
        &mut self.bcp
    }

    pub(crate) const fn ocp(&self) -> &ColorPalette {
        &self.ocp
    }

    pub(crate) const fn bcp(&self) -> &ColorPalette {
        &self.bcp
    }

    pub(crate) const fn read_stat(&self) -> u8 {
        self.stat | 0x80
    }

    pub(crate) const fn read_ly(&self) -> u8 {
        self.ly
    }

    pub(crate) fn write_opri(&mut self, val: u8) {
        self.opri = val;
    }

    pub(crate) const fn read_opri(&self) -> u8 {
        self.opri
    }

    pub(crate) fn write_vbk(&mut self, val: u8) {
        self.vbk = val & 1 != 0;
    }

    pub(crate) const fn read_vbk(&self) -> u8 {
        (self.vbk as u8) | 0xFE
    }

    pub(crate) fn write_scx(&mut self, val: u8) {
        self.scx = val;
    }

    pub(crate) const fn read_scx(&self) -> u8 {
        self.scx
    }

    pub(crate) fn write_scy(&mut self, val: u8) {
        self.scy = val;
    }

    pub(crate) const fn read_scy(&self) -> u8 {
        self.scy
    }

    pub(crate) fn write_lyc(&mut self, val: u8) {
        self.lyc = val;
    }

    pub(crate) const fn read_lyc(&self) -> u8 {
        self.lyc
    }

    pub(crate) fn write_bgp(&mut self, val: u8) {
        self.bgp = val;
    }

    pub(crate) const fn read_bgp(&self) -> u8 {
        self.bgp
    }

    pub(crate) fn write_obp0(&mut self, val: u8) {
        self.obp0 = val;
    }

    pub(crate) const fn read_obp0(&self) -> u8 {
        self.obp0
    }

    pub(crate) fn write_obp1(&mut self, val: u8) {
        self.obp1 = val;
    }

    pub(crate) const fn read_obp1(&self) -> u8 {
        self.obp1
    }

    pub(crate) fn write_wy(&mut self, val: u8) {
        self.wy = val;
    }

    pub(crate) const fn read_wy(&self) -> u8 {
        self.wy
    }

    pub(crate) fn write_wx(&mut self, val: u8) {
        self.wx = val;
    }

    pub(crate) const fn read_wx(&self) -> u8 {
        self.wx
    }

    pub(crate) const fn read_lcdc(&self) -> u8 {
        self.lcdc
    }

    pub(crate) fn write_lcdc(&mut self, val: u8, ints: &mut Interrupts) {
        // turn off
        if val & LCDC_ON_B == 0 && self.lcdc & LCDC_ON_B != 0 {
            // debug_assert!(self.ppu_mode() == Mode::VBlank);
            // self.scx = 0;
            // self.rgb_buf.clear();
            self.ly = 0;
        }

        // turn on
        if val & LCDC_ON_B != 0 && self.lcdc & LCDC_ON_B == 0 {
            self.set_mode_stat(Mode::OamScan);
            self.ppu_cycles = Mode::OamScan.cycles(self.scx);
            self.ly = 0;
            self.check_lyc(ints);
        }

        self.lcdc = val;
    }

    pub(crate) fn write_stat(&mut self, val: u8) {
        let ly_equals_lyc = self.stat & STAT_LYC_B;
        let mode: u8 = self.ppu_mode() as u8;

        self.stat = val;
        self.stat &= !(STAT_LYC_B | STAT_MODE_B);
        self.stat |= ly_equals_lyc | mode;
    }

    pub(crate) const fn read_vram(&self, addr: u16) -> u8 {
        #[allow(clippy::single_match_else)]
        match self.ppu_mode() {
            Mode::Drawing => 0xFF,
            _ => {
                let bank = self.vbk as u16 * VRAM_SIZE_GB;
                let i = (addr & 0x1FFF) + bank;
                self.vram[i as usize]
            }
        }
    }

    pub(crate) fn write_vram(&mut self, addr: u16, val: u8) {
        if !matches!(self.ppu_mode(), Mode::Drawing) {
            let bank = u16::from(self.vbk) * VRAM_SIZE_GB;
            let i = (addr & 0x1FFF) + bank;
            self.vram[i as usize] = val;
        }
    }

    pub(crate) const fn read_oam(&self, addr: u16, dma_on: bool) -> u8 {
        match self.ppu_mode() {
            Mode::HBlank | Mode::VBlank if !dma_on => self.oam[(addr & 0xFF) as usize],
            _ => 0xFF,
        }
    }

    pub(crate) fn write_oam(&mut self, addr: u16, val: u8, dma_active: bool) {
        match self.ppu_mode() {
            Mode::HBlank | Mode::VBlank if !dma_active => {
                self.oam[(addr & 0xFF) as usize] = val;
            }
            _ => (),
        };
    }

    pub(crate) fn write_oam_direct(&mut self, addr: u16, val: u8) {
        self.oam[(addr & 0xFF) as usize] = val;
    }
}

// General
impl Ppu {
    pub(crate) fn run(&mut self, cycles: i32, ints: &mut Interrupts, compat_mode: CMode) {
        if self.lcdc & LCDC_ON_B == 0 {
            return;
        }

        self.ppu_cycles -= cycles;

        if self.ppu_cycles < 0 {
            match self.ppu_mode() {
                Mode::OamScan => self.enter_mode(Mode::Drawing, ints),
                Mode::Drawing => {
                    self.draw_scanline(compat_mode);
                    self.enter_mode(Mode::HBlank, ints);
                }
                Mode::HBlank => {
                    self.ly += 1;
                    if self.ly < 144 {
                        self.enter_mode(Mode::OamScan, ints);
                    } else {
                        self.enter_mode(Mode::VBlank, ints);
                    }
                    self.check_lyc(ints);
                }
                Mode::VBlank => {
                    self.ly += 1;
                    if self.ly > 153 {
                        self.ly = 0;
                        self.rgba_buf_present = self.rgb_buf.clone();
                        self.enter_mode(Mode::OamScan, ints);
                    } else {
                        self.ppu_cycles = self
                            .ppu_cycles
                            .wrapping_add(self.ppu_mode().cycles(self.scx));
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
    pub(crate) const fn ppu_mode(&self) -> Mode {
        match self.stat & 3 {
            0 => Mode::HBlank,
            1 => Mode::VBlank,
            2 => Mode::OamScan,
            _ => Mode::Drawing,
        }
    }

    fn set_mode_stat(&mut self, mode: Mode) {
        self.stat = (self.stat & !STAT_MODE_B) | mode as u8;
    }

    fn enter_mode(&mut self, mode: Mode, ints: &mut Interrupts) {
        self.set_mode_stat(mode);
        self.ppu_cycles = self.ppu_cycles.wrapping_add(mode.cycles(self.scx));

        match mode {
            Mode::OamScan => {
                if self.stat & STAT_IF_OAM_B != 0 {
                    ints.req_lcd();
                }

                self.ppu_win_in_ly = false;
            }
            Mode::VBlank => {
                ints.req_vblank();

                if self.stat & STAT_IF_VBLANK_B != 0 {
                    ints.req_lcd();
                }

                // TODO: why?
                // if self.stat & STAT_IF_OAM_B != 0 {
                //   *ifr |= IF_LCD_B;
                // }

                self.ppu_win_skipped = 0;
                self.ppu_win_in_frame = false;
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
    pub(crate) const fn pixel_data_rgb(&self) -> &[u8] {
        self.rgba_buf_present.pixel_data()
    }
}
