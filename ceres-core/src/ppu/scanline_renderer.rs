use {
    super::{
        Ppu, SpriteAttr, BG_PAL, BG_TO_OAM_PR, BG_X_FLIP, BG_Y_FLIP, LARGE_SPRITES, OAM_SIZE,
        OBJECTS_ENABLED, SPR_BG_WIN_OVER_OBJ, SPR_CGB_PAL, SPR_FLIP_X, SPR_FLIP_Y, SPR_PAL,
    },
    crate::{FunctionMode, PX_WIDTH},
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PixelPriority {
    SpritesOnTop,
    BackgroundOnTop,
    Normal,
}

impl Ppu {
    pub(crate) fn draw_scanline(&mut self, function_mode: FunctionMode) {
        let mut bg_priority = [PixelPriority::Normal; PX_WIDTH as usize];
        let index_start = PX_WIDTH as usize * self.ly as usize;

        self.draw_background(function_mode, &mut bg_priority, index_start);
        self.draw_window(function_mode, &mut bg_priority, index_start);
        self.draw_sprites(function_mode, &mut bg_priority, index_start);
    }

    fn shade_index(reg: u8, color_number: u8) -> u8 {
        (reg >> (color_number * 2)) & 0x3
    }

    fn draw_background(
        &mut self,
        function_mode: FunctionMode,
        bg_priority: &mut [PixelPriority; PX_WIDTH as usize],
        index_start: usize,
    ) {
        if self.is_bg_enabled(function_mode) {
            let tile_map_addr = self.bg_tile_map_addr();
            let y = self.ly.wrapping_add(self.scy);
            let row = (y / 8) as u16 * 32;
            let line = ((y % 8) * 2) as u16;

            for i in 0..PX_WIDTH {
                let x = i.wrapping_add(self.scx);
                let col = (x / 8) as u16;

                let tile_num_addr = tile_map_addr + row + col;
                let tile_number = self.tile_number(tile_num_addr);

                let gb_attr = match function_mode {
                    FunctionMode::Monochrome | FunctionMode::Compatibility => 0,
                    FunctionMode::Color => self.bg_attr(tile_num_addr),
                };

                let tile_data_addr = if gb_attr & BG_Y_FLIP != 0 {
                    self.tile_data_addr(tile_number) + 14 - line
                } else {
                    self.tile_data_addr(tile_number) + line
                };

                let (data_low, data_high) = self.bg_tile_data(tile_data_addr, gb_attr);

                let color_bit = 1
                    << if gb_attr & BG_X_FLIP != 0 {
                        x & 7
                    } else {
                        7 - (x & 7)
                    };

                let color_number =
                    (((data_high & color_bit != 0) as u8) << 1) | (data_low & color_bit != 0) as u8;

                let color = match function_mode {
                    FunctionMode::Monochrome => {
                        self.get_mono_color(Self::shade_index(self.bgp, color_number))
                    }
                    FunctionMode::Compatibility => self
                        .cgb_bg_palette
                        .get_color(gb_attr & BG_PAL, Self::shade_index(self.bgp, color_number)),
                    FunctionMode::Color => self
                        .cgb_bg_palette
                        .get_color(gb_attr & BG_PAL, color_number),
                };

                self.rgba_buf
                    .set_pixel_color(index_start + i as usize, color);

                bg_priority[i as usize] = if color_number == 0 {
                    PixelPriority::SpritesOnTop
                } else if gb_attr & BG_TO_OAM_PR != 0 {
                    PixelPriority::BackgroundOnTop
                } else {
                    PixelPriority::Normal
                };
            }
        }
    }

    fn draw_window(
        &mut self,
        function_mode: FunctionMode,
        bg_priority: &mut [PixelPriority; PX_WIDTH as usize],
        index_start: usize,
    ) {
        if self.is_win_enabled(function_mode) && self.wy <= self.ly {
            let tile_map_addr = self.window_tile_map_addr();
            let wx = self.wx.saturating_sub(7);
            let y = ((self.ly - self.wy) as u16).wrapping_sub(self.window_lines_skipped) as u8;
            let row = (y / 8) as u16 * 32;
            let line = ((y % 8) * 2) as u16;

            for i in wx..PX_WIDTH {
                self.frame_used_window = true;
                self.scanline_used_window = true;

                let x = i.wrapping_sub(wx);
                let col = (x / 8) as u16;

                let tile_num_addr = tile_map_addr + row + col;
                let tile_number = self.tile_number(tile_num_addr);

                let bg_attr = match function_mode {
                    FunctionMode::Monochrome | FunctionMode::Compatibility => 0,
                    FunctionMode::Color => self.bg_attr(tile_num_addr),
                };

                let tile_data_addr = if bg_attr & BG_Y_FLIP != 0 {
                    self.tile_data_addr(tile_number) + 14 - line
                } else {
                    self.tile_data_addr(tile_number) + line
                };

                let (data_low, data_high) = self.bg_tile_data(tile_data_addr, bg_attr);

                let color_bit = 1
                    << if bg_attr & BG_X_FLIP != 0 {
                        x % 8
                    } else {
                        7 - (x % 8)
                    };

                let color_number =
                    (((data_high & color_bit != 0) as u8) << 1) | (data_low & color_bit != 0) as u8;

                let color = match function_mode {
                    FunctionMode::Monochrome => {
                        self.get_mono_color(Self::shade_index(self.bgp, color_number))
                    }
                    FunctionMode::Compatibility => self
                        .cgb_bg_palette
                        .get_color(bg_attr & BG_PAL, Self::shade_index(self.bgp, color_number)),
                    FunctionMode::Color => self
                        .cgb_bg_palette
                        .get_color(bg_attr & BG_PAL, color_number),
                };

                bg_priority[i as usize] = if color_number == 0 {
                    PixelPriority::SpritesOnTop
                } else if bg_attr & BG_TO_OAM_PR != 0 {
                    PixelPriority::BackgroundOnTop
                } else {
                    PixelPriority::Normal
                };

                self.rgba_buf
                    .set_pixel_color(index_start + i as usize, color);
            }
        }

        if self.frame_used_window && !self.scanline_used_window {
            self.window_lines_skipped += 1;
        }
    }

    fn get_sprites(&mut self, height: u8) -> ([SpriteAttr; 10], usize) {
        let mut len = 0;
        // TODO: not pretty
        let mut sprites = [
            SpriteAttr::default(),
            SpriteAttr::default(),
            SpriteAttr::default(),
            SpriteAttr::default(),
            SpriteAttr::default(),
            SpriteAttr::default(),
            SpriteAttr::default(),
            SpriteAttr::default(),
            SpriteAttr::default(),
            SpriteAttr::default(),
        ];

        for i in (0..OAM_SIZE).step_by(4) {
            let y = self.oam[i].wrapping_sub(16);

            if self.ly.wrapping_sub(y) < height {
                let attr = SpriteAttr {
                    y,
                    x: self.oam[i + 1].wrapping_sub(8),
                    tile_index: self.oam[i + 2],
                    flags: self.oam[i + 3],
                };

                sprites[len] = attr;
                len += 1;

                if len == 10 {
                    break;
                }
            }
        }

        if self.opri & 1 == 1 {
            // DMG order: first X pos, else OAM pos
            for i in 0..len {
                let mut j = i;

                while j > 0 && sprites[j - 1].x <= sprites[j].x {
                    sprites.swap(j - 1, j);
                    j -= 1;
                }
            }
        } else {
            // CGB order: OAM pos
            for i in 0..len {
                let mut j = i;

                while j > 0 && j - 1 < j {
                    sprites.swap(j - 1, j);
                    j -= 1;
                }
            }
        }

        (sprites, len)
    }

    fn draw_sprites(
        &mut self,
        function_mode: FunctionMode,
        bg_priority: &mut [PixelPriority; PX_WIDTH as usize],
        index_start: usize,
    ) {
        if self.lcdc & OBJECTS_ENABLED != 0 {
            let is_large = self.lcdc & LARGE_SPRITES != 0;
            let height = if is_large { 16 } else { 8 };

            let (sprites, sprites_len) = self.get_sprites(height);

            for sprite in sprites.iter().take(sprites_len) {
                let tile_number = if is_large {
                    sprite.tile_index & !1
                } else {
                    sprite.tile_index
                };

                let tile_data_addr =
                    (tile_number as u16 * 16).wrapping_add(if sprite.flags & SPR_FLIP_Y != 0 {
                        (height as u16 - 1).wrapping_sub((self.ly.wrapping_sub(sprite.y)) as u16)
                            * 2
                    } else {
                        self.ly.wrapping_sub(sprite.y) as u16 * 2
                    });

                let (data_low, data_high) = self.sprite_tile_data(tile_data_addr, sprite);

                for xi in (0..8).rev() {
                    let target_x = sprite.x.wrapping_add(7 - xi);

                    if target_x >= PX_WIDTH {
                        continue;
                    }

                    if bg_priority[target_x as usize] == PixelPriority::BackgroundOnTop
                        && !self.is_cgb_sprite_master_priority_on(function_mode)
                    {
                        continue;
                    }

                    let color_bit = 1
                        << if sprite.flags & SPR_FLIP_X != 0 {
                            7 - xi
                        } else {
                            xi
                        };

                    let color_number = (((data_high & color_bit != 0) as u8) << 1)
                        | (data_low & color_bit != 0) as u8;

                    // transparent
                    if color_number == 0 {
                        continue;
                    }

                    let color = match function_mode {
                        FunctionMode::Monochrome => {
                            let palette = if sprite.flags & SPR_PAL != 0 {
                                self.obp1
                            } else {
                                self.obp0
                            };
                            self.get_mono_color(Self::shade_index(palette, color_number))
                        }
                        FunctionMode::Compatibility => {
                            let palette = if sprite.flags & SPR_PAL != 0 {
                                self.obp1
                            } else {
                                self.obp0
                            };
                            self.cgb_sprite_palette
                                .get_color(0, Self::shade_index(palette, color_number))
                        }
                        FunctionMode::Color => {
                            let cgb_palette = sprite.flags & SPR_CGB_PAL;
                            self.cgb_sprite_palette.get_color(cgb_palette, color_number)
                        }
                    };

                    if !self.is_cgb_sprite_master_priority_on(function_mode)
                        && sprite.flags & SPR_BG_WIN_OVER_OBJ != 0
                        && bg_priority[target_x as usize] == PixelPriority::Normal
                    {
                        continue;
                    }

                    self.rgba_buf
                        .set_pixel_color(index_start + target_x as usize, color);
                }
            }
        }
    }
}
