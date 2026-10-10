use crate::ppu::ColorCorrectionMode;

// DMG palette colors RGB
pub const GRAYSCALE_PALETTE: [(u8, u8, u8); 4] = [
    (0xFF, 0xFF, 0xFF),
    (0xAA, 0xAA, 0xAA),
    (0x55, 0x55, 0x55),
    (0x00, 0x00, 0x00),
];

// BCPS/OCPS bits
/// The index goes up after each write to the data register.
const SPEC_INCREMENT_B: u8 = 0x80;
/// The byte of palette RAM the data register accesses.
const SPEC_INDEX: u8 = 0x3F;

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
    pub fn init_compat_palette(&mut self) {
        // White, light gray, dark gray and black in every palette.
        const GRAYS: [u8; 12] = [31, 31, 31, 21, 21, 21, 10, 10, 10, 0, 0, 0];
        let (palettes, _) = self.buffer.as_chunks_mut::<{ GRAYS.len() }>();
        palettes.fill(GRAYS);
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

    #[must_use]
    const fn index(&self) -> u8 {
        self.spec & SPEC_INDEX
    }

    #[must_use]
    const fn is_increment_enabled(&self) -> bool {
        self.spec & SPEC_INCREMENT_B != 0
    }

    // For color correction values see: https://github.com/LIJI32/SameBoy/blob/master/Core/display.c#L355
    // TODO: should this be done on GPU?
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
                (new_r, new_g, new_b) =
                    squeeze_contrast((new_r, new_g, new_b), [(45, 162), (41, 167), (38, 157)]);
            }
            ColorCorrectionMode::ReduceContrast => {
                (new_r, new_g, new_b) =
                    squeeze_contrast((new_r, new_g, new_b), [(40, 220), (36, 224), (32, 216)]);
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

        self.auto_increment();
    }

    /// A data write the PPU's palette access blocking dropped still advances
    /// the index.
    pub const fn auto_increment(&mut self) {
        if self.is_increment_enabled() {
            self.spec = (self.spec & SPEC_INCREMENT_B) | (self.index() + 1) & SPEC_INDEX;
        }
    }

    pub const fn set_spec(&mut self, val: u8) {
        self.spec = val;
    }

    #[must_use]
    pub const fn spec(&self) -> u8 {
        // Bit 6 reads 1.
        self.spec | 0x40
    }
}

/// Mixes a little of the other two channels into each one, then squeezes it
/// into its `(low, high)` range.
#[expect(clippy::cast_possible_truncation)]
fn squeeze_contrast((r, g, b): (u8, u8, u8), ranges: [(u16, u16); 3]) -> (u8, u8, u8) {
    let (r, g, b) = (u16::from(r), u16::from(g), u16::from(b));
    let squeeze = |c: u16, (low, high): (u16, u16)| (c * (high - low) / 255 + low) as u8;
    (
        squeeze(r * 15 / 16 + (g + b) / 32, ranges[0]),
        squeeze(g * 15 / 16 + (r + b) / 32, ranges[1]),
        squeeze(b * 15 / 16 + (r + g) / 32, ranges[2]),
    )
}
