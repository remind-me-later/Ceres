#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Pixel {
    /// 2-bit color index (0..=3)
    pub color_id: u8,
    /// Palette index (0..=7 for CGB, or 0/1 for DMG OBP0/OBP1, 0 for BGP)
    pub palette: u8,
    /// Background priority bit (from CGB tilemap attribute bit 7 or OAM flag bit 7)
    pub bg_priority: bool,
    /// Sprite priority (for ordering on DMG X-pos or CGB OAM index)
    pub sprite_priority: u8,
}

impl Pixel {
    pub const fn empty() -> Self {
        Self {
            color_id: 0,
            palette: 0,
            bg_priority: false,
            sprite_priority: u8::MAX,
        }
    }
}
