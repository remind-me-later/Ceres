use super::Color;

const GRAYSCALE_PALETTE: [Color; 4] = [
    Color::new(0xff, 0xff, 0xff),
    Color::new(0xcc, 0xcc, 0xcc),
    Color::new(0x77, 0x77, 0x77),
    Color::new(0x00, 0x00, 0x00),
];

const ORIGINAL_PALETTE: [Color; 4] = [
    Color::new(0x9b, 0xbc, 0x0f),
    Color::new(0x8b, 0xac, 0x0f),
    Color::new(0x30, 0x62, 0x30),
    Color::new(0x0f, 0x38, 0x0f),
];

#[derive(Clone, Copy)]
pub enum MonochromePaletteColors {
    Grayscale,
    Original,
}

impl MonochromePaletteColors {
    pub fn get_color(self, index: MonochromeColorIndex) -> Color {
        use MonochromePaletteColors::*;
        let idx: u8 = index.into();

        match self {
            Grayscale => GRAYSCALE_PALETTE[idx as usize],
            Original => ORIGINAL_PALETTE[idx as usize],
        }
    }
}

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum MonochromeColorIndex {
    Off,
    Light,
    Dark,
    On,
}

impl From<u8> for MonochromeColorIndex {
    fn from(val: u8) -> Self {
        use MonochromeColorIndex::{Dark, Light, Off, On};
        match val {
            1 => Light,
            2 => Dark,
            3 => On,
            0 => Off,
            _ => unreachable!("Index out of bounds"),
        }
    }
}

impl From<MonochromeColorIndex> for u8 {
    fn from(val: MonochromeColorIndex) -> Self {
        match val {
            MonochromeColorIndex::Off => 0,
            MonochromeColorIndex::Light => 1,
            MonochromeColorIndex::Dark => 2,
            MonochromeColorIndex::On => 3,
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

    pub fn color_index(self, idx: MonochromeColorIndex) -> MonochromeColorIndex {
        let idx: u8 = idx.into();
        MonochromeColorIndex::from((self.palette >> (idx * 2)) & 0x3)
    }
}

pub struct ColorPalette {
    palette: [Color; 32],
    index: u8,
    auto_increment: bool,
}

impl ColorPalette {
    pub fn new() -> Self {
        Self {
            palette: [Color::default(); 32],
            index: 0,
            auto_increment: false,
        }
    }

    pub fn set_color_palette_specification(&mut self, value: u8) {
        self.index = value & 0x3f;
        self.auto_increment = value & 0x80 != 0;
    }

    pub const fn color_palette_specification(&self) -> u8 {
        if self.auto_increment {
            self.index | 0x80 | 0x40
        } else {
            self.index | 0x40
        }
    }

    pub const fn color_palette_data(&self) -> u8 {
        let color_index = (self.index / 2) as usize;

        if self.index % 2 == 0 {
            self.palette[color_index].red | (self.palette[color_index].green << 5)
        } else {
            (self.palette[color_index].green >> 3) | (self.palette[color_index].blue << 2)
        }
    }

    pub fn set_color_palette_data(&mut self, value: u8) {
        let color_index = (self.index / 2) as usize;

        if self.index % 2 == 0 {
            self.palette[color_index].red = value & 0b11111;
            self.palette[color_index].green =
                ((self.palette[color_index].green & 0b11) << 3) | ((value & 0xe0) >> 5);
        } else {
            self.palette[color_index].green =
                (self.palette[color_index].green & 0b111) | ((value & 0b11) << 3);
            self.palette[color_index].blue = (value & 0x7c) >> 2;
        }

        if self.auto_increment {
            self.index = (self.index + 1) & 0b0011_1111;
        }
    }

    pub const fn get_color(&self, palette_number: u8, color_number: u8) -> Color {
        let r = self.palette[palette_number as usize * 4 + color_number as usize].red;
        let g = self.palette[palette_number as usize * 4 + color_number as usize].green;
        let b = self.palette[palette_number as usize * 4 + color_number as usize].blue;
        Color::new(r << 3 | r >> 2, g << 3 | g >> 2, b << 3 | b >> 2)
    }
}
