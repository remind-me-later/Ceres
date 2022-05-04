use bitflags::bitflags;

bitflags! {
  pub struct SpriteFlags: u8 {
    const CGB_PALETTE     = 0b_0000_0111; // CGB Only
    const TILE_VRAM_BANK  = 0b_0000_1000; // CGB Only
    const NON_CGB_PALETTE = 0b_0001_0000; // Non CGB Only
    const FLIP_X          = 0b_0010_0000;
    const FLIP_Y          = 0b_0100_0000;
    const BG_WIN_OVER_OBJ = 0b_1000_0000;
  }
}

pub struct SpriteAttributes {
    x: u8,
    y: u8,
    tile_index: u8,
    flags: SpriteFlags,
}

impl SpriteAttributes {
    pub fn x(&self) -> u8 {
        self.x
    }

    pub fn y(&self) -> u8 {
        self.y
    }

    pub fn tile_index(&self) -> u8 {
        self.tile_index
    }

    pub fn flags(&self) -> &SpriteFlags {
        &self.flags
    }

    pub fn cgb_palette(&self) -> u8 {
        self.flags().bits() & 0x7
    }
}

pub struct SpriteAttributesIterator<'a> {
    oam: &'a Oam,
    index: usize,
}

impl<'a> Iterator for SpriteAttributesIterator<'a> {
    type Item = SpriteAttributes;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= 0x100 {
            return None;
        }

        let sprite = SpriteAttributes {
            y: self.oam.data[self.index].wrapping_sub(16),
            x: self.oam.data[self.index + 1].wrapping_sub(8),
            tile_index: self.oam.data[self.index + 2],
            flags: SpriteFlags::from_bits_truncate(self.oam.data[self.index + 3]),
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

    pub fn read(&self, address: u8) -> u8 {
        self.data[address as usize]
    }

    pub fn write(&mut self, address: u8, val: u8) {
        self.data[address as usize] = val;
    }

    pub fn sprite_attributes_iterator(&self) -> SpriteAttributesIterator {
        SpriteAttributesIterator {
            oam: self,
            index: 0,
        }
    }
}
