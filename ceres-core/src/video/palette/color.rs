use crate::video::rgb_color::RgbColor;

pub struct ColorPalette {
    palette: [RgbColor; 32],
    index: u8,
    auto_increment: bool,
}

impl ColorPalette {
    pub fn new() -> Self {
        Self {
            palette: [RgbColor::default(); 32],
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
            self.palette[color_index].r | (self.palette[color_index].g << 5)
        } else {
            (self.palette[color_index].g >> 3) | (self.palette[color_index].b << 2)
        }
    }

    pub fn set_color_palette_data(&mut self, value: u8) {
        let color_index = (self.index / 2) as usize;

        if self.index % 2 == 0 {
            self.palette[color_index].r = value & 0b11111;
            self.palette[color_index].g =
                ((self.palette[color_index].g & 0b11) << 3) | ((value & 0xe0) >> 5);
        } else {
            self.palette[color_index].g =
                (self.palette[color_index].g & 0b111) | ((value & 0b11) << 3);
            self.palette[color_index].b = (value & 0x7c) >> 2;
        }

        if self.auto_increment {
            self.index = (self.index + 1) & 0b0011_1111;
        }
    }

    pub fn get_color(&self, palette_number: u8, color_number: u8) -> RgbColor {
        let index = palette_number as usize * 4 + color_number as usize;
        let r = self.palette[index].r;
        let g = self.palette[index].g;
        let b = self.palette[index].b;
        RgbColor::new(scale_channel(r), scale_channel(g), scale_channel(b))
    }
}

fn scale_channel(c: u8) -> u8 {
    (c << 3) | (c >> 2)
}
