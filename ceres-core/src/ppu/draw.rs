use {
    super::{Ppu, display::PixelOut},
    crate::CgbMode,
};

impl Ppu {
    #[must_use]
    const fn mono_rgb(index: u8) -> (u8, u8, u8) {
        super::color_palette::GRAYSCALE_PALETTE[index as usize]
    }

    /// Palette lookup for one LCD pixel (SameBoy's `render_pixel_if_possible`
    /// tail): the BGP/OBP shade is picked here, at the dot the pixel is drawn.
    #[must_use]
    pub(super) fn pixel_rgb(&self, out: PixelOut) -> (u8, u8, u8) {
        let cgb_mode = self.cgb_mode;

        if let Some((pixel, palette)) = out.obj {
            let shade = if cgb_mode == CgbMode::Cgb {
                pixel
            } else {
                let obp = if palette == 0 { self.obp0 } else { self.obp1 };
                (obp >> (pixel << 1)) & 3
            };
            return match cgb_mode {
                CgbMode::Dmg => Self::mono_rgb(shade),
                _ => self.ocp.rgb(palette, shade, self.color_correction_mode),
            };
        }

        let shade = if cgb_mode == CgbMode::Cgb {
            out.bg_pixel
        } else {
            (self.bgp >> (out.bg_pixel << 1)) & 3
        };
        match cgb_mode {
            CgbMode::Dmg => Self::mono_rgb(shade),
            _ => self
                .bcp
                .rgb(out.bg_palette, shade, self.color_correction_mode),
        }
    }

    /// Colour used for the first pixel of a line the PPU failed to draw.
    #[must_use]
    pub(super) fn desync_color(&self) -> (u8, u8, u8) {
        match self.cgb_mode {
            CgbMode::Dmg => Self::mono_rgb(0),
            _ => self.bcp.rgb(0, 0, self.color_correction_mode),
        }
    }
}
