// CGB palette RAM
const PAL_RAM_SIZE: u8 = 0x20;
const PAL_RAM_SIZE_COLORS: u8 = PAL_RAM_SIZE * 3;

pub struct ColorPalette {
    // Rgb color ram
    col: [u8; PAL_RAM_SIZE_COLORS as usize],
    idx: u8,
    inc: bool, // increment after write
}

impl Default for ColorPalette {
    fn default() -> Self {
        Self {
            col: [0; PAL_RAM_SIZE_COLORS as usize],
            idx: 0,
            inc: false,
        }
    }
}

impl ColorPalette {
    pub(crate) fn set_spec(&mut self, val: u8) {
        self.idx = val & 0x3F;
        self.inc = val & 0x80 != 0;
    }

    pub(crate) const fn spec(&self) -> u8 {
        self.idx | 0x40 | ((self.inc as u8) << 7)
    }

    pub(crate) const fn data(&self) -> u8 {
        let i = (self.idx as usize / 2) * 3;

        if self.idx & 1 == 0 {
            // red and green
            let r = self.col[i];
            let g = self.col[i + 1] << 5;
            r | g
        } else {
            // green and blue
            let g = self.col[i + 1] >> 3;
            let b = self.col[i + 2] << 2;
            g | b
        }
    }

    pub(crate) fn set_data(&mut self, val: u8) {
        let i = (self.idx as usize / 2) * 3;

        if self.idx & 1 == 0 {
            // red
            self.col[i] = val & 0x1F;
            // green
            let tmp = (self.col[i + 1] & 3) << 3;
            self.col[i + 1] = tmp | (val & 0xE0) >> 5;
        } else {
            // green
            let tmp = self.col[i + 1] & 7;
            self.col[i + 1] = tmp | (val & 3) << 3;
            // blue
            self.col[i + 2] = (val & 0x7C) >> 2;
        }

        if self.inc {
            self.idx = (self.idx + 1) & 0x3F;
        }
    }

    pub(super) const fn rgb(&self, palette: u8, color: u8) -> (u8, u8, u8) {
        const fn scale_channel(c: u8) -> u8 {
            (c << 3) | (c >> 2)
        }

        let i = (palette as usize * 4 + color as usize) * 3;
        let r = self.col[i];
        let g = self.col[i + 1];
        let b = self.col[i + 2];

        (scale_channel(r), scale_channel(g), scale_channel(b))
    }
}
