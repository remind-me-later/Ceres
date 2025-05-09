// DMG palette colors RGB
pub const GRAYSCALE_PALETTE: [(u8, u8, u8); 4] = [
    (0xFF, 0xFF, 0xFF),
    (0xCC, 0xCC, 0xCC),
    (0x77, 0x77, 0x77),
    (0x00, 0x00, 0x00),
];

// CGB palette RAM
const PAL_RAM_SIZE: u8 = 0x20;
const PAL_RAM_SIZE_COLORS: u8 = PAL_RAM_SIZE * 3;

#[derive(Debug)]
pub struct ColorPalette {
    // Rgb color ram
    buffer: [u8; PAL_RAM_SIZE_COLORS as usize],
    spec: u8,
}

impl Default for ColorPalette {
    fn default() -> Self {
        Self {
            buffer: [0; PAL_RAM_SIZE_COLORS as usize],
            spec: 0,
        }
    }
}

impl ColorPalette {
    pub const fn set_spec(&mut self, val: u8) {
        self.spec = val;
    }

    #[must_use]
    pub const fn spec(&self) -> u8 {
        self.spec | 0x40
    }

    #[must_use]
    const fn index(&self) -> u8 {
        self.spec & 0x3F
    }

    #[must_use]
    const fn is_increment_enabled(&self) -> bool {
        self.spec & 0x80 != 0
    }

    #[must_use]
    pub const fn data(&self) -> u8 {
        let i = (self.index() as usize / 2) * 3;

        if self.index() & 1 == 0 {
            // red and green
            let r = self.buffer[i];
            let g = self.buffer[i + 1] << 5;
            r | g
        } else {
            // green and blue
            let g = self.buffer[i + 1] >> 3;
            let b = self.buffer[i + 2] << 2;
            g | b
        }
    }

    pub const fn set_data(&mut self, val: u8) {
        let i = (self.index() as usize / 2) * 3;

        if self.index() & 1 == 0 {
            // red
            self.buffer[i] = val & 0x1F;
            // green
            let tmp = (self.buffer[i + 1] & 3) << 3;
            self.buffer[i + 1] = tmp | ((val & 0xE0) >> 5);
        } else {
            // green
            let tmp = self.buffer[i + 1] & 7;
            self.buffer[i + 1] = tmp | ((val & 3) << 3);
            // blue
            self.buffer[i + 2] = (val & 0x7C) >> 2;
        }

        if self.is_increment_enabled() {
            self.spec = (self.spec & 0x80) | (self.index() + 1) & 0x3F;
        }
    }

    #[must_use]
    pub const fn rgb(&self, palette: u8, color: u8) -> (u8, u8, u8) {
        const fn scale_channel(c: u8) -> u8 {
            (c << 3) | (c >> 2)
        }

        let i = (palette as usize * 4 + color as usize) * 3;
        let r = self.buffer[i];
        let g = self.buffer[i + 1];
        let b = self.buffer[i + 2];

        (scale_channel(r), scale_channel(g), scale_channel(b))
    }
}
