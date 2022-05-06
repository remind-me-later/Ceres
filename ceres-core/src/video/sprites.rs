pub const SPR_CGB_PAL: u8 = 0x7; // CGB Only
pub const SPR_TILE_BANK: u8 = 0x8; // CGB Only
pub const SPR_PAL: u8 = 0x10; // Non CGB Only
pub const SPR_FLIP_X: u8 = 0x20;
pub const SPR_FLIP_Y: u8 = 0x40;
pub const SPR_BG_WIN_OVER_OBJ: u8 = 0x80;

pub struct SpriteAttr {
    x: u8,
    y: u8,
    tile_index: u8,
    flags: u8,
}

impl SpriteAttr {
    pub fn x(&self) -> u8 {
        self.x
    }

    pub fn y(&self) -> u8 {
        self.y
    }

    pub fn tile_index(&self) -> u8 {
        self.tile_index
    }

    pub fn flags(&self) -> u8 {
        self.flags
    }

    pub fn cgb_palette(&self) -> u8 {
        self.flags() & SPR_CGB_PAL
    }
}

pub struct SpriteAttrIter<'a> {
    oam: &'a Oam,
    index: usize,
}

impl<'a> Iterator for SpriteAttrIter<'a> {
    type Item = SpriteAttr;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= 0x100 {
            return None;
        }

        let sprite = SpriteAttr {
            y: self.oam.data[self.index].wrapping_sub(16),
            x: self.oam.data[self.index + 1].wrapping_sub(8),
            tile_index: self.oam.data[self.index + 2],
            flags: self.oam.data[self.index + 3],
        };

        self.index += 4;

        Some(sprite)
    }
}

pub struct Oam {
    data: [u8; 0x100],
}

impl Oam {
    pub fn new() -> Self {
        Self { data: [0; 0x100] }
    }

    pub fn read(&self, addr: u8) -> u8 {
        self.data[addr as usize]
    }

    pub fn write(&mut self, addr: u8, val: u8) {
        self.data[addr as usize] = val;
    }

    pub fn sprite_attributes_iterator(&self) -> SpriteAttrIter {
        SpriteAttrIter {
            oam: self,
            index: 0,
        }
    }
}
