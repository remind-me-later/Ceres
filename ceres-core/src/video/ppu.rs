use {
    super::{
        palette::{ColorPalette, MonochromePalette, MonochromePaletteColors},
        pixel_data::PixelData,
        sprites::Oam,
        vram::Vram,
        VideoCallbacks, ACCESS_OAM_CYCLES, ACCESS_VRAM_CYCLES, HBLANK_CYCLES, VBLANK_LINE_CYCLES,
    },
    crate::{
        interrupts::{Interrupts, LCD_STAT_INT, VBLANK_INT},
        FunctionMode,
    },
    alloc::rc::Rc,
    core::cell::RefCell,
};

mod scanline_renderer;

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum Mode {
    OamScan,
    DrawingPixels,
    HBlank,
    VBlank,
}

impl Mode {
    pub fn cycles(self, scroll_x: u8) -> i16 {
        let scroll_adjust = (scroll_x & 0x7) as i16;
        match self {
            Mode::OamScan => ACCESS_OAM_CYCLES,
            Mode::DrawingPixels => ACCESS_VRAM_CYCLES + scroll_adjust * 4,
            Mode::HBlank => HBLANK_CYCLES - scroll_adjust * 4,
            Mode::VBlank => VBLANK_LINE_CYCLES,
        }
    }

    pub fn to_u8_low(self) -> u8 {
        match self {
            Mode::HBlank => 0,
            Mode::VBlank => 1,
            Mode::OamScan => 2,
            Mode::DrawingPixels => 3,
        }
    }
}

// LCDC bits
pub const BACKGROUND_ENABLED: u8 = 1;
pub const OBJECTS_ENABLED: u8 = 1 << 1;
pub const LARGE_SPRITES: u8 = 1 << 2;
pub const BG_TILE_MAP_AREA: u8 = 1 << 3;
pub const BG_WINDOW_TILE_DATA_AREA: u8 = 1 << 4;
pub const WINDOW_ENABLED: u8 = 1 << 5;
pub const WINDOW_TILE_MAP_AREA: u8 = 1 << 6;
pub const LCD_ENABLE: u8 = 1 << 7;

#[derive(Clone, Copy, Default)]
pub struct Lcdc {
    pub val: u8,
}

impl Lcdc {
    fn win_enabled(self, function_mode: FunctionMode) -> bool {
        match function_mode {
            FunctionMode::Monochrome | FunctionMode::Compatibility => {
                self.val & BACKGROUND_ENABLED & WINDOW_ENABLED != 0
            }
            FunctionMode::Color => self.val & WINDOW_ENABLED != 0,
        }
    }

    fn bg_enabled(self, function_mode: FunctionMode) -> bool {
        match function_mode {
            FunctionMode::Monochrome | FunctionMode::Compatibility => {
                self.val & BACKGROUND_ENABLED != 0
            }
            FunctionMode::Color => true,
        }
    }

    fn cgb_sprite_master_priority_on(self, function_mode: FunctionMode) -> bool {
        match function_mode {
            FunctionMode::Monochrome | FunctionMode::Compatibility => false,
            FunctionMode::Color => self.val & BACKGROUND_ENABLED == 0,
        }
    }

    fn signed_byte_for_tile_offset(self) -> bool {
        self.val & BG_WINDOW_TILE_DATA_AREA == 0
    }

    pub fn bg_tile_map_addr(self) -> u16 {
        if self.val & BG_TILE_MAP_AREA != 0 {
            0x9c00
        } else {
            0x9800
        }
    }

    pub fn window_tile_map_addr(self) -> u16 {
        if self.val & WINDOW_TILE_MAP_AREA != 0 {
            0x9c00
        } else {
            0x9800
        }
    }

    fn bg_window_tile_addr(self) -> u16 {
        if self.val & BG_WINDOW_TILE_DATA_AREA != 0 {
            0x8000
        } else {
            0x8800
        }
    }

    pub fn tile_data_addr(self, tile_number: u8) -> u16 {
        self.bg_window_tile_addr()
            + if self.signed_byte_for_tile_offset() {
                ((tile_number as i8 as i16) + 128) as u16 * 16
            } else {
                tile_number as u16 * 16
            }
    }
}

// STAT bits
const MODE_BITS: u8 = 3;
const LY_EQUALS_LYC: u8 = 1 << 2;
const HBLANK_INTERRUPT: u8 = 1 << 3;
const VBLANK_INTERRUPT: u8 = 1 << 4;
const OAM_INTERRUPT: u8 = 1 << 5;
const LY_EQUALS_LYC_INTERRUPT: u8 = 1 << 6;

#[derive(Clone, Copy, Default)]
pub struct Stat {
    pub val: u8,
}

impl Stat {
    pub fn set_mode(&mut self, mode: Mode) {
        let bits: u8 = self.val & !MODE_BITS;
        let mode: u8 = mode.to_u8_low();
        self.val = bits | mode;
    }

    pub fn mode(self) -> Mode {
        match self.val & 0x3 {
            0 => Mode::HBlank,
            1 => Mode::VBlank,
            2 => Mode::OamScan,
            _ => Mode::DrawingPixels,
        }
    }
}

pub const BG_PAL: u8 = 0x7;
pub const BG_TILE_BANK: u8 = 0x8;
pub const BG_X_FLIP: u8 = 0x20;
pub const BG_Y_FLIP: u8 = 0x40;
pub const BG_TO_OAM_PR: u8 = 0x80;

#[derive(Clone, Copy, PartialEq, Eq)]
enum PixelPriority {
    SpritesOnTop,
    BackgroundOnTop,
    Normal,
}

pub struct Ppu {
    monochrome_palette_colors: MonochromePaletteColors,
    vram: Vram,
    oam: Oam,
    cycles: i16,
    pixel_data: PixelData,
    frame_used_window: bool,
    scanline_used_window: bool,
    window_lines_skipped: u16,
    is_frame_done: bool,
    video_callbacks: Rc<RefCell<dyn VideoCallbacks>>,

    // registers
    lcdc: Lcdc,
    stat: Stat,
    scy: u8,
    scx: u8,
    ly: u8,
    lyc: u8,
    wy: u8,
    wx: u8,
    opri: u8,
    bgp: MonochromePalette,
    obp0: MonochromePalette,
    obp1: MonochromePalette,
    cgb_bg_palette: ColorPalette,
    cgb_sprite_palette: ColorPalette,
}

impl Ppu {
    pub fn new(video_callbacks: Rc<RefCell<dyn VideoCallbacks>>) -> Self {
        let stat = Stat { val: 0 };
        let cycles = stat.mode().cycles(0);

        Self {
            monochrome_palette_colors: MonochromePaletteColors::Grayscale,
            vram: Vram::new(),
            oam: Oam::new(),
            pixel_data: PixelData::new(),
            cycles,
            frame_used_window: false,
            window_lines_skipped: 0,
            scanline_used_window: false,
            is_frame_done: false,
            video_callbacks,
            lcdc: Lcdc::default(),
            stat,
            scy: 0,
            scx: 0,
            ly: 0,
            lyc: 0,
            wy: 0,
            wx: 0,
            bgp: MonochromePalette::new(),
            obp0: MonochromePalette::new(),
            obp1: MonochromePalette::new(),
            cgb_bg_palette: ColorPalette::new(),
            cgb_sprite_palette: ColorPalette::new(),
            opri: 0,
        }
    }

    pub fn reset_frame_done(&mut self) {
        self.is_frame_done = false;
    }

    pub fn is_frame_done(&self) -> bool {
        self.is_frame_done
    }

    pub fn read_lcdc(&mut self) -> u8 {
        self.lcdc.val
    }

    pub fn read_stat(&mut self) -> u8 {
        if self.lcdc.val & LCD_ENABLE != 0 {
            self.stat.val | 0x80
        } else {
            0x80
        }
    }

    pub fn read_scy(&mut self) -> u8 {
        self.scy
    }

    pub fn read_scx(&mut self) -> u8 {
        self.scx
    }

    pub fn read_ly(&mut self) -> u8 {
        self.ly
    }

    pub fn read_lyc(&mut self) -> u8 {
        self.lyc
    }

    pub fn read_wy(&mut self) -> u8 {
        self.wy
    }

    pub fn read_wx(&mut self) -> u8 {
        self.wx
    }

    pub fn read_bgp(&mut self) -> u8 {
        self.bgp.val
    }

    pub fn read_obp0(&mut self) -> u8 {
        self.obp0.val
    }

    pub fn read_obp1(&mut self) -> u8 {
        self.obp1.val
    }

    pub fn read_bcps(&mut self) -> u8 {
        self.cgb_bg_palette.spec()
    }

    pub fn read_bcpd(&mut self) -> u8 {
        self.cgb_bg_palette.data()
    }

    pub fn read_ocps(&mut self) -> u8 {
        self.cgb_sprite_palette.spec()
    }

    pub fn read_ocpd(&mut self) -> u8 {
        self.cgb_sprite_palette.data()
    }

    pub fn read_opri(&mut self) -> u8 {
        self.opri
    }

    pub fn read_vram(&mut self, addr: u16) -> u8 {
        let mode = self.stat.mode();

        match mode {
            Mode::DrawingPixels => 0xff,
            _ => self.vram.read(addr),
        }
    }

    pub fn read_vbk(&mut self) -> u8 {
        self.vram.read_bank_number()
    }

    pub fn read_oam(&mut self, addr: u16, dma_active: bool) -> u8 {
        let mode = self.stat.mode();

        match mode {
            Mode::HBlank | Mode::VBlank if !dma_active => self.oam.read(addr as u8),
            _ => 0xff,
        }
    }

    pub fn write_lcdc(&mut self, val: u8) {
        if val & LCD_ENABLE == 0 && self.lcdc.val & LCD_ENABLE != 0 {
            debug_assert!(self.stat.mode() == Mode::VBlank);
            self.ly = 0;
        }

        if val & LCD_ENABLE != 0 && self.lcdc.val & LCD_ENABLE == 0 {
            self.stat.set_mode(Mode::HBlank);
            self.stat.val |= LY_EQUALS_LYC;
            self.cycles = Mode::OamScan.cycles(self.scx);
        }

        self.lcdc.val = val;
    }

    pub fn write_stat(&mut self, val: u8) {
        let ly_equals_lyc = self.stat.val & LY_EQUALS_LYC;
        let mode: u8 = self.stat.mode().to_u8_low();

        self.stat.val = val;
        self.stat.val &= !(LY_EQUALS_LYC | MODE_BITS);
        self.stat.val |= ly_equals_lyc | mode;
    }

    pub fn write_scy(&mut self, val: u8) {
        self.scy = val;
    }

    pub fn write_scx(&mut self, val: u8) {
        self.scx = val;
    }

    pub fn write_ly(&mut self, _: u8) {}

    pub fn write_lyc(&mut self, val: u8) {
        self.lyc = val;
    }

    pub fn write_wy(&mut self, val: u8) {
        self.wy = val;
    }

    pub fn write_wx(&mut self, val: u8) {
        self.wx = val;
    }

    pub fn write_bgp(&mut self, val: u8) {
        self.bgp.val = val
    }

    pub fn write_obp0(&mut self, val: u8) {
        self.obp0.val = val;
    }

    pub fn write_obp1(&mut self, val: u8) {
        self.obp1.val = val;
    }

    pub fn write_bcps(&mut self, val: u8) {
        self.cgb_bg_palette.set_spec(val);
    }

    pub fn write_bcpd(&mut self, val: u8) {
        self.cgb_bg_palette.set_data(val);
    }

    pub fn write_ocps(&mut self, val: u8) {
        self.cgb_sprite_palette.set_spec(val);
    }

    pub fn write_ocpd(&mut self, val: u8) {
        self.cgb_sprite_palette.set_data(val);
    }

    pub fn write_opri(&mut self, val: u8) {
        self.opri = val;
    }

    pub fn write_vram(&mut self, addr: u16, val: u8) {
        let mode = self.stat.mode();

        match mode {
            Mode::DrawingPixels => (),
            _ => self.vram.write(addr, val),
        };
    }

    pub fn write_vbk(&mut self, val: u8) {
        self.vram.write_bank_number(val);
    }

    pub fn write_oam(&mut self, addr: u16, val: u8, dma_active: bool) {
        let mode = self.stat.mode();

        match mode {
            Mode::HBlank | Mode::VBlank if !dma_active => self.oam.write(addr as u8, val),
            _ => (),
        };
    }

    pub fn hdma_write(&mut self, addr: u16, val: u8) {
        let mode = self.stat.mode();

        match mode {
            Mode::DrawingPixels => (),
            _ => self.vram.write(addr, val),
        }
    }

    pub fn dma_write(&mut self, addr: u8, val: u8) {
        self.oam.write(addr, val)
    }

    fn switch_mode(&mut self, mode: Mode, ints: &mut Interrupts) {
        self.stat.set_mode(mode);
        let scx = self.scx;
        self.cycles += mode.cycles(scx);
        let stat = self.stat;

        match mode {
            Mode::OamScan => {
                if stat.val & OAM_INTERRUPT != 0 {
                    ints.request(LCD_STAT_INT);
                }

                self.scanline_used_window = false;
            }
            Mode::VBlank => {
                ints.request(VBLANK_INT);

                if stat.val & VBLANK_INTERRUPT != 0 {
                    ints.request(LCD_STAT_INT);
                }

                if stat.val & OAM_INTERRUPT != 0 {
                    ints.request(LCD_STAT_INT);
                }

                self.window_lines_skipped = 0;
                self.frame_used_window = false;
            }
            Mode::DrawingPixels => (),
            Mode::HBlank => {
                if stat.val & HBLANK_INTERRUPT != 0 {
                    ints.request(LCD_STAT_INT);
                }
            }
        }
    }

    pub fn mode(&self) -> Mode {
        self.stat.mode()
    }

    pub(crate) fn tick(
        &mut self,
        ints: &mut Interrupts,
        function_mode: FunctionMode,
        mus_elapsed: u8,
    ) {
        if self.lcdc.val & LCD_ENABLE == 0 {
            return;
        }

        self.cycles -= i16::from(mus_elapsed);
        let stat = self.stat;

        if self.cycles > 0 {
            return;
        }

        match stat.mode() {
            Mode::OamScan => self.switch_mode(Mode::DrawingPixels, ints),
            Mode::DrawingPixels => {
                self.draw_scanline(function_mode);
                self.switch_mode(Mode::HBlank, ints);
            }
            Mode::HBlank => {
                self.ly += 1;
                if self.ly < 144 {
                    self.switch_mode(Mode::OamScan, ints);
                } else {
                    self.switch_mode(Mode::VBlank, ints);
                }
                self.check_compare_interrupt(ints);
            }
            Mode::VBlank => {
                self.ly += 1;
                if self.ly > 153 {
                    self.ly = 0;
                    self.switch_mode(Mode::OamScan, ints);
                    self.is_frame_done = true;
                    self.video_callbacks
                        .borrow_mut()
                        .draw(self.pixel_data.rgba());
                } else {
                    let scx = self.scx;
                    self.cycles += self.stat.mode().cycles(scx);
                }
                self.check_compare_interrupt(ints);
            }
        };
    }

    fn check_compare_interrupt(&mut self, ints: &mut Interrupts) {
        self.stat.val &= !LY_EQUALS_LYC;

        if self.ly == self.lyc {
            self.stat.val |= LY_EQUALS_LYC;
            if self.stat.val & LY_EQUALS_LYC_INTERRUPT != 0 {
                ints.request(LCD_STAT_INT);
            }
        }
    }
}
