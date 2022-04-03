use crate::video::{ACCESS_OAM_CYCLES, ACCESS_VRAM_CYCLES, HBLANK_CYCLES, VBLANK_LINE_CYCLES};

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum Mode {
    OamScan,
    DrawingPixels,
    HBlank,
    VBlank,
}

impl Mode {
    pub const fn cycles(self, scroll_x: u8) -> i16 {
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
