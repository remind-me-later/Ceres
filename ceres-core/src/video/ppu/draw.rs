use crate::{
    memory::FunctionMode,
    video::sprites::{SpriteAttributes, SpriteFlags},
    SCREEN_WIDTH,
};

use super::{registers::Lcdc, BgAttributes, PixelPriority, Ppu};
use core::cmp::Ordering;
use stackvec::StackVec;

impl Ppu {
    pub fn draw_line(&mut self, function_mode: FunctionMode) {
        let mut bg_priority = [PixelPriority::Normal; SCREEN_WIDTH as usize];

        self.draw_background(function_mode, &mut bg_priority);
        self.draw_window(function_mode, &mut bg_priority);
        self.draw_sprites(function_mode, &mut bg_priority);
    }

    fn draw_background(
        &mut self,
        function_mode: FunctionMode,
        bg_priority: &mut [PixelPriority; SCREEN_WIDTH as usize],
    ) {
        let ly = self.registers.ly();
        let scy = self.registers.scy();
        let scx = self.registers.scx();
        let lcdc = self.registers.lcdc();
        let bgp = self.registers.bgp();
        let index_start = SCREEN_WIDTH as usize * ly as usize;

        if lcdc.background_enabled(function_mode) {
            let tile_map_address = lcdc.bg_tile_map_address();
            let y = ly.wrapping_add(scy);
            let row = (y / 8) as u16 * 32;
            let line = ((y % 8) * 2) as u16;

            for i in 0..SCREEN_WIDTH {
                let x = i.wrapping_add(scx);
                let col = (x / 8) as u16;

                let tile_num_address = tile_map_address + row + col;
                let tile_number = self.vram.tile_number(tile_num_address);

                let background_attributes = match function_mode {
                    FunctionMode::Monochrome | FunctionMode::Compatibility => BgAttributes::empty(),
                    FunctionMode::Color => self.vram.background_attributes(tile_num_address),
                };

                let tile_data_address = if background_attributes.contains(BgAttributes::Y_FLIP) {
                    lcdc.tile_data_address(tile_number) + 14 - line
                } else {
                    lcdc.tile_data_address(tile_number) + line
                };

                let (data_low, data_high) = self
                    .vram
                    .tile_data(tile_data_address, &background_attributes);

                let color_bit = 1
                    << if background_attributes.contains(BgAttributes::X_FLIP) {
                        x & 7
                    } else {
                        7 - (x & 7)
                    };

                let color_number =
                    (((data_high & color_bit != 0) as u8) << 1) | (data_low & color_bit != 0) as u8;

                let color = match function_mode {
                    FunctionMode::Monochrome => self
                        .monochrome_palette_colors
                        .get_color(bgp.shade_index(color_number)),
                    FunctionMode::Compatibility => self.registers.cgb_bg_palette().get_color(
                        background_attributes.bits() & 0x7,
                        bgp.shade_index(color_number),
                    ),
                    FunctionMode::Color => self
                        .registers
                        .cgb_bg_palette()
                        .get_color(background_attributes.bits() & 0x7, color_number),
                };

                self.pixel_data
                    .set_pixel_color(index_start + i as usize, color);

                bg_priority[i as usize] = if color_number == 0 {
                    PixelPriority::SpritesOnTop
                } else if background_attributes.contains(BgAttributes::BG_TO_OAM_PR) {
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
        bg_priority: &mut [PixelPriority; SCREEN_WIDTH as usize],
    ) {
        let ly = self.registers.ly();
        let lcdc = self.registers.lcdc();
        let bgp = self.registers.bgp();
        let index_start = SCREEN_WIDTH as usize * ly as usize;

        let wy = self.registers.wy();

        if lcdc.window_enabled(function_mode) && wy <= ly {
            let tile_map_address = lcdc.window_tile_map_address();
            let wx = self.registers.wx().saturating_sub(7);
            let y = ((ly - wy) as u16).wrapping_sub(self.window_lines_skipped) as u8;
            let row = (y / 8) as u16 * 32;
            let line = ((y % 8) * 2) as u16;

            for i in wx..SCREEN_WIDTH {
                self.frame_used_window = true;
                self.scanline_used_window = true;

                let x = i.wrapping_sub(wx);
                let col = (x / 8) as u16;

                let tile_num_address = tile_map_address + row + col;
                let tile_number = self.vram.tile_number(tile_num_address);

                let background_attributes = match function_mode {
                    FunctionMode::Monochrome | FunctionMode::Compatibility => BgAttributes::empty(),
                    FunctionMode::Color => self.vram.background_attributes(tile_num_address),
                };

                let tile_data_address = if background_attributes.contains(BgAttributes::Y_FLIP) {
                    lcdc.tile_data_address(tile_number) + 14 - line
                } else {
                    lcdc.tile_data_address(tile_number) + line
                };

                let (data_low, data_high) = self
                    .vram
                    .tile_data(tile_data_address, &background_attributes);

                let color_bit = 1
                    << if background_attributes.contains(BgAttributes::X_FLIP) {
                        x % 8
                    } else {
                        7 - (x % 8)
                    };

                let color_number =
                    (((data_high & color_bit != 0) as u8) << 1) | (data_low & color_bit != 0) as u8;

                let color = match function_mode {
                    FunctionMode::Monochrome => self
                        .monochrome_palette_colors
                        .get_color(bgp.shade_index(color_number)),
                    FunctionMode::Compatibility => self.registers.cgb_bg_palette().get_color(
                        background_attributes.bits() & 0x7,
                        bgp.shade_index(color_number),
                    ),
                    FunctionMode::Color => self
                        .registers
                        .cgb_bg_palette()
                        .get_color(background_attributes.bits() & 0x7, color_number),
                };

                bg_priority[i as usize] = if color_number == 0 {
                    PixelPriority::SpritesOnTop
                } else if background_attributes.contains(BgAttributes::BG_TO_OAM_PR) {
                    PixelPriority::BackgroundOnTop
                } else {
                    PixelPriority::Normal
                };

                self.pixel_data
                    .set_pixel_color(index_start + i as usize, color);
            }
        }

        if self.frame_used_window && !self.scanline_used_window {
            self.window_lines_skipped += 1;
        }
    }

    fn draw_sprites(
        &mut self,
        function_mode: FunctionMode,
        bg_priority: &mut [PixelPriority; SCREEN_WIDTH as usize],
    ) {
        let ly = self.registers.ly();
        let lcdc = self.registers.lcdc();
        let index_start = SCREEN_WIDTH as usize * ly as usize;

        let mut sprites_to_draw: StackVec<[(usize, SpriteAttributes); 10]>;

        if lcdc.contains(Lcdc::OBJECTS_ENABLED) {
            let large_sprites = lcdc.contains(Lcdc::LARGE_SPRITES);
            let sprite_height = if large_sprites { 16 } else { 8 };

            sprites_to_draw = self
                .oam
                .sprite_attributes_iterator()
                .filter(|sprite| ly.wrapping_sub(sprite.y()) < sprite_height)
                .take(10)
                .enumerate()
                .collect();

            match function_mode {
                FunctionMode::Color | FunctionMode::Compatibility
                    if self.registers.prioritize_by_oam() =>
                {
                    sprites_to_draw.sort_unstable_by(|(a_index, a), (b_index, b)| {
                        match a_index.cmp(b_index) {
                            Ordering::Equal => a.x().cmp(&b.x()),
                            other => other.reverse(),
                        }
                    });
                }
                _ => {
                    sprites_to_draw.sort_unstable_by(|(a_index, a), (b_index, b)| {
                        match a.x().cmp(&b.x()) {
                            Ordering::Equal => a_index.cmp(b_index).reverse(),
                            other => other.reverse(),
                        }
                    });
                }
            };

            for (_, sprite) in sprites_to_draw {
                let tile_number = if large_sprites {
                    sprite.tile_index() & !1
                } else {
                    sprite.tile_index()
                };

                let tile_data_address = (tile_number as u16 * 16).wrapping_add(
                    if sprite.flags().contains(SpriteFlags::FLIP_Y) {
                        (sprite_height as u16 - 1)
                            .wrapping_sub((ly.wrapping_sub(sprite.y())) as u16)
                            * 2
                    } else {
                        ly.wrapping_sub(sprite.y()) as u16 * 2
                    },
                );

                let (data_low, data_high) = self.vram.sprite_data(tile_data_address, &sprite);

                for xi in (0..8).rev() {
                    let target_x = sprite.x().wrapping_add(7 - xi);

                    if target_x >= SCREEN_WIDTH {
                        continue;
                    }

                    if bg_priority[target_x as usize] == PixelPriority::BackgroundOnTop
                        && !self
                            .registers
                            .lcdc()
                            .cgb_sprite_master_priority_on(function_mode)
                    {
                        continue;
                    }

                    let color_bit = 1
                        << if sprite.flags().contains(SpriteFlags::FLIP_X) {
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
                            let palette = if sprite.flags().contains(SpriteFlags::NON_CGB_PALETTE) {
                                self.registers.obp1()
                            } else {
                                self.registers.obp0()
                            };
                            self.monochrome_palette_colors
                                .get_color(palette.shade_index(color_number))
                        }
                        FunctionMode::Compatibility => {
                            let palette = if sprite.flags().contains(SpriteFlags::NON_CGB_PALETTE) {
                                self.registers.obp1()
                            } else {
                                self.registers.obp0()
                            };
                            self.registers
                                .cgb_sprite_palette()
                                .get_color(0, palette.shade_index(color_number))
                        }
                        FunctionMode::Color => {
                            let cgb_palette = sprite.cgb_palette();
                            self.registers
                                .cgb_sprite_palette()
                                .get_color(cgb_palette, color_number)
                        }
                    };

                    if !self
                        .registers
                        .lcdc()
                        .cgb_sprite_master_priority_on(function_mode)
                        && sprite.flags().contains(SpriteFlags::BG_WIN_OVER_OBJ)
                        && bg_priority[target_x as usize] == PixelPriority::Normal
                    {
                        continue;
                    }

                    self.pixel_data
                        .set_pixel_color(index_start + target_x as usize, color);
                }
            }
        }
    }
}
