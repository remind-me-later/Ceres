#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Pixel {
    /// 2-bit color index (0..=3)
    color_id: u8,
    /// Palette index (0..=7 for CGB, or 0/1 for DMG OBP0/OBP1, 0 for BGP)
    palette: u8,
    /// Background priority bit (from CGB tilemap attribute bit 7 or OAM flag bit 7)
    bg_priority: bool,
    /// Sprite priority (for ordering on DMG X-pos or CGB OAM index)
    sprite_priority: u8,
}

impl Pixel {
    #[must_use]
    pub const fn new(
        color_id: u8,
        palette: u8,
        bg_priority: bool,
        sprite_priority: u8,
    ) -> Self {
        Self {
            color_id,
            palette,
            bg_priority,
            sprite_priority,
        }
    }

    #[must_use]
    pub const fn empty() -> Self {
        Self {
            color_id: 0,
            palette: 0,
            bg_priority: false,
            sprite_priority: u8::MAX,
        }
    }

    #[must_use]
    pub const fn color_id(&self) -> u8 {
        self.color_id
    }

    #[must_use]
    pub const fn palette(&self) -> u8 {
        self.palette
    }

    #[must_use]
    pub const fn bg_priority(&self) -> bool {
        self.bg_priority
    }

    #[must_use]
    pub const fn sprite_priority(&self) -> u8 {
        self.sprite_priority
    }
}
