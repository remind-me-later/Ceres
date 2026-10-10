//! Mixing the background and object pixels, and the palette lookup.

use {
    super::{
        LINES,
        fifo::{Item, PixelOut},
    },
    crate::{
        CgbMode,
        ppu::{LCDC_BG_EN_B, LCDC_OBJ_EN_B, PX_WIDTH, Ppu, color_palette::GRAYSCALE_PALETTE},
    },
};

impl Ppu {
    pub(super) fn render_pixel_if_possible(&mut self) -> Option<PixelOut> {
        let obj_en = self.lcdc & LCDC_OBJ_EN_B != 0 || self.hw_cgb();
        if self.d.objs.count != 0 && obj_en && self.d.objs.x[self.d.objs.count - 1] == 0 {
            return None;
        }
        if self.d.bg_fifo.size == 0 {
            return None;
        }

        let fifo_item = if self.d.insert_bg_pixel {
            self.d.insert_bg_pixel = false;
            Item::default()
        } else {
            self.d.bg_fifo.pop()
        };
        let mut bg_priority = fifo_item.bg_priority;
        let mut oam_item = Item::default();
        let mut draw_oam = false;

        if self.d.oam_fifo.size != 0 {
            oam_item = self.d.oam_fifo.pop();
            if oam_item.pixel != 0 && self.lcdc & LCDC_OBJ_EN_B != 0 {
                draw_oam = true;
                bg_priority |= oam_item.bg_priority;
            }
        }

        // (position + 16 < 8) in u8 arithmetic: the lead-in range.
        let position = self.d.position_in_line;
        if position.wrapping_add(16) < 8 {
            if position == 239 {
                self.d.position_in_line = 240;
            } else if position & 7 == self.scx & 7
                || self.d.window.being_fetched && position & 7 == 6 && self.scx & 7 == 7
            {
                self.d.position_in_line = 248;
            } else if position == 247 {
                self.d.position_in_line = 240;
                return None;
            } else {
                self.d.line_has_fractional_scrolling = true;
            }
        }

        self.d.window.being_fetched = false;

        // Drop pixels for scrolling (negative positions compare >= 160 in u8).
        if self.d.position_in_line >= PX_WIDTH {
            self.d.position_in_line = self.d.position_in_line.wrapping_add(1);
            return None;
        }

        // Mixing
        // LCDC bit 0 off: in CGB mode the objects lose their priority, in
        // DMG mode the background is blank.
        let bg_enabled = self.lcdc & LCDC_BG_EN_B != 0 || self.cgb_mode_on();
        if self.lcdc & LCDC_BG_EN_B == 0 && self.cgb_mode_on() {
            bg_priority = false;
        }
        let bg_pixel = if bg_enabled { fifo_item.pixel } else { 0 };
        if bg_pixel != 0 && bg_priority {
            draw_oam = false;
        }

        let out = PixelOut {
            lx: self.d.lcd_x,
            bg_pixel,
            bg_palette: fifo_item.palette,
            obj: draw_oam.then_some((oam_item.pixel, oam_item.palette)),
        };

        self.d.position_in_line = self.d.position_in_line.wrapping_add(1);
        self.d.lcd_x = self.d.lcd_x.wrapping_add(1);
        Some(out)
    }

    pub(super) fn output_pixel(&mut self, out: PixelOut) {
        if out.lx >= PX_WIDTH || self.d.current_line >= LINES {
            return;
        }
        let rgb = self.pixel_rgb(out);
        let idx = u32::from(self.d.current_line) * u32::from(PX_WIDTH) + u32::from(out.lx);
        self.rgb_buf.set_px(idx, rgb);
    }

    #[must_use]
    const fn mono_rgb(index: u8) -> (u8, u8, u8) {
        GRAYSCALE_PALETTE[index as usize]
    }

    /// Palette lookup for one LCD pixel (SameBoy's `render_pixel_if_possible`
    /// tail): the BGP/OBP shade is picked here, at the dot the pixel is drawn.
    #[must_use]
    pub(super) fn pixel_rgb(&self, out: PixelOut) -> (u8, u8, u8) {
        let cgb_mode = self.cgb_mode;

        // The PPU can't read the palettes in STOP mode: black.
        if self.d.bus.cgb_palettes_ppu_blocked {
            return (0, 0, 0);
        }

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
