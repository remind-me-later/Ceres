use crate::video::rgb_color::RgbColor;

const GRAYSCALE_PALETTE: [RgbColor; 4] = [
    RgbColor::new(0xff, 0xff, 0xff),
    RgbColor::new(0xcc, 0xcc, 0xcc),
    RgbColor::new(0x77, 0x77, 0x77),
    RgbColor::new(0x00, 0x00, 0x00),
];

const ORIGINAL_PALETTE: [RgbColor; 4] = [
    RgbColor::new(0x9b, 0xbc, 0x0f),
    RgbColor::new(0x8b, 0xac, 0x0f),
    RgbColor::new(0x30, 0x62, 0x30),
    RgbColor::new(0x0f, 0x38, 0x0f),
];

#[derive(Clone, Copy)]
pub enum MonochromePaletteColors {
    Grayscale,
    Original,
}

impl MonochromePaletteColors {
    pub fn get_color(self, index: u8) -> RgbColor {
        use MonochromePaletteColors::*;

        match self {
            Grayscale => GRAYSCALE_PALETTE[index as usize],
            Original => ORIGINAL_PALETTE[index as usize],
        }
    }
}

#[derive(Clone, Copy)]
pub struct MonochromePalette {
    palette: u8,
}

impl MonochromePalette {
    pub const fn new(value: u8) -> Self {
        Self { palette: value }
    }

    pub fn set(&mut self, value: u8) {
        self.palette = value;
    }

    pub const fn get(self) -> u8 {
        self.palette
    }

    pub fn shade_index(self, color_index: u8) -> u8 {
        (self.palette >> (color_index * 2)) & 0x3
    }
}
