use {
    crate::{
        interrupts::{LCD_STAT_INT, VBLANK_INT},
        FunctionMode, Gb,
    },
    core::hint::unreachable_unchecked,
};

mod scanline_renderer;

pub const PX_WIDTH: u8 = 160;
pub const PX_HEIGHT: u8 = 144;

const PX_TOTAL: u16 = PX_WIDTH as u16 * PX_HEIGHT as u16;

// Mode timings
const OAM_SCAN_CYCLES: i16 = 80; // Constant
const DRAWING_CYCLES: i16 = 172; // Variable, minimum ammount
const HBLANK_CYCLES: i16 = 204; // Variable, maximum ammount
const VBLANK_CYCLES: i16 = 456; // Constant

// LCDC bits
const BACKGROUND_ENABLED: u8 = 1;
const OBJ_ENABLED: u8 = 1 << 1;
const LARGE_SPRITES: u8 = 1 << 2;
const BG_TILE_MAP_AREA: u8 = 1 << 3;
const BG_WINDOW_TILE_DATA_AREA: u8 = 1 << 4;
const WINDOW_ENABLED: u8 = 1 << 5;
const WINDOW_TILE_MAP_AREA: u8 = 1 << 6;
pub const LCD_ENABLE: u8 = 1 << 7;

// STAT bits
const MODE_BITS: u8 = 3;
const LY_EQUALS_LYC: u8 = 1 << 2;
const HBLANK_INTERRUPT: u8 = 1 << 3;
const VBLANK_INTERRUPT: u8 = 1 << 4;
const OAM_INTERRUPT: u8 = 1 << 5;
const LY_EQUALS_LYC_INTERRUPT: u8 = 1 << 6;

// BG attributes bits
const BG_PAL: u8 = 0x7;
const BG_TILE_BANK: u8 = 0x8;
const BG_X_FLIP: u8 = 0x20;
const BG_Y_FLIP: u8 = 0x40;
const BG_TO_OAM_PR: u8 = 0x80;

pub const OAM_SIZE: usize = 0x100;

const VRAM_SIZE: usize = 0x2000;
pub const VRAM_SIZE_CGB: usize = VRAM_SIZE * 2;

// Sprite attributes bites
const SPR_CGB_PAL: u8 = 0x7;
const SPR_TILE_BANK: u8 = 0x8;
const SPR_PAL: u8 = 0x10;
const SPR_FLIP_X: u8 = 0x20;
const SPR_FLIP_Y: u8 = 0x40;
const SPR_BG_FIRST: u8 = 0x80;

// CGB palette RAM
const PAL_RAM_SIZE: usize = 0x20;
const PAL_RAM_SIZE_COLORS: usize = PAL_RAM_SIZE * 3;

// DMG palette colors RGB
const GRAYSCALE_PALETTE: [(u8, u8, u8); 4] = [
    (0xff, 0xff, 0xff),
    (0xcc, 0xcc, 0xcc),
    (0x77, 0x77, 0x77),
    (0x00, 0x00, 0x00),
];

pub trait VideoCallbacks {
    fn draw(&mut self, rgba_data: &[u8]);
}

const RGBA_BUF_SIZE: usize = PX_TOTAL as usize * 4;

pub struct RgbaBuf {
    data: [u8; RGBA_BUF_SIZE],
}

impl RgbaBuf {
    #[must_use]
    pub(crate) fn new() -> Self {
        Self {
            data: [0xff; RGBA_BUF_SIZE],
        }
    }

    fn set_px(&mut self, i: usize, rgb: (u8, u8, u8)) {
        let base = i * 4;
        self.data[base] = rgb.0;
        self.data[base + 1] = rgb.1;
        self.data[base + 2] = rgb.2;
    }
}

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum Mode {
    HBlank  = 0,
    VBlank  = 1,
    OamScan = 2,
    Drawing = 3,
}

impl Mode {
    pub(crate) fn cycles(self, scroll_x: u8) -> i16 {
        let scroll_adjust = (scroll_x & 7) as i16 * 4;
        match self {
            Mode::OamScan => OAM_SCAN_CYCLES,
            Mode::Drawing => DRAWING_CYCLES + scroll_adjust,
            Mode::HBlank => HBLANK_CYCLES - scroll_adjust,
            Mode::VBlank => VBLANK_CYCLES,
        }
    }
}

pub struct ColorPalette {
    // Rgb color ram
    col: [u8; PAL_RAM_SIZE_COLORS],
    idx: u8,
    inc: bool, // increment after write
}

impl ColorPalette {
    pub(crate) fn new() -> Self {
        Self {
            col: [0; PAL_RAM_SIZE_COLORS],
            idx: 0,
            inc: false,
        }
    }

    pub(crate) fn set_spec(&mut self, val: u8) {
        self.idx = val & 0x3f;
        self.inc = val & 0x80 != 0;
    }

    pub(crate) fn spec(&self) -> u8 {
        self.idx | 0x40 | ((self.inc as u8) << 7)
    }

    pub(crate) fn data(&self) -> u8 {
        let i = (self.idx as usize / 2) * 3;

        if self.idx & 1 == 0 {
            // red and green
            self.col[i] | (self.col[i + 1] << 5)
        } else {
            // green and blue
            (self.col[i + 1] >> 3) | (self.col[i + 2] << 2)
        }
    }

    pub(crate) fn set_data(&mut self, val: u8) {
        let i = (self.idx as usize / 2) * 3;

        if self.idx & 1 == 0 {
            // red
            self.col[i] = val & 0x1F;
            // green
            self.col[i + 1] = ((self.col[i + 1] & 3) << 3) | ((val & 0xe0) >> 5);
        } else {
            // green
            self.col[i + 1] = (self.col[i + 1] & 7) | ((val & 3) << 3);
            // blue
            self.col[i + 2] = (val & 0x7c) >> 2;
        }

        if self.inc {
            self.idx = (self.idx + 1) & 0x3f;
        }
    }

    fn get_color(&self, palette_number: u8, color_number: u8) -> (u8, u8, u8) {
        fn scale_channel(c: u8) -> u8 {
            (c << 3) | (c >> 2)
        }

        let i = (palette_number as usize * 4 + color_number as usize) * 3;
        let r = self.col[i];
        let g = self.col[i + 1];
        let b = self.col[i + 2];

        (scale_channel(r), scale_channel(g), scale_channel(b))
    }
}

#[derive(Default)]
struct Obj {
    pub x: u8,
    pub y: u8,
    pub tile_index: u8,
    pub attr: u8,
}

impl Gb {
    pub(crate) fn tick_ppu(&mut self) {
        fn check_lyc(gb: &mut Gb) {
            gb.stat &= !LY_EQUALS_LYC;

            if gb.ly == gb.lyc {
                gb.stat |= LY_EQUALS_LYC;
                if gb.stat & LY_EQUALS_LYC_INTERRUPT != 0 {
                    gb.req_interrupt(LCD_STAT_INT);
                }
            }
        }

        if self.lcdc & LCD_ENABLE == 0 {
            return;
        }

        self.cycles -= self.t_elapsed() as i16;

        if self.cycles > 0 {
            return;
        }

        match self.ppu_mode() {
            Mode::OamScan => self.switch_mode(Mode::Drawing),
            Mode::Drawing => {
                self.draw_scanline();
                self.switch_mode(Mode::HBlank);
            }
            Mode::HBlank => {
                self.ly += 1;
                if self.ly < 144 {
                    self.switch_mode(Mode::OamScan);
                } else {
                    self.switch_mode(Mode::VBlank);
                }
                check_lyc(self);
            }
            Mode::VBlank => {
                self.ly += 1;
                if self.ly > 153 {
                    self.ly = 0;
                    self.switch_mode(Mode::OamScan);
                    self.exit = true;
                    unsafe {
                        (*self.video_callbacks).draw(&self.rgba_buf.data);
                    }
                } else {
                    let scx = self.scx;
                    self.cycles += self.ppu_mode().cycles(scx);
                }
                check_lyc(self);
            }
        }
    }

    #[must_use]
    pub(crate) fn ppu_mode(&self) -> Mode {
        match self.stat & 3 {
            0 => Mode::HBlank,
            1 => Mode::VBlank,
            2 => Mode::OamScan,
            3 => Mode::Drawing,
            _ => unsafe { unreachable_unchecked() },
        }
    }

    pub(crate) fn read_vram(&mut self, addr: u16) -> u8 {
        match self.ppu_mode() {
            Mode::Drawing => 0xff,
            _ => self.vram[((addr & 0x1fff) + self.vbk as u16 * VRAM_SIZE as u16) as usize],
        }
    }

    pub(crate) fn read_oam(&mut self, addr: u16) -> u8 {
        match self.ppu_mode() {
            Mode::HBlank | Mode::VBlank if !self.dma_on => self.oam[(addr & 0xff) as usize],
            _ => 0xff,
        }
    }

    pub(crate) fn write_lcdc(&mut self, val: u8) {
        if val & LCD_ENABLE == 0 && self.lcdc & LCD_ENABLE != 0 {
            debug_assert!(self.ppu_mode() == Mode::VBlank);
            self.ly = 0;
        }

        if val & LCD_ENABLE != 0 && self.lcdc & LCD_ENABLE == 0 {
            self.set_mode(Mode::HBlank);
            self.stat |= LY_EQUALS_LYC;
            self.cycles = Mode::OamScan.cycles(self.scx);
        }

        self.lcdc = val;
    }

    pub(crate) fn write_stat(&mut self, val: u8) {
        let ly_equals_lyc = self.stat & LY_EQUALS_LYC;
        let mode: u8 = self.ppu_mode() as u8;

        self.stat = val;
        self.stat &= !(LY_EQUALS_LYC | MODE_BITS);
        self.stat |= ly_equals_lyc | mode;
    }

    pub(crate) fn write_vram(&mut self, addr: u16, val: u8) {
        match self.ppu_mode() {
            Mode::Drawing => (),
            _ => self.vram[((addr & 0x1fff) + self.vbk as u16 * VRAM_SIZE as u16) as usize] = val,
        };
    }

    pub(crate) fn write_oam(&mut self, addr: u16, val: u8, dma_active: bool) {
        match self.ppu_mode() {
            Mode::HBlank | Mode::VBlank if !dma_active => self.oam[(addr & 0xff) as usize] = val,
            _ => (),
        };
    }

    fn set_mode(&mut self, mode: Mode) {
        let bits: u8 = self.stat & !MODE_BITS;
        self.stat = bits | (mode as u8);
    }

    fn get_mono_color(index: u8) -> (u8, u8, u8) {
        GRAYSCALE_PALETTE[index as usize]
    }

    fn switch_mode(&mut self, mode: Mode) {
        self.set_mode(mode);
        let scx = self.scx;
        self.cycles += mode.cycles(scx);

        match mode {
            Mode::OamScan => {
                if self.stat & OAM_INTERRUPT != 0 {
                    self.req_interrupt(LCD_STAT_INT);
                }

                self.win_in_ly = false;
            }
            Mode::VBlank => {
                self.req_interrupt(VBLANK_INT);

                if self.stat & VBLANK_INTERRUPT != 0 {
                    self.req_interrupt(LCD_STAT_INT);
                }

                if self.stat & OAM_INTERRUPT != 0 {
                    self.req_interrupt(LCD_STAT_INT);
                }

                self.win_lines_skipped = 0;
                self.win_in_frame = false;
            }
            Mode::Drawing => (),
            Mode::HBlank => {
                if self.stat & HBLANK_INTERRUPT != 0 {
                    self.req_interrupt(LCD_STAT_INT);
                }
            }
        }
    }

    fn win_enabled(&self) -> bool {
        match self.function_mode {
            FunctionMode::Dmg | FunctionMode::Compat => {
                (self.lcdc & BACKGROUND_ENABLED != 0) && (self.lcdc & WINDOW_ENABLED != 0)
            }
            FunctionMode::Cgb => self.lcdc & WINDOW_ENABLED != 0,
        }
    }

    fn bg_enabled(&self) -> bool {
        match self.function_mode {
            FunctionMode::Dmg | FunctionMode::Compat => self.lcdc & BACKGROUND_ENABLED != 0,
            FunctionMode::Cgb => true,
        }
    }

    fn cgb_master_priority(&self) -> bool {
        match self.function_mode {
            FunctionMode::Dmg | FunctionMode::Compat => false,
            FunctionMode::Cgb => self.lcdc & BACKGROUND_ENABLED == 0,
        }
    }

    fn signed_byte_for_tile_offset(&self) -> bool {
        self.lcdc & BG_WINDOW_TILE_DATA_AREA == 0
    }

    fn bg_tile_map(&self) -> u16 {
        if self.lcdc & BG_TILE_MAP_AREA == 0 {
            0x9800
        } else {
            0x9c00
        }
    }

    fn win_tile_map(&self) -> u16 {
        if self.lcdc & WINDOW_TILE_MAP_AREA == 0 {
            0x9800
        } else {
            0x9c00
        }
    }

    fn tile_addr(&self, tile_number: u8) -> u16 {
        let base = if self.lcdc & BG_WINDOW_TILE_DATA_AREA == 0 {
            0x8800
        } else {
            0x8000
        };

        let offset = if self.signed_byte_for_tile_offset() {
            ((tile_number as i8 as i16) + 128) as u16 * 16
        } else {
            tile_number as u16 * 16
        };

        base + offset
    }

    fn vram_at_bank(&self, addr: u16, bank: u8) -> u8 {
        self.vram[((addr & 0x1fff) + bank as u16 * VRAM_SIZE as u16) as usize]
    }

    fn tile_number(&self, tile_map: u16) -> u8 {
        self.vram_at_bank(tile_map, 0)
    }

    fn bg_attr(&self, tile_addr: u16) -> u8 {
        self.vram_at_bank(tile_addr, 1)
    }

    fn bg_tile(&self, tile_addr: u16, attr: u8) -> (u8, u8) {
        let bank = (attr & BG_TILE_BANK != 0) as u8;
        let lo = self.vram_at_bank(tile_addr & 0x1fff, bank);
        let hi = self.vram_at_bank((tile_addr & 0x1fff) + 1, bank);

        (lo, hi)
    }

    fn obj_tile(&self, tile_addr: u16, obj: &Obj) -> (u8, u8) {
        let bank = (obj.attr & SPR_TILE_BANK != 0) as u8;
        let lo = self.vram_at_bank(tile_addr, bank);
        let hi = self.vram_at_bank(tile_addr + 1, bank);

        (lo, hi)
    }
}
