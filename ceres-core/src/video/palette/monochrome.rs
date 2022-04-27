use crate::video::rgb_color::Color;

const GRAYSCALE_PALETTE: [Color; 4] = [
    Color::rgb(0xff, 0xff, 0xff),
    Color::rgb(0xcc, 0xcc, 0xcc),
    Color::rgb(0x77, 0x77, 0x77),
    Color::rgb(0x00, 0x00, 0x00),
];

const ORIGINAL_PALETTE: [Color; 4] = [
    Color::rgb(0x9b, 0xbc, 0x0f),
    Color::rgb(0x8b, 0xac, 0x0f),
    Color::rgb(0x30, 0x62, 0x30),
    Color::rgb(0x0f, 0x38, 0x0f),
];

#[derive(Clone, Copy)]
pub enum MonochromePaletteColors {
    Grayscale,
    Original,
}

impl MonochromePaletteColors {
    pub fn get_color(self, index: u8) -> Color {
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
    pub fn new(value: u8) -> Self {
        Self { palette: value }
    }

    pub fn set(&mut self, value: u8) {
        self.palette = value;
    }

    pub fn get(self) -> u8 {
        self.palette
    }

    pub fn shade_index(self, color_index: u8) -> u8 {
        (self.palette >> (color_index * 2)) & 0x3
    }
}
