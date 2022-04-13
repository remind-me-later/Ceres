use super::{register::PpuRegister, Mode};
use crate::{
    memory::FunctionMode,
    video::palette::{ColorPalette, MonochromePalette},
};
use bitflags::bitflags;

pub struct Registers {
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

impl Default for Registers {
    fn default() -> Self {
        Self {
            lcdc: Lcdc::empty(),
            stat: Stat::from_bits_truncate(2),
            scy: 0,
            scx: 0,
            ly: 0,
            lyc: 0,
            wy: 0,
            wx: 0,
            bgp: MonochromePalette::new(0),
            obp0: MonochromePalette::new(0),
            obp1: MonochromePalette::new(0),
            cgb_bg_palette: ColorPalette::new(),
            cgb_sprite_palette: ColorPalette::new(),
            opri: 0,
        }
    }
}

impl Registers {
    pub fn new() -> Self {
        Self::default()
    }

    pub const fn cgb_bg_palette(&self) -> &ColorPalette {
        &self.cgb_bg_palette
    }

    pub const fn cgb_sprite_palette(&self) -> &ColorPalette {
        &self.cgb_sprite_palette
    }

    pub const fn lcdc(&self) -> &Lcdc {
        &self.lcdc
    }

    pub const fn stat(&self) -> &Stat {
        &self.stat
    }

    pub fn mut_stat(&mut self) -> &mut Stat {
        &mut self.stat
    }

    pub const fn ly(&self) -> u8 {
        self.ly
    }

    pub fn mut_ly(&mut self) -> &mut u8 {
        &mut self.ly
    }

    pub const fn scx(&self) -> u8 {
        self.scx
    }

    pub const fn scy(&self) -> u8 {
        self.scy
    }

    pub const fn wx(&self) -> u8 {
        self.wx
    }

    pub const fn wy(&self) -> u8 {
        self.wy
    }

    pub const fn bgp(&self) -> &MonochromePalette {
        &self.bgp
    }

    pub const fn obp0(&self) -> &MonochromePalette {
        &self.obp0
    }

    pub const fn obp1(&self) -> &MonochromePalette {
        &self.obp1
    }

    pub const fn prioritize_by_oam(&self) -> bool {
        self.opri & 1 == 0
    }

    pub const fn is_on_coincidence_scanline(&self) -> bool {
        self.ly == self.lyc
    }

    pub const fn read(&self, reg: PpuRegister) -> u8 {
        match reg {
            PpuRegister::Lcdc => self.lcdc.bits(),
            PpuRegister::Stat => {
                if self.lcdc().contains(Lcdc::LCD_ENABLE) {
                    self.stat.bits() | 0b1000_0000
                } else {
                    0b1000_0000
                }
            }
            PpuRegister::Scy => self.scy,
            PpuRegister::Scx => self.scx,
            PpuRegister::Ly => self.ly,
            PpuRegister::Lyc => self.lyc,
            PpuRegister::Wy => self.wy,
            PpuRegister::Wx => self.wx,
            PpuRegister::Bgp => self.bgp.get(),
            PpuRegister::Obp0 => self.obp0.get(),
            PpuRegister::Obp1 => self.obp1.get(),
            PpuRegister::Bcps => self.cgb_bg_palette.color_palette_specification(),
            PpuRegister::Bcpd => self.cgb_bg_palette.color_palette_data(),
            PpuRegister::Ocps => self.cgb_sprite_palette.color_palette_specification(),
            PpuRegister::Ocpd => self.cgb_sprite_palette.color_palette_data(),
            PpuRegister::Opri => self.opri,
        }
    }

    pub fn write(&mut self, reg: PpuRegister, val: u8, cycles: &mut i16) {
        match reg {
            PpuRegister::Lcdc => {
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
                    *cycles = Mode::OamScan.cycles(self.scx);
                }

                self.lcdc = new_lcdc;
            }
            PpuRegister::Stat => {
                let ly_equals_lyc = self.stat & Stat::LY_EQUALS_LYC;
                let mode = self.stat.mode();

                self.stat = Stat::from_bits_truncate(val);
                self.stat.remove(Stat::LY_EQUALS_LYC);
                self.stat.remove(Stat::MODE_FLAG_HIGH);
                self.stat.remove(Stat::MODE_FLAG_LOW);
                self.stat.insert(ly_equals_lyc);
                self.stat.insert(Stat::from_bits_truncate(mode.into()));
            }
            PpuRegister::Scy => self.scy = val,
            PpuRegister::Scx => self.scx = val,
            PpuRegister::Ly => (),
            PpuRegister::Lyc => self.lyc = val,
            PpuRegister::Wy => self.wy = val,
            PpuRegister::Wx => self.wx = val,
            PpuRegister::Bgp => self.bgp.set(val),
            PpuRegister::Obp0 => self.obp0.set(val),
            PpuRegister::Obp1 => self.obp1.set(val),
            PpuRegister::Bcps => self.cgb_bg_palette.set_color_palette_specification(val),
            PpuRegister::Bcpd => self.cgb_bg_palette.set_color_palette_data(val),
            PpuRegister::Ocps => self.cgb_sprite_palette.set_color_palette_specification(val),
            PpuRegister::Ocpd => self.cgb_sprite_palette.set_color_palette_data(val),
            PpuRegister::Opri => self.opri = val,
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
    pub const fn window_enabled(self, function_mode: FunctionMode) -> bool {
        match function_mode {
            FunctionMode::Monochrome | FunctionMode::Compatibility => {
                self.contains(Self::BACKGROUND_ENABLED) && self.contains(Self::WINDOW_ENABLED)
            }
            FunctionMode::Color => self.contains(Self::WINDOW_ENABLED),
        }
    }

    pub const fn background_enabled(self, function_mode: FunctionMode) -> bool {
        match function_mode {
            FunctionMode::Monochrome | FunctionMode::Compatibility => {
                self.contains(Self::BACKGROUND_ENABLED)
            }
            FunctionMode::Color => true,
        }
    }

    pub const fn cgb_sprite_master_priority_on(self, function_mode: FunctionMode) -> bool {
        match function_mode {
            FunctionMode::Monochrome | FunctionMode::Compatibility => false,
            FunctionMode::Color => !self.contains(Self::BACKGROUND_ENABLED),
        }
    }

    const fn signed_byte_for_tile_offset(self) -> bool {
        !self.contains(Self::BG_WINDOW_TILE_DATA_AREA)
    }

    pub const fn bg_tile_map_address(self) -> u16 {
        if self.contains(Self::BG_TILE_MAP_AREA) {
            0x9c00
        } else {
            0x9800
        }
    }

    pub const fn window_tile_map_address(self) -> u16 {
        if self.contains(Self::WINDOW_TILE_MAP_AREA) {
            0x9c00
        } else {
            0x9800
        }
    }

    const fn bg_window_tile_address(self) -> u16 {
        if self.contains(Self::BG_WINDOW_TILE_DATA_AREA) {
            0x8000
        } else {
            0x8800
        }
    }

    pub const fn tile_data_address(self, tile_number: u8) -> u16 {
        self.bg_window_tile_address()
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
