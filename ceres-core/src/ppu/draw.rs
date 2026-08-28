use {
    super::Ppu,
    crate::{ppu::fifo::Pixel, CgbMode},
};

impl Ppu {
    #[must_use]
    const fn mono_rgb(index: u8) -> (u8, u8, u8) {
        super::color_palette::GRAYSCALE_PALETTE[index as usize]
    }

    #[must_use]
    pub fn resolve_fifo_pixel(
        &self,
        bg_px: Pixel,
        sprite_px: Pixel,
        cgb_mode: CgbMode,
    ) -> (u8, u8, u8) {
        let master_bg_enable = self.lcdc & 0x01 != 0;
        let master_obj_enable = self.lcdc & 0x02 != 0;

        let bg_has_priority = match cgb_mode {
            CgbMode::Dmg | CgbMode::Compat => {
                sprite_px.bg_priority() && bg_px.color_id() != 0
            }
            CgbMode::Cgb => {
                if !master_bg_enable {
                    false
                } else if bg_px.bg_priority() && bg_px.color_id() != 0 {
                    true
                } else {
                    sprite_px.bg_priority() && bg_px.color_id() != 0
                }
            }
        };

        let show_sprite = master_obj_enable && sprite_px.color_id() != 0 && !bg_has_priority;

        if show_sprite {
            match cgb_mode {
                CgbMode::Dmg => {
                    let pal = if sprite_px.palette() == 0 {
                        self.obp0
                    } else {
                        self.obp1
                    };
                    Self::mono_rgb(shade_index(pal, sprite_px.color_id()))
                }
                CgbMode::Compat => {
                    let pal = if sprite_px.palette() == 0 {
                        self.obp0
                    } else {
                        self.obp1
                    };
                    self.ocp.rgb(
                        sprite_px.palette(),
                        shade_index(pal, sprite_px.color_id()),
                        self.color_correction_mode,
                    )
                }
                CgbMode::Cgb => self.ocp.rgb(
                    sprite_px.palette(),
                    sprite_px.color_id(),
                    self.color_correction_mode,
                ),
            }
        } else if !master_bg_enable && cgb_mode != CgbMode::Cgb {
            match cgb_mode {
                CgbMode::Dmg => Self::mono_rgb(0),
                CgbMode::Compat => self.bcp.rgb(0, 0, self.color_correction_mode),
                CgbMode::Cgb => unreachable!(),
            }
        } else {
            let bg_color = bg_px.color_id();
            match cgb_mode {
                CgbMode::Dmg => Self::mono_rgb(shade_index(self.bgp, bg_color)),
                CgbMode::Compat => self.bcp.rgb(
                    0,
                    shade_index(self.bgp, bg_color),
                    self.color_correction_mode,
                ),
                CgbMode::Cgb => self
                    .bcp
                    .rgb(bg_px.palette(), bg_color, self.color_correction_mode),
            }
        }
    }
}

const fn shade_index(palette: u8, color: u8) -> u8 {
    (palette >> (color * 2)) & 0x3
}
