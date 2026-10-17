use {crate::ppu::ColorCorrectionMode, core::array};

/// A colour's red, green and blue: 8-bit channels, or anything else kept per
/// channel.
#[derive(Clone, Copy)]
pub(crate) struct Rgb<T = u8> {
    pub r: T,
    pub g: T,
    pub b: T,
}

impl Rgb {
    pub(super) const BLACK: Self = Self::gray(0x00);

    const fn gray(level: u8) -> Self {
        Self {
            r: level,
            g: level,
            b: level,
        }
    }
}

// DMG palette colors RGB
pub(super) const GRAYSCALE_PALETTE: [Rgb; 4] = [
    Rgb::gray(0xFF),
    Rgb::gray(0xAA),
    Rgb::gray(0x55),
    Rgb::BLACK,
];

/// The range a channel is squeezed into by the reduced contrast modes.
#[derive(Clone, Copy)]
struct ChannelRange {
    low: u16,
    high: u16,
}

impl ChannelRange {
    #[expect(clippy::cast_possible_truncation, reason = "c is at most 255")]
    const fn squeeze(self, c: u16) -> u8 {
        (c * (self.high - self.low) / 255 + self.low) as u8
    }
}

const LOW_CONTRAST: Rgb<ChannelRange> = Rgb {
    r: ChannelRange { low: 45, high: 162 },
    g: ChannelRange { low: 41, high: 167 },
    b: ChannelRange { low: 38, high: 157 },
};

const REDUCED_CONTRAST: Rgb<ChannelRange> = Rgb {
    r: ChannelRange { low: 40, high: 220 },
    g: ChannelRange { low: 36, high: 224 },
    b: ChannelRange { low: 32, high: 216 },
};

// BCPS/OCPS bits
/// The index goes up after each write to the data register.
const SPEC_INCREMENT_B: u8 = 0x80;
/// The byte of palette RAM the data register accesses.
const SPEC_INDEX: u8 = 0x3F;

// CGB palette RAM
const PAL_RAM_SIZE: u8 = 0x20;
const PAL_RAM_SIZE_COLORS: u8 = PAL_RAM_SIZE * 3;

pub(crate) struct ColorPalette {
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
    pub(crate) fn init_compat_palette(&mut self) {
        // White, light gray, dark gray and black in every palette.
        const GRAYS: [u8; 12] = [31, 31, 31, 21, 21, 21, 10, 10, 10, 0, 0, 0];
        let (palettes, _) = self.buffer.as_chunks_mut::<{ GRAYS.len() }>();
        palettes.fill(GRAYS);
    }
    /// The palette RAM as the CPU sees it: 32 colors, 2 bytes each.
    pub(crate) const RAW_SIZE: usize = PAL_RAM_SIZE as usize * 2;

    /// The palette RAM as the CPU sees it.
    #[must_use]
    pub(crate) fn raw(&self) -> [u8; Self::RAW_SIZE] {
        array::from_fn(|i| {
            #[expect(clippy::cast_possible_truncation, reason = "i < 64")]
            self.byte(i as u8)
        })
    }

    /// Loads the palette RAM as the CPU sees it (a state load).
    pub(crate) fn set_raw(&mut self, raw: &[u8; Self::RAW_SIZE]) {
        for (i, &val) in raw.iter().enumerate() {
            #[expect(clippy::cast_possible_truncation, reason = "i < 64")]
            self.set_byte(i as u8, val);
        }
    }

    #[must_use]
    pub(crate) const fn data(&self) -> u8 {
        self.byte(self.index())
    }

    /// The byte at `index` of the palette RAM.
    #[must_use]
    const fn byte(&self, index: u8) -> u8 {
        let i = (index as usize / 2) * 3;

        if index & 1 == 0 {
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
    pub(crate) fn rgb(
        &self,
        palette: u8,
        color: u8,
        color_correction_mode: ColorCorrectionMode,
    ) -> Rgb {
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
            return Rgb {
                r: scale_channel(r),
                g: scale_channel(g),
                b: scale_channel(b),
            };
        }

        (r, g, b) = (
            SCALE_CHANNEL_WITH_CURVE[r as usize],
            SCALE_CHANNEL_WITH_CURVE[g as usize],
            SCALE_CHANNEL_WITH_CURVE[b as usize],
        );

        if matches!(color_correction_mode, ColorCorrectionMode::CorrectCurves) {
            return Rgb { r, g, b };
        }

        // Green is mixed with a little blue.
        let mut corrected = Rgb { r, g, b };
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
                corrected.g = (((f32::from(g) / 255.0)
                    .powf(gamma)
                    .mul_add(3.0, (f32::from(b) / 255.0).powf(gamma))
                    / 4.0)
                    .powf(1.0 / gamma)
                    * 255.0)
                    .round() as u8;
            }
        }

        match color_correction_mode {
            ColorCorrectionMode::LowContrast => squeeze_contrast(corrected, LOW_CONTRAST),
            ColorCorrectionMode::ReduceContrast => squeeze_contrast(corrected, REDUCED_CONTRAST),
            ColorCorrectionMode::ModernBoostContrast => boost_contrast(Rgb { r, g, b }, corrected),
            ColorCorrectionMode::ModernBalanced
            | ColorCorrectionMode::Disabled
            | ColorCorrectionMode::CorrectCurves => corrected,
        }
    }

    pub(crate) const fn set_data(&mut self, val: u8) {
        self.set_byte(self.index(), val);
        self.auto_increment();
    }

    /// Writes the byte at `index` of the palette RAM.
    const fn set_byte(&mut self, index: u8, val: u8) {
        let i = (index as usize / 2) * 3;

        if index & 1 == 0 {
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
    }

    /// A data write the PPU's palette access blocking dropped still advances
    /// the index.
    pub(crate) const fn auto_increment(&mut self) {
        if self.is_increment_enabled() {
            self.spec = (self.spec & SPEC_INCREMENT_B) | (self.index() + 1) & SPEC_INDEX;
        }
    }

    pub(crate) const fn set_spec(&mut self, val: u8) {
        self.spec = val;
    }

    #[must_use]
    pub(crate) const fn spec(&self) -> u8 {
        // Bit 6 reads 1.
        self.spec | 0x40
    }
}

/// Stretches the corrected colour `new` back to the brightest and darkest
/// channels of the uncorrected `old`.
#[expect(clippy::cast_possible_truncation)]
fn boost_contrast(old: Rgb, new: Rgb) -> Rgb {
    let Rgb { r, g, b } = old;
    let Rgb {
        r: mut new_r,
        g: mut new_g,
        b: mut new_b,
    } = new;

    let old_max = r.max(g.max(b));
    let new_max = new_r.max(new_g.max(new_b));
    if new_max != 0 {
        new_r = (u16::from(new_r) * u16::from(old_max) / u16::from(new_max)) as u8;
        new_g = (u16::from(new_g) * u16::from(old_max) / u16::from(new_max)) as u8;
        new_b = (u16::from(new_b) * u16::from(old_max) / u16::from(new_max)) as u8;
    }

    let old_min = r.min(g.min(b));
    let new_min = new_r.min(new_g.min(new_b));
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

    Rgb {
        r: new_r,
        g: new_g,
        b: new_b,
    }
}

/// Mixes a little of the other two channels into each one, then squeezes it
/// into its range.
const fn squeeze_contrast(rgb: Rgb, ranges: Rgb<ChannelRange>) -> Rgb {
    let (r, g, b) = (rgb.r as u16, rgb.g as u16, rgb.b as u16);
    Rgb {
        r: ranges.r.squeeze(r * 15 / 16 + (g + b) / 32),
        g: ranges.g.squeeze(g * 15 / 16 + (r + b) / 32),
        b: ranges.b.squeeze(b * 15 / 16 + (r + g) / 32),
    }
}
