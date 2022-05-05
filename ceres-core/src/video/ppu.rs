mod scanline_renderer;

use {
    super::{
        palette::{ColorPalette, MonochromePalette, MonochromePaletteColors},
        pixel_data::PixelData,
        sprites::Oam,
        vram::Vram,
        ACCESS_OAM_CYCLES, ACCESS_VRAM_CYCLES, HBLANK_CYCLES, VBLANK_LINE_CYCLES,
    },
    crate::{
        interrupts::{Interrupt, Interrupts},
        memory::FunctionMode,
    },
    bitflags::bitflags,
};

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
}

impl From<u8> for Mode {
    fn from(val: u8) -> Self {
        match val & 0x3 {
            0 => Self::HBlank,
            1 => Self::VBlank,
            2 => Self::OamScan,
            _ => Self::DrawingPixels,
        }
    }
}

impl From<Mode> for u8 {
    fn from(val: Mode) -> Self {
        match val {
            Mode::HBlank => 0,
            Mode::VBlank => 1,
            Mode::OamScan => 2,
            Mode::DrawingPixels => 3,
        }
    }
}

bitflags!(
  pub struct Lcdc: u8 {
    const BACKGROUND_ENABLED = 1;
    const OBJECTS_ENABLED = 1 << 1;
    const LARGE_SPRITES = 1 << 2;
    const BG_TILE_MAP_AREA = 1 << 3;
    const BG_WINDOW_TILE_DATA_AREA = 1 << 4;
    const WINDOW_ENABLED = 1 << 5;
    const WINDOW_TILE_MAP_AREA = 1 << 6;
    const LCD_ENABLE = 1 << 7;
  }
);

impl Lcdc {
    pub fn window_enabled(self, function_mode: FunctionMode) -> bool {
        match function_mode {
            FunctionMode::Monochrome | FunctionMode::Compatibility => {
                self.contains(Self::BACKGROUND_ENABLED) && self.contains(Self::WINDOW_ENABLED)
            }
            FunctionMode::Color => self.contains(Self::WINDOW_ENABLED),
        }
    }

    pub fn background_enabled(self, function_mode: FunctionMode) -> bool {
        match function_mode {
            FunctionMode::Monochrome | FunctionMode::Compatibility => {
                self.contains(Self::BACKGROUND_ENABLED)
            }
            FunctionMode::Color => true,
        }
    }

    pub fn cgb_sprite_master_priority_on(self, function_mode: FunctionMode) -> bool {
        match function_mode {
            FunctionMode::Monochrome | FunctionMode::Compatibility => false,
            FunctionMode::Color => !self.contains(Self::BACKGROUND_ENABLED),
        }
    }

    fn signed_byte_for_tile_offset(self) -> bool {
        !self.contains(Self::BG_WINDOW_TILE_DATA_AREA)
    }

    pub fn bg_tile_map_addr(self) -> u16 {
        if self.contains(Self::BG_TILE_MAP_AREA) {
            0x9c00
        } else {
            0x9800
        }
    }

    pub fn window_tile_map_addr(self) -> u16 {
        if self.contains(Self::WINDOW_TILE_MAP_AREA) {
            0x9c00
        } else {
            0x9800
        }
    }

    fn bg_window_tile_addr(self) -> u16 {
        if self.contains(Self::BG_WINDOW_TILE_DATA_AREA) {
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

bitflags!(
  pub struct Stat: u8 {
    const MODE_FLAG_LOW = 1;
    const MODE_FLAG_HIGH  = 1 << 1;
    const LY_EQUALS_LYC = 1 << 2;
    const HBLANK_INTERRUPT = 1 << 3;
    const VBLANK_INTERRUPT = 1 << 4;
    const OAM_INTERRUPT = 1 << 5;
    const LY_EQUALS_LYC_INTERRUPT = 1 << 6;
  }
);

impl Stat {
    pub fn set_mode(&mut self, mode: Mode) {
        let bits: u8 = self.bits() & !3;
        let mode: u8 = mode.into();
        *self = Self::from_bits_truncate(bits | mode);
    }

    pub fn mode(self) -> Mode {
        self.bits().into()
    }
}

bitflags! {
   pub struct BgAttributes: u8{
        const PALETTE_NUMBER   = 0b0000_0111;
        const VRAM_BANK_NUMBER = 0b0000_1000;
        const X_FLIP           = 0b0010_0000;
        const Y_FLIP           = 0b0100_0000;
        const BG_TO_OAM_PR     = 0b1000_0000;
    }
}

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

    // registers
    lcdc: Lcdc,                       // lcd control
    stat: Stat,                       // lcd status
    scy: u8,                          // scroll y
    scx: u8,                          // scroll x
    ly: u8,                           // LCD Y coordinate
    lyc: u8,                          // LY compare
    wy: u8,                           // window y position
    wx: u8,                           // window x position
    opri: u8,                         // object priority mode
    bgp: MonochromePalette,           // bg palette data
    obp0: MonochromePalette,          // obj palette 0
    obp1: MonochromePalette,          // obj palette 1
    cgb_bg_palette: ColorPalette,     // cgb only
    cgb_sprite_palette: ColorPalette, // cgb only
}

impl Ppu {
    pub fn new() -> Self {
        let stat = Stat::from_bits_truncate(2);
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
            lcdc: Lcdc::empty(),
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

    pub fn mut_pixel_data(&mut self) -> &mut PixelData {
        &mut self.pixel_data
    }

    pub fn reset_frame_done(&mut self) {
        self.is_frame_done = false;
    }

    pub fn is_frame_done(&self) -> bool {
        self.is_frame_done
    }

    pub fn read_lcdc(&mut self) -> u8 {
        self.lcdc.bits()
    }

    pub fn read_stat(&mut self) -> u8 {
        if self.lcdc.contains(Lcdc::LCD_ENABLE) {
            self.stat.bits() | 0b1000_0000
        } else {
            0b1000_0000
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
        let new_lcdc = Lcdc::from_bits_truncate(val);

        if !new_lcdc.contains(Lcdc::LCD_ENABLE) && self.lcdc.contains(Lcdc::LCD_ENABLE) {
            if self.stat.mode() != Mode::VBlank {
                log::error!("LCD off, but not in VBlank");
            }
            self.ly = 0;
        }

        if new_lcdc.contains(Lcdc::LCD_ENABLE) && !self.lcdc.contains(Lcdc::LCD_ENABLE) {
            self.stat.set_mode(Mode::HBlank);
            self.stat.insert(Stat::LY_EQUALS_LYC);
            self.cycles = Mode::OamScan.cycles(self.scx);
        }

        self.lcdc = new_lcdc;
    }

    pub fn write_stat(&mut self, val: u8) {
        let ly_equals_lyc = self.stat & Stat::LY_EQUALS_LYC;
        let mode = self.stat.mode();

        self.stat = Stat::from_bits_truncate(val);
        self.stat.remove(Stat::LY_EQUALS_LYC);
        self.stat.remove(Stat::MODE_FLAG_HIGH);
        self.stat.remove(Stat::MODE_FLAG_LOW);
        self.stat.insert(ly_equals_lyc);
        self.stat.insert(Stat::from_bits_truncate(mode.into()));
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

    fn switch_mode(&mut self, mode: Mode, interrupt_controller: &mut Interrupts) {
        self.stat.set_mode(mode);
        let scx = self.scx;
        self.cycles += mode.cycles(scx);
        let stat = self.stat;

        match mode {
            Mode::OamScan => {
                if stat.contains(Stat::OAM_INTERRUPT) {
                    interrupt_controller.request(Interrupt::LCD_STAT);
                }

                self.scanline_used_window = false;
            }
            Mode::VBlank => {
                interrupt_controller.request(Interrupt::VBLANK);

                if stat.contains(Stat::VBLANK_INTERRUPT) {
                    interrupt_controller.request(Interrupt::LCD_STAT);
                }

                if stat.contains(Stat::OAM_INTERRUPT) {
                    interrupt_controller.request(Interrupt::LCD_STAT);
                }

                self.window_lines_skipped = 0;
                self.frame_used_window = false;
            }
            Mode::DrawingPixels => (),
            Mode::HBlank => {
                if stat.contains(Stat::HBLANK_INTERRUPT) {
                    interrupt_controller.request(Interrupt::LCD_STAT);
                }
            }
        }
    }

    pub fn mode(&self) -> Mode {
        self.stat.mode()
    }

    pub fn tick(
        &mut self,
        interrupt_controller: &mut Interrupts,
        function_mode: FunctionMode,
        mus_elapsed: u8,
    ) {
        if !self.lcdc.contains(Lcdc::LCD_ENABLE) {
            return;
        }

        self.cycles -= i16::from(mus_elapsed);
        let stat = self.stat;

        if self.cycles > 0 {
            return;
        }

        match stat.mode() {
            Mode::OamScan => self.switch_mode(Mode::DrawingPixels, interrupt_controller),
            Mode::DrawingPixels => {
                self.draw_scanline(function_mode);
                self.switch_mode(Mode::HBlank, interrupt_controller);
            }
            Mode::HBlank => {
                self.ly += 1;
                if self.ly < 144 {
                    self.switch_mode(Mode::OamScan, interrupt_controller);
                } else {
                    self.switch_mode(Mode::VBlank, interrupt_controller);
                }
                self.check_compare_interrupt(interrupt_controller);
            }
            Mode::VBlank => {
                self.ly += 1;
                if self.ly > 153 {
                    self.ly = 0;
                    self.switch_mode(Mode::OamScan, interrupt_controller);
                    self.is_frame_done = true;
                } else {
                    let scx = self.scx;
                    self.cycles += self.stat.mode().cycles(scx);
                }
                self.check_compare_interrupt(interrupt_controller);
            }
        };
    }

    fn check_compare_interrupt(&mut self, interrupt_controller: &mut Interrupts) {
        self.stat.remove(Stat::LY_EQUALS_LYC);

        if self.ly == self.lyc {
            self.stat.insert(Stat::LY_EQUALS_LYC);
            if self.stat.contains(Stat::LY_EQUALS_LYC_INTERRUPT) {
                interrupt_controller.request(Interrupt::LCD_STAT);
            }
        }
    }
}
