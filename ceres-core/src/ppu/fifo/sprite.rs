use crate::ppu::oam::Oam;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Sprite {
    pub y: u8,
    pub x: u8,
    pub tile: u8,
    pub flags: u8,
    pub oam_index: u8,
}

impl Sprite {
    #[must_use]
    pub const fn bg_priority(self) -> bool {
        self.flags & 0x80 != 0
    }

    #[must_use]
    pub const fn y_flip(self) -> bool {
        self.flags & 0x40 != 0
    }

    #[must_use]
    pub const fn x_flip(self) -> bool {
        self.flags & 0x20 != 0
    }

    #[must_use]
    pub const fn dmg_palette(self) -> u8 {
        (self.flags >> 4) & 1
    }

    #[must_use]
    pub const fn cgb_vram_bank(self) -> u8 {
        (self.flags >> 3) & 1
    }

    #[must_use]
    pub const fn cgb_palette(self) -> u8 {
        self.flags & 0x07
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SpriteBuffer {
    sprites: [Option<Sprite>; 10],
    count: usize,
}

impl SpriteBuffer {
    pub const fn new() -> Self {
        Self {
            sprites: [None; 10],
            count: 0,
        }
    }

    pub fn clear(&mut self) {
        self.sprites = [None; 10];
        self.count = 0;
    }

    #[must_use]
    pub const fn count(&self) -> usize {
        self.count
    }

    #[must_use]
    pub const fn sprites(&self) -> &[Option<Sprite>; 10] {
        &self.sprites
    }

    /// Scan OAM for scanline `ly` with sprite height (8 or 16).
    /// Extracts up to 10 matching sprites.
    pub fn scan_line(&mut self, oam: &Oam, ly: u8, sprite_height: u8, is_cgb: bool, opri: bool) {
        self.clear();
        let bytes = oam.bytes();

        for i in 0..40 {
            let offset = i * 4;
            let y = bytes[offset];
            let x = bytes[offset + 1];
            let tile = bytes[offset + 2];
            let flags = bytes[offset + 3];

            // Sprite intersects scanline if ly + 16 in [y, y + sprite_height)
            let ly_plus_16 = u16::from(ly) + 16;
            let y_u16 = u16::from(y);

            if ly_plus_16 >= y_u16 && ly_plus_16 < y_u16 + u16::from(sprite_height) {
                self.sprites[self.count] = Some(Sprite {
                    y,
                    x,
                    tile,
                    flags,
                    oam_index: i as u8,
                });
                self.count += 1;
                if self.count == 10 {
                    break;
                }
            }
        }

        // On DMG (or CGB with OPRI = false), sprites are sorted by X coordinate
        // (smaller X takes priority; ties preserved by OAM index order).
        // On CGB (OPRI = true or default CGB), priority is strictly OAM index.
        if !is_cgb || opri {
            // Stable sort by X coordinate
            self.sprites[..self.count].sort_by_key(|s| s.map_or(u8::MAX, |spr| spr.x));
        }
    }

    /// Check if any sprite in buffer starts at screen X coordinate `lx`
    #[must_use]
    pub fn find_sprite_at(&self, lx: u8) -> Option<Sprite> {
        let target_x = lx.wrapping_add(8);
        for s in self.sprites[..self.count].iter().flatten() {
            if s.x == target_x {
                return Some(*s);
            }
        }
        None
    }
}
