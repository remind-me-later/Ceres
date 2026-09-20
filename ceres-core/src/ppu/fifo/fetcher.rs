use super::pixel::Pixel;
use super::sprite::Sprite;
use crate::ppu::vram::Vram;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FetcherState {
    #[default]
    GetTile,
    GetTileDataLow,
    GetTileDataHigh,
    Push,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct TileFetcher {
    state: FetcherState,
    cycle: u8,
    tile_id: u8,
    tile_attr: u8,
    tile_data_low: u8,
    tile_data_high: u8,
    row_in_tile: u16,
    bg_tile_x: u8,
    window_tile_x: u8,
    window_line_counter: u8,
    is_window: bool,
    sprite_fetch: Option<(Sprite, u8)>, // (Sprite, sub_cycle)
    map_base: u16,
    data_addr: u16,
}

impl TileFetcher {
    pub const fn new() -> Self {
        Self {
            state: FetcherState::GetTile,
            cycle: 0,
            tile_id: 0,
            tile_attr: 0,
            tile_data_low: 0,
            tile_data_high: 0,
            row_in_tile: 0,
            bg_tile_x: 0,
            window_tile_x: 0,
            window_line_counter: 0,
            is_window: false,
            sprite_fetch: None,
            map_base: 0x1800,
            data_addr: 0,
        }
    }

    pub fn reset_bg(&mut self, scx: u8) {
        self.state = FetcherState::GetTile;
        self.cycle = 0;
        self.tile_id = 0;
        self.tile_attr = 0;
        self.tile_data_low = 0;
        self.tile_data_high = 0;
        self.row_in_tile = 0;
        self.bg_tile_x = (scx >> 3) & 0x1F;
        self.window_tile_x = 0;
        self.is_window = false;
        self.sprite_fetch = None;
        self.map_base = 0x1800;
        self.data_addr = 0;
    }

    pub fn set_bg_tile_x(&mut self, scx: u8) {
        self.bg_tile_x = (scx >> 3) & 0x1F;
    }

    pub fn is_get_tile_t1(&self) -> bool {
        self.state == FetcherState::GetTile && self.cycle == 0
    }

    pub fn reset_window(&mut self, window_line_counter: u8) {
        self.state = FetcherState::GetTile;
        self.cycle = 1;
        self.tile_id = 0;
        self.tile_attr = 0;
        self.tile_data_low = 0;
        self.tile_data_high = 0;
        self.row_in_tile = 0;
        self.window_tile_x = 0;
        self.window_line_counter = window_line_counter;
        self.is_window = true;
        self.sprite_fetch = None;
        self.map_base = 0;
        self.data_addr = 0;
    }

    /// Advance fetcher by 1 T-cycle.
    /// Returns true if 8 pixels are ready to push to FIFO.
    /// `position` is the FIFO's u8-wrapped PPU X position (SameBoy's
    /// `position_in_line`), from which the BG map column is derived live.
    #[allow(clippy::too_many_arguments)]
    pub fn step_t_cycle(
        &mut self,
        vram: &Vram,
        ly: u8,
        scy: u8,
        scx: u8,
        position: u8,
        lcdc: u8,
        is_cgb: bool,
        is_cgb_hardware: bool,
        bg_len: usize,
    ) -> Option<[Pixel; 8]> {
        // Handle sprite fetch stall if active
        if let Some((_sprite, cycle)) = &mut self.sprite_fetch {
            *cycle += 1;
            if *cycle >= 6 {
                self.sprite_fetch = None;
            }
            return None;
        }

        if self.cycle == 0 {
            self.cycle = 1;
            match self.state {
                FetcherState::GetTile => {
                    if !self.is_window {
                        self.map_base = if lcdc & 0x08 != 0 { 0x1C00 } else { 0x1800 };
                    }
                }
                FetcherState::GetTileDataLow => {
                    self.data_addr = self.calculate_tile_data_addr(lcdc, ly, scy);
                }
                FetcherState::GetTileDataHigh => {
                    self.data_addr = self.calculate_tile_data_addr(lcdc, ly, scy) + 1;
                }
                FetcherState::Push => {}
            }
            return None;
        }

        self.cycle = 0;

        match self.state {
            FetcherState::GetTile => {
                let (map_base, tile_col, tile_row, row_in_tile) = if self.is_window {
                    let map = if lcdc & 0x40 != 0 { 0x1C00 } else { 0x1800 };
                    let col = self.window_tile_x & 0x1F;
                    let row = (self.window_line_counter / 8) & 0x1F;
                    let r = (self.window_line_counter % 8) as u16;
                    (map, col, row, r)
                } else {
                    let map = self.map_base;
                    let col = if position.wrapping_add(16) < 8 || position >= 240 {
                        (scx >> 3) & 0x1F
                    } else {
                        let offset = if is_cgb_hardware { 7 } else { 8 };
                        let fetch_x = scx.wrapping_add(position).wrapping_add(offset);
                        (fetch_x >> 3) & 0x1F
                    };
                    let row = (ly.wrapping_add(scy) / 8) & 0x1F;
                    let r = (ly.wrapping_add(scy) % 8) as u16;
                    (map, col, row, r)
                };

                let map_addr = map_base + (u16::from(tile_row) * 32) + u16::from(tile_col);
                self.row_in_tile = row_in_tile;

                self.tile_id = vram.vram_at_bank(map_addr, 0);
                self.tile_attr = if is_cgb {
                    vram.vram_at_bank(map_addr, 1)
                } else {
                    0
                };

                self.state = FetcherState::GetTileDataLow;
                None
            }
            FetcherState::GetTileDataLow => {
                let bank = if is_cgb && (self.tile_attr & 0x08 != 0) {
                    1
                } else {
                    0
                };
                self.tile_data_low = vram.vram_at_bank(self.data_addr, bank);

                self.state = FetcherState::GetTileDataHigh;
                None
            }
            FetcherState::GetTileDataHigh => {
                let bank = if is_cgb && (self.tile_attr & 0x08 != 0) {
                    1
                } else {
                    0
                };
                self.tile_data_high = vram.vram_at_bank(self.data_addr, bank);

                // Always hand over to Push: the fetch cycle must stay a
                // constant 8 dots so the fetcher never laps the pixel pops
                // (a 6-dot cycle injects a duplicate tile row and shifts
                // the line by 8 px).
                self.state = FetcherState::Push;
                None
            }
            FetcherState::Push => {
                if bg_len <= 8 {
                    // Decode 8 pixels
                    let pixels = self.decode_bg_pixels(is_cgb);
                    if self.is_window {
                        self.window_tile_x = (self.window_tile_x + 1) & 0x1F;
                    } else {
                        self.bg_tile_x = (self.bg_tile_x + 1) & 0x1F;
                    }
                    self.state = FetcherState::GetTile;
                    Some(pixels)
                } else {
                    None
                }
            }
        }
    }

    fn calculate_tile_data_addr(&self, lcdc: u8, ly: u8, scy: u8) -> u16 {
        let is_signed = lcdc & 0x10 == 0;
        let flip_y = self.tile_attr & 0x40 != 0;
        let mut row_in_tile = if self.is_window {
            self.row_in_tile
        } else {
            (ly.wrapping_add(scy) % 8) as u16
        };
        if flip_y {
            row_in_tile = 7 - row_in_tile;
        }

        if is_signed {
            let signed_id = self.tile_id as i8;
            let offset = (i32::from(signed_id) + 128) as u16;
            0x0800 + (offset * 16) + (row_in_tile * 2)
        } else {
            (u16::from(self.tile_id) * 16) + (row_in_tile * 2)
        }
    }

    fn decode_bg_pixels(&self, is_cgb: bool) -> [Pixel; 8] {
        let mut pixels = [Pixel::empty(); 8];
        let flip_x = self.tile_attr & 0x20 != 0;
        let palette = if is_cgb { self.tile_attr & 0x07 } else { 0 };
        let bg_priority = is_cgb && (self.tile_attr & 0x80 != 0);

        for i in 0..8 {
            let bit_idx = if flip_x { i } else { 7 - i };
            let low_bit = (self.tile_data_low >> bit_idx) & 1;
            let high_bit = (self.tile_data_high >> bit_idx) & 1;
            let color_id = (high_bit << 1) | low_bit;

            pixels[i as usize] = Pixel::new(color_id, palette, bg_priority, u8::MAX);
        }

        pixels
    }

    #[must_use]
    pub fn fetch_sprite_low(
        &self,
        sprite: Sprite,
        vram: &Vram,
        ly: u8,
        sprite_height: u8,
        is_cgb: bool,
    ) -> u8 {
        let flip_y = sprite.y_flip();
        let height_16 = sprite_height == 16;
        let mut tile_y = (u16::from(ly) + 16).wrapping_sub(u16::from(sprite.y()))
            & if height_16 { 0x0F } else { 7 };
        if flip_y {
            tile_y ^= if height_16 { 0x0F } else { 7 };
        }

        let tile_id = if height_16 {
            sprite.tile() & 0xFE
        } else {
            sprite.tile()
        };

        let tile_addr = (u16::from(tile_id) * 16) + tile_y * 2;
        let bank = if is_cgb { sprite.cgb_vram_bank() } else { 0 };
        vram.vram_at_bank(tile_addr, bank)
    }

    #[must_use]
    pub fn fetch_sprite_high(
        &self,
        sprite: Sprite,
        vram: &Vram,
        ly: u8,
        sprite_height: u8,
        is_cgb: bool,
    ) -> u8 {
        let flip_y = sprite.y_flip();
        let height_16 = sprite_height == 16;
        let mut tile_y = (u16::from(ly) + 16).wrapping_sub(u16::from(sprite.y()))
            & if height_16 { 0x0F } else { 7 };
        if flip_y {
            tile_y ^= if height_16 { 0x0F } else { 7 };
        }

        let tile_id = if height_16 {
            sprite.tile() & 0xFE
        } else {
            sprite.tile()
        };

        let tile_addr = (u16::from(tile_id) * 16) + tile_y * 2;
        let bank = if is_cgb { sprite.cgb_vram_bank() } else { 0 };
        vram.vram_at_bank(tile_addr + 1, bank)
    }

    #[must_use]
    pub fn decode_sprite_pixels(
        &self,
        sprite: Sprite,
        low: u8,
        high: u8,
        is_cgb: bool,
    ) -> [Pixel; 8] {
        let flip_x = sprite.x_flip();
        let palette = if is_cgb {
            sprite.cgb_palette()
        } else {
            sprite.dmg_palette()
        };

        let mut pixels = [Pixel::empty(); 8];
        for i in 0..8 {
            let bit_idx = if flip_x { i } else { 7 - i };
            let color_id = ((high >> bit_idx) & 1) << 1 | ((low >> bit_idx) & 1);
            pixels[i as usize] = Pixel::new(
                color_id,
                palette,
                sprite.bg_priority(),
                if is_cgb {
                    sprite.oam_index()
                } else {
                    sprite.x()
                },
            );
        }

        pixels
    }
}
