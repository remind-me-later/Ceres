//! Sprite data structures for cycle-accurate PPU.

/// Sprite entry collected during OAM scan (Mode 2).
///
/// Up to 10 sprites can be visible per scanline. This struct stores the
/// relevant data from OAM for sprites that intersect the current scanline.
#[derive(Clone, Copy, Default)]
pub struct SpriteEntry {
    /// Y position from OAM (actual screen Y = y - 16).
    pub y: u8,
    /// X position from OAM (actual screen X = x - 8).
    pub x: u8,
    /// Tile index.
    pub tile: u8,
    /// Sprite attributes/flags.
    ///
    /// - Bit 7: BG/Window priority (0=Above, 1=Behind non-zero BG)
    /// - Bit 6: Y flip
    /// - Bit 5: X flip
    /// - Bit 4: DMG palette (0=OBP0, 1=OBP1)
    /// - Bit 3: CGB VRAM bank
    /// - Bits 0-2: CGB palette
    pub flags: u8,
    /// Original OAM index (0-39), used for sprite priority.
    pub oam_index: u8,
}

impl SpriteEntry {
    /// Returns true if sprite has BG/Window priority (appears behind non-zero BG pixels).
    #[inline]
    #[must_use]
    pub const fn bg_priority(&self) -> bool {
        self.flags & 0x80 != 0
    }

    /// Returns true if sprite is Y-flipped.
    #[inline]
    #[must_use]
    pub const fn y_flip(&self) -> bool {
        self.flags & 0x40 != 0
    }

    /// Returns true if sprite is X-flipped.
    #[inline]
    #[must_use]
    pub const fn x_flip(&self) -> bool {
        self.flags & 0x20 != 0
    }

    /// Returns DMG palette (0=OBP0, 1=OBP1).
    #[inline]
    #[must_use]
    pub const fn dmg_palette(&self) -> u8 {
        (self.flags >> 4) & 1
    }

    /// Returns CGB VRAM bank (0 or 1).
    #[inline]
    #[must_use]
    pub const fn cgb_vram_bank(&self) -> u8 {
        (self.flags >> 3) & 1
    }

    /// Returns CGB palette (0-7).
    #[inline]
    #[must_use]
    pub const fn cgb_palette(&self) -> u8 {
        self.flags & 7
    }
}

/// Container for sprites visible on the current scanline.
#[derive(Clone, Copy, Default)]
pub struct SpriteBuffer {
    /// Sprites collected during OAM scan, sorted by X position (DMG) or OAM index (CGB).
    pub sprites: [SpriteEntry; 10],
    /// Number of sprites in the buffer (0-10).
    pub count: u8,
}

impl SpriteBuffer {
    /// Clears the sprite buffer.
    #[inline]
    pub fn clear(&mut self) {
        self.count = 0;
    }

    /// Adds a sprite to the buffer if not full.
    ///
    /// Returns true if the sprite was added, false if buffer is full.
    pub fn add(&mut self, sprite: SpriteEntry) -> bool {
        if self.count >= 10 {
            return false;
        }
        self.sprites[self.count as usize] = sprite;
        self.count += 1;
        true
    }
}
