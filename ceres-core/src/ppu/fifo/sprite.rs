use crate::ppu::oam::Oam;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Sprite {
    y: u8,
    x: u8,
    tile: u8,
    flags: u8,
    oam_index: u8,
}

impl Sprite {
    #[must_use]
    pub const fn new(y: u8, x: u8, tile: u8, flags: u8, oam_index: u8) -> Self {
        Self {
            y,
            x,
            tile,
            flags,
            oam_index,
        }
    }

    #[must_use]
    pub const fn y(&self) -> u8 {
        self.y
    }

    #[must_use]
    pub const fn x(&self) -> u8 {
        self.x
    }

    #[must_use]
    pub const fn tile(&self) -> u8 {
        self.tile
    }

    #[must_use]
    pub const fn oam_index(&self) -> u8 {
        self.oam_index
    }

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

    /// Scan OAM for scanline `ly` with sprite height (8 or 16).
    /// Extracts up to 10 matching sprites.
    pub fn scan_line(&mut self, oam: &Oam, ly: u8, sprite_height: u8) {
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
                self.sprites[self.count] = Some(Sprite::new(y, x, tile, flags, i as u8));
                self.count += 1;
                if self.count == 10 {
                    break;
                }
            }
        }

        // Fetch order is always ascending X (stable sort, so equal X keeps
        // OAM order). Pixel priority between overlapping sprites is not
        // decided here: the FIFO overlay resolves it — first opaque pixel
        // in fetch order on DMG (== X priority, Pan Docs "Drawing
        // priority"), lowest OAM index on CGB regardless of fetch order.
        self.sprites[..self.count].sort_by_key(|s| s.map_or(u8::MAX, |spr| spr.x()));
    }

    /// Effective match X for a sprite's OAM X coordinate: sprites at or
    /// left of the screen edge all match once pixel output begins, since
    /// their fetch point lies in the line's bootstrap phase.
    const fn effective_x(x: u8) -> u8 {
        if x < 8 { 8 } else { x }
    }

    /// Drop sprites that can no longer be fetched this line: their match
    /// point is behind the PPU's current X position. Mirrors SameBoy
    /// popping objects with `objects_x < x_for_object_match()`.
    pub fn discard_behind(&mut self, match_x: u8) {
        while self.count > 0 {
            let front = self.sprites[0];
            let Some(spr) = front else { break };
            if Self::effective_x(spr.x()) < match_x {
                self.sprites.copy_within(1.., 0);
                self.sprites[self.count - 1] = None;
                self.count -= 1;
            } else {
                break;
            }
        }
    }

    /// X coordinate the next sprite (highest fetch priority) matches at.
    #[must_use]
    pub fn next_x(&self) -> Option<u8> {
        self.sprites[0].map(|spr| Self::effective_x(spr.x()))
    }

    /// Consume the next sprite (highest fetch priority).
    pub fn pop_next(&mut self) -> Option<Sprite> {
        if self.count == 0 {
            return None;
        }
        let spr = self.sprites[0];
        self.sprites.copy_within(1.., 0);
        self.sprites[self.count - 1] = None;
        self.count -= 1;
        spr
    }
}
