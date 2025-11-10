use crate::ppu::ColorCorrectionMode;

// DMG palette colors RGB
pub const GRAYSCALE_PALETTE: [(u8, u8, u8); 4] = [
    (0xFF, 0xFF, 0xFF),
    (0xAA, 0xAA, 0xAA),
    (0x55, 0x55, 0x55),
    (0x00, 0x00, 0x00),
];

// CGB palette RAM
const PAL_RAM_SIZE: u8 = 0x20;
const PAL_RAM_SIZE_COLORS: u8 = PAL_RAM_SIZE * 3;

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

    #[must_use]
    const fn index(&self) -> u8 {
        self.spec & 0x3F
    }

    #[must_use]
    const fn is_increment_enabled(&self) -> bool {
        self.spec & 0x80 != 0
    }

    // For color correction values see: https://github.com/LIJI32/SameBoy/blob/master/Core/display.c#L355
    // TODO: should this be done on GPU?
    #[expect(clippy::too_many_lines)]
    #[must_use]
    pub fn rgb(
        &self,
        palette: u8,
        color: u8,
        color_correction_mode: ColorCorrectionMode,
    ) -> (u8, u8, u8) {
        const SCALE_CHANNEL_WITH_CURVE: [u8; 32] = [
            0, 6, 12, 20, 28, 36, 45, 56, 66, 76, 88, 100, 113, 125, 137, 149, 161, 172, 182, 192,
            202, 210, 218, 225, 232, 238, 243, 247, 250, 252, 254, 255,
        ];

        const fn scale_channel(c: u8) -> u8 {
            (c << 3) | (c >> 2)
        }

        let i = (palette as usize * 4 + color as usize) * 3;
        let mut r = self.buffer[i];
        let mut g = self.buffer[i + 1];
        let mut b = self.buffer[i + 2];

        if matches!(color_correction_mode, ColorCorrectionMode::Disabled) {
            return (scale_channel(r), scale_channel(g), scale_channel(b));
        }

        (r, g, b) = (
            SCALE_CHANNEL_WITH_CURVE[r as usize],
            SCALE_CHANNEL_WITH_CURVE[g as usize],
            SCALE_CHANNEL_WITH_CURVE[b as usize],
        );

        if matches!(color_correction_mode, ColorCorrectionMode::CorrectCurves) {
            return (r, g, b);
        }

        let (mut new_r, mut new_g, mut new_b) = (r, g, b);

        if g != b {
            let gamma = if matches!(
                color_correction_mode,
                ColorCorrectionMode::ReduceContrast | ColorCorrectionMode::LowContrast
            ) {
                2.2
            } else {
                1.6
            };

            #[expect(
                clippy::float_arithmetic,
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss
            )]
            {
                new_g = (((f32::from(g) / 255.0)
                    .powf(gamma)
                    .mul_add(3.0, (f32::from(b) / 255.0).powf(gamma))
                    / 4.0)
                    .powf(1.0 / gamma)
                    * 255.0)
                    .round() as u8;
            }
        }

        match color_correction_mode {
            ColorCorrectionMode::LowContrast => {
                (r, g, b) = (new_r, new_g, new_b);

                #[expect(clippy::cast_possible_truncation)]
                {
                    new_r = (u16::from(new_r) * 15 / 16 + (u16::from(g) + u16::from(b)) / 32) as u8;
                    new_g = (u16::from(new_g) * 15 / 16 + (u16::from(r) + u16::from(b)) / 32) as u8;
                    new_b = (u16::from(new_b) * 15 / 16 + (u16::from(r) + u16::from(g)) / 32) as u8;

                    new_r = (u16::from(new_r) * (162 - 45) / 255 + 45) as u8;
                    new_g = (u16::from(new_g) * (167 - 41) / 255 + 41) as u8;
                    new_b = (u16::from(new_b) * (157 - 38) / 255 + 38) as u8;
                }
            }
            ColorCorrectionMode::ReduceContrast => {
                (r, g, b) = (new_r, new_g, new_b);

                #[expect(clippy::cast_possible_truncation)]
                {
                    new_r = (u16::from(new_r) * 15 / 16 + (u16::from(g) + u16::from(b)) / 32) as u8;
                    new_g = (u16::from(new_g) * 15 / 16 + (u16::from(r) + u16::from(b)) / 32) as u8;
                    new_b = (u16::from(new_b) * 15 / 16 + (u16::from(r) + u16::from(g)) / 32) as u8;

                    new_r = (u16::from(new_r) * (220 - 40) / 255 + 40) as u8;
                    new_g = (u16::from(new_g) * (224 - 36) / 255 + 36) as u8;
                    new_b = (u16::from(new_b) * (216 - 32) / 255 + 32) as u8;
                }
            }
            ColorCorrectionMode::ModernBoostContrast => {
                let old_max = r.max(g.max(b));
                let new_max = new_r.max(new_g.max(new_b));

                #[expect(clippy::cast_possible_truncation)]
                if new_max != 0 {
                    new_r = (u16::from(new_r) * u16::from(old_max) / u16::from(new_max)) as u8;
                    new_g = (u16::from(new_g) * u16::from(old_max) / u16::from(new_max)) as u8;
                    new_b = (u16::from(new_b) * u16::from(old_max) / u16::from(new_max)) as u8;
                }

                let old_min = r.min(g.min(b));
                let new_min = new_r.min(new_g.min(new_b));

                #[expect(clippy::cast_possible_truncation)]
                if new_min != 0xFF {
                    new_r = 0xFF
                        - ((0xFF - u16::from(new_r)) * (0xFF - u16::from(old_min))
                            / (0xFF - u16::from(new_min))) as u8;
                    new_g = 0xFF
                        - ((0xFF - u16::from(new_g)) * (0xFF - u16::from(old_min))
                            / (0xFF - u16::from(new_min))) as u8;
                    new_b = 0xFF
                        - ((0xFF - u16::from(new_b)) * (0xFF - u16::from(old_min))
                            / (0xFF - u16::from(new_min))) as u8;
                }
            }
            ColorCorrectionMode::ModernBalanced
            | ColorCorrectionMode::Disabled
            | ColorCorrectionMode::CorrectCurves => {}
        }

        (new_r, new_g, new_b)
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

    pub const fn set_spec(&mut self, val: u8) {
        self.spec = val;
    }

    #[must_use]
    pub const fn spec(&self) -> u8 {
        self.spec | 0x40
    }
}
