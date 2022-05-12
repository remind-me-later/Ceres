pub const OAM_SIZE: usize = 0x100;

pub const SPR_CGB_PAL: u8 = 0x7;
pub const SPR_TILE_BANK: u8 = 0x8;
pub const SPR_PAL: u8 = 0x10;
pub const SPR_FLIP_X: u8 = 0x20;
pub const SPR_FLIP_Y: u8 = 0x40;
pub const SPR_BG_WIN_OVER_OBJ: u8 = 0x80;

#[derive(Default)]
pub struct SpriteAttr {
    pub x: u8,
    pub y: u8,
    pub tile_index: u8,
    pub flags: u8,
}

impl SpriteAttr {
    pub fn cgb_palette(&self) -> u8 {
        self.flags & SPR_CGB_PAL
    }
}

pub struct Oam {
    pub(crate) data: [u8; OAM_SIZE],
}

impl Oam {
    pub fn new() -> Self {
        Self {
            data: [0; OAM_SIZE],
        }
    }

    pub fn read(&self, addr: u8) -> u8 {
        self.data[addr as usize]
    }

    pub fn write(&mut self, addr: u8, val: u8) {
        self.data[addr as usize] = val;
    }
}
