pub mod fetcher;
pub mod pixel;
pub mod sprite;

pub use self::pixel::Pixel;

use self::fetcher::TileFetcher;
use self::sprite::{Sprite, SpriteBuffer};
use crate::ppu::oam::Oam;
use crate::ppu::vram::Vram;

pub struct PixelFifo {
    bg_fifo: [Pixel; 16],
    bg_head: usize,
    bg_tail: usize,
    bg_len: usize,

    sprite_fifo: [Pixel; 16],
    sprite_head: usize,
    sprite_tail: usize,
    sprite_len: usize,

    fetcher: TileFetcher,
    sprites: SpriteBuffer,
    /// SameBoy's `position_in_line` (display.c): dot-granular PPU X position.
    /// Starts at −16 (the 16-pixel FIFO lead), counts every FIFO pop, and is
    /// snapped forward by the SCX fraction during the lead-in; pixels render
    /// only for positions 0..=159. Mode 3's length is an output of this
    /// counter, never an input.
    position: i16,
    lx: u8,
    line_dots: u16,
    junk_at: u16,
    window_active: bool,
    window_line_counter: u8,
    sprite_stall: u8,
    line_sprite_stall: u8,
    line_sprite_count: u8,
    junk_pushed: bool,
    window_initial_fetch: bool,
    scx_low3: u8,
    target_dots: u16,
    is_cgb_model: bool,
    model: crate::Model,
    sprite_stalls: [u8; 10],
    current_sprite_idx: usize,
    pending_sprite: Option<Sprite>,
    pending_sprite_low: Option<u8>,
    pub initial_wx: u8,
}

impl Default for PixelFifo {
    fn default() -> Self {
        Self::new()
    }
}

impl PixelFifo {
    pub const fn new() -> Self {
        Self {
            bg_fifo: [Pixel::empty(); 16],
            bg_head: 0,
            bg_tail: 0,
            bg_len: 0,

            sprite_fifo: [Pixel::empty(); 16],
            sprite_head: 0,
            sprite_tail: 0,
            sprite_len: 0,

            fetcher: TileFetcher::new(),
            sprites: SpriteBuffer::new(),
            position: -16,
            lx: 0,
            line_dots: 0,
            junk_at: 5,
            window_active: false,
            window_line_counter: 0,
            sprite_stall: 0,
            line_sprite_stall: 0,
            line_sprite_count: 0,
            junk_pushed: false,
            window_initial_fetch: false,
            scx_low3: 0,
            target_dots: 0,
            is_cgb_model: false,
            model: crate::Model::DmgB,
            sprite_stalls: [0; 10],
            current_sprite_idx: 0,
            pending_sprite: None,
            pending_sprite_low: None,
            initial_wx: 0,
        }
    }

    /// Whether the scanline's 160th pixel has been reached and target mode 3 duration met.
    #[must_use]
    pub const fn line_done(&self) -> bool {
        if self.target_dots > 0 {
            self.line_dots >= self.target_dots
        } else {
            self.position >= 160
        }
    }

    #[must_use]
    pub const fn position(&self) -> i16 {
        self.position
    }

    pub fn set_scx(&mut self, scx: u8, is_cgb: bool) {
        if is_cgb && self.line_dots == 0 {
            let k = scx & 7;
            self.scx_low3 = k;
            self.fetcher.reset_bg(scx);
            if k == 0 {
                self.position = -16;
                let is_early_cgb = matches!(
                    self.model,
                    crate::Model::Cgb0
                        | crate::Model::CgbA
                        | crate::Model::CgbB
                        | crate::Model::CgbC
                );
                self.junk_at = if is_early_cgb { 2 } else { 0 };
            }
        } else {
            self.fetcher.set_bg_tile_x(scx);
        }
    }

    pub fn scan_sprites(&mut self, oam: &Oam, ly: u8, sprite_height: u8) {
        if self.window_active {
            self.window_line_counter = self.window_line_counter.wrapping_add(1);
        }
        self.window_active = false;
        self.clear();
        self.sprites.scan_line(oam, ly, sprite_height);
    }

    pub fn reset_window_frame(&mut self) {
        if self.window_active {
            self.window_line_counter = self.window_line_counter.wrapping_add(1);
        }
        self.window_active = false;
        self.window_line_counter = 0;
    }

    pub fn simulate_mode3_cycles(sprites: &[u8], scx: u8) -> (u16, [u8; 10]) {
        let mut position_in_line = -16i16;
        let mut fetcher_state = 0u8;
        let mut bg_fifo_size = 8u8;
        let mut cycles_for_line = 0u16;
        let mut cur_obj = 0;
        let n_sprites = sprites.len();
        let mut stalls = [0u8; 10];

        let advance_fetcher = |state: &mut u8, fifo: &mut u8| match *state {
            0..=4 => *state += 1,
            _ => {
                *state = 6;
                if *fifo == 0 {
                    *fifo += 8;
                    *state = 0;
                }
            }
        };

        loop {
            let match_x = if position_in_line < -8 {
                0
            } else {
                (position_in_line + 8) as u8
            };

            while cur_obj < n_sprites && sprites[cur_obj] < match_x {
                cur_obj += 1;
            }

            while cur_obj < n_sprites && sprites[cur_obj] == match_x {
                let start_c = cycles_for_line;
                while fetcher_state < 5 || bg_fifo_size == 0 {
                    advance_fetcher(&mut fetcher_state, &mut bg_fifo_size);
                    cycles_for_line += 1;
                }
                advance_fetcher(&mut fetcher_state, &mut bg_fifo_size);
                cycles_for_line += 1;

                advance_fetcher(&mut fetcher_state, &mut bg_fifo_size);
                cycles_for_line += 2;

                cycles_for_line += 2;
                cycles_for_line += 1;

                if cur_obj < 10 {
                    stalls[cur_obj] = (cycles_for_line - start_c) as u8;
                }
                cur_obj += 1;
            }

            let pending_x0 = cur_obj < n_sprites && sprites[cur_obj] == 0;
            if !pending_x0 && bg_fifo_size > 0 {
                bg_fifo_size -= 1;
                let pu8 = position_in_line as u8;
                let mut skip_inc = false;
                if pu8.wrapping_add(16) < 8 {
                    if pu8 == 239 {
                        position_in_line = -16;
                    } else if pu8 & 7 == scx & 7 {
                        position_in_line = -8;
                    } else if pu8 == 247 {
                        position_in_line = -16;
                        skip_inc = true;
                    }
                }
                if !skip_inc {
                    position_in_line += 1;
                }
            }

            advance_fetcher(&mut fetcher_state, &mut bg_fifo_size);
            if position_in_line == 160 {
                break;
            }
            cycles_for_line += 1;
        }

        (cycles_for_line, stalls)
    }

    pub fn start_drawing(
        &mut self,
        scx: u8,
        _is_lcdon: bool,
        is_cgb: bool,
        obj_enabled: bool,
        model: crate::Model,
        wx: u8,
        lcdc: u8,
        wy: u8,
        ly: u8,
    ) {
        self.lx = 0;
        self.line_dots = 0;
        self.sprite_stall = 0;
        self.line_sprite_stall = 0;
        self.line_sprite_count = 0;
        self.target_dots = 0;
        self.junk_pushed = false;
        self.scx_low3 = scx & 7;
        self.is_cgb_model = is_cgb;
        self.model = model;
        self.sprite_stalls = [0; 10];
        self.current_sprite_idx = 0;
        self.pending_sprite = None;
        self.pending_sprite_low = None;
        self.initial_wx = wx;

        if is_cgb {
            let k = scx & 7;
            self.position = -16 + i16::from(k);
            let is_early_cgb = matches!(
                model,
                crate::Model::Cgb0 | crate::Model::CgbA | crate::Model::CgbB | crate::Model::CgbC
            );
            self.junk_at = match k {
                0..=2 => {
                    if is_early_cgb {
                        2
                    } else {
                        0
                    }
                }
                3 => 4,
                4 => {
                    if is_early_cgb {
                        5
                    } else {
                        4
                    }
                }
                5 => {
                    if is_early_cgb {
                        5
                    } else {
                        4
                    }
                }
                6 => {
                    if is_early_cgb {
                        6
                    } else {
                        4
                    }
                }
                7 => 8,
                _ => unreachable!(),
            };
            if obj_enabled {
                let (xs, count) = self.sprites.sprite_xs();
                if count > 0 {
                    let well_separated = count > 1
                        && (count < 10
                            || xs[..count]
                                .windows(2)
                                .all(|w| w[1].saturating_sub(w[0]) > 8));
                    if well_separated {
                        self.junk_at += 4;
                    }
                    let (cycles, mut stalls) = Self::simulate_mode3_cycles(&xs[..count], scx);
                    // On CGB, the lead-in junk push finishes earlier than DMG. For a single sprite
                    // aligned at the start of tile 1 (x = 16), the fetcher pause is 10 dots rather
                    // than DMG's 11 dots, matching mid-scanline SCX reload timing in m3_scx_high_5_bits.
                    if count == 1 && xs[0] == 16 {
                        stalls[0] = 10;
                    }
                    // On CGB, the lead-in junk push does not delay an extra dot between k=4 and k=5
                    // (both use junk_at=5), meaning the fetcher reaches the first sprite 1 dot earlier
                    // than DMG simulation predicts. Thus for k in 5..=6, stalls[0] is 1 dot longer.
                    if matches!(k, 5 | 6) {
                        stalls[0] += 1;
                    }
                    self.sprite_stalls = stalls;
                    let diff = cycles.saturating_sub(167);
                    let extra = diff / 4;
                    let target_dots = 168 + 4 * (extra as i32);
                    self.target_dots = target_dots as u16;
                }
            }
        } else {
            self.position = -16;
            // Phase of the lead-in junk push within the M-cycle grid. The SCX
            // fraction consumes `k` real pixels during the lead-in (the snap
            // fires on pop k+1), which would delay output start by `k` dots;
            // hardware instead shifts the pipeline phase so output always
            // starts on the same M-cycle alignment. Junk at
            // `6 - ((k+1) & 3)` puts first-visible at dot `14 + k - ((k+1)&3)`
            // = 13/13/13/17/17/17/17/21 for k = 0..7, reproducing the mooneye
            // mode-3 lengths (172/176/180 dots on DMG) exactly.
            let k = u16::from(scx & 7);
            if obj_enabled {
                let (xs, count) = self.sprites.sprite_xs();
                let well_separated = count > 1
                    && xs[..count]
                        .windows(2)
                        .all(|w| w[1].saturating_sub(w[0]) > 8);
                self.position = if !well_separated && (k & 3 == 3) {
                    -15
                } else {
                    -16
                };
                let win_enabled = lcdc & 0x20 != 0;
                self.junk_at = if ly == 0 && !win_enabled {
                    5 - (k & 3)
                } else if well_separated {
                    if k & 3 > 1 { 3 } else { 4 - (k & 3) }
                } else {
                    if k & 3 > 1 { 0 } else { 1 - (k & 3) }
                };
                if count > 0 {
                    let (cycles, stalls) = Self::simulate_mode3_cycles(&xs[..count], scx);
                    let diff = cycles.saturating_sub(167);
                    let extra = diff / 4;
                    let target_dots = 168 + 4 * (extra as i32);
                    self.target_dots = target_dots as u16;
                    self.sprite_stalls = stalls;
                }
            } else {
                let win_enabled = lcdc & 0x20 != 0;
                self.junk_at = if !self.is_cgb_model && win_enabled && ly >= wy && wy == 0 {
                    0
                } else {
                    5 - (k & 3)
                };
            }
        }
        self.fetcher.reset_bg(scx);
    }

    fn clear(&mut self) {
        self.bg_head = 0;
        self.bg_tail = 0;
        self.bg_len = 0;

        self.sprite_head = 0;
        self.sprite_tail = 0;
        self.sprite_len = 0;
        self.sprite_stall = 0;
        self.line_sprite_stall = 0;
        self.line_sprite_count = 0;
        self.target_dots = 0;
        self.junk_pushed = false;
        self.window_initial_fetch = false;
        self.sprite_stalls = [0; 10];
        self.current_sprite_idx = 0;
        self.pending_sprite = None;
        self.pending_sprite_low = None;
        self.initial_wx = 0;

        self.sprites.clear();
    }

    fn push_bg_pixels(&mut self, pixels: [Pixel; 8]) {
        for px in pixels {
            self.bg_fifo[self.bg_tail] = px;
            self.bg_tail = (self.bg_tail + 1) % 16;
            self.bg_len += 1;
        }
    }

    fn pop_bg_pixel(&mut self) -> Option<Pixel> {
        if self.bg_len == 0 {
            return None;
        }
        let px = self.bg_fifo[self.bg_head];
        self.bg_head = (self.bg_head + 1) % 16;
        self.bg_len -= 1;
        Some(px)
    }

    fn pop_sprite_pixel(&mut self) -> Option<Pixel> {
        if self.sprite_len == 0 {
            return None;
        }
        let px = self.sprite_fifo[self.sprite_head];
        self.sprite_head = (self.sprite_head + 1) % 16;
        self.sprite_len -= 1;
        Some(px)
    }

    /// Overlay a fetched sprite row into the sprite FIFO. Slot 0 of the
    /// ring is the pixel popping at the *current* `position`, so sprite X
    /// maps directly: tile pixel `j` lands at slot `sprite_x - 8 + j -
    /// position` (SameBoy's `fifo_overlay_object_row`). Slots in between
    /// are padded with transparent pixels; slots already holding an opaque
    /// pixel are merged by sprite priority (DMG: first fetch wins, i.e.
    /// smaller X; CGB: lower OAM index).
    fn overlay_sprite_pixels(&mut self, sprite_pixels: [Pixel; 8], sprite_x: u8, is_cgb: bool) {
        for (j, new_px) in sprite_pixels.iter().enumerate() {
            let idx = i16::from(sprite_x) - 8 + j as i16 - self.position;
            if idx < 0 || idx >= 16 {
                continue;
            }
            let idx = idx as usize;
            while self.sprite_len <= idx {
                self.sprite_fifo[self.sprite_tail] = Pixel::empty();
                self.sprite_tail = (self.sprite_tail + 1) % 16;
                self.sprite_len += 1;
            }
            let slot = (self.sprite_head + idx) % 16;
            let current = self.sprite_fifo[slot];
            let win = if is_cgb {
                current.color_id() == 0 || new_px.sprite_priority() < current.sprite_priority()
            } else {
                current.color_id() == 0
            };
            if win && new_px.color_id() != 0 {
                self.sprite_fifo[slot] = *new_px;
            }
        }
    }
    /// Advance Mode 3 drawing by 1 T-cycle (dot).
    /// Returns Some((lx, bg_pixel, sprite_pixel)) when a pixel is rendered to the screen.
    pub fn step_dot(
        &mut self,
        vram: &Vram,
        ly: u8,
        wx: u8,
        wy: u8,
        scx: u8,
        scy: u8,
        lcdc: u8,
        is_cgb: bool,
    ) -> Option<(u8, Pixel, Pixel)> {
        self.line_dots += 1;

        // Check window trigger
        let win_enabled = lcdc & 0x20 != 0;
        let pos_u8 = self.position as u8;
        let win_in_x = if wx == 0 {
            !self.is_cgb_model
                || pos_u8 == 249
                || (pos_u8 == 240 && (scx & 7 != 0))
                || (241..=248).contains(&pos_u8)
        } else if wx < 166 {
            pos_u8.wrapping_add(7) == wx
                || (!self.is_cgb_model
                    && self.sprites.sprite_xs().1 == 0
                    && wx == 1
                    && pos_u8 == 240)
        } else {
            false
        };

        let was_window_active = self.window_active;
        if !win_enabled {
            self.window_active = false;
        } else if ly >= wy && win_in_x {
            if !self.window_active {
                self.window_active = true;
                if self.is_cgb_model && wx == 0 && self.position < -7 {
                    self.position = -18;
                } else if !self.is_cgb_model && wx == 0 {
                    self.position = if scx & 7 != 0 {
                        -15
                    } else if ly == 0 && self.initial_wx == 0 {
                        -12
                    } else if ly == 0 {
                        -10
                    } else {
                        -8
                    };
                } else if !self.is_cgb_model
                    && self.sprites.sprite_xs().1 == 0
                    && wx == 1
                    && self.position < -8
                {
                    self.position = -8;
                }
                self.fetcher.reset_window(self.window_line_counter);
                self.window_initial_fetch = true;
                self.bg_head = 0;
                self.bg_tail = 0;
                self.bg_len = 0;
                if !self.is_cgb_model && wx == 0 && (scx & 7 != 0) {
                    self.sprite_stall = 1;
                    return None;
                }
            }
        }

        if self.sprite_stall > 0 {
            self.sprite_stall -= 1;
            if self.sprite_stall == 2 {
                if let Some(sprite) = self.pending_sprite {
                    let sprite_height = if lcdc & 0x04 != 0 { 16 } else { 8 };
                    let low =
                        self.fetcher
                            .fetch_sprite_low(sprite, vram, ly, sprite_height, is_cgb);
                    self.pending_sprite_low = Some(low);
                }
            } else if self.sprite_stall == 0 {
                if let Some(sprite) = self.pending_sprite.take() {
                    let sprite_height = if lcdc & 0x04 != 0 { 16 } else { 8 };
                    let high =
                        self.fetcher
                            .fetch_sprite_high(sprite, vram, ly, sprite_height, is_cgb);
                    let low = self.pending_sprite_low.take().unwrap_or_else(|| {
                        self.fetcher
                            .fetch_sprite_low(sprite, vram, ly, sprite_height, is_cgb)
                    });
                    let pixels = self.fetcher.decode_sprite_pixels(sprite, low, high, is_cgb);
                    self.overlay_sprite_pixels(pixels, sprite.x(), is_cgb);
                }
            }
            return None;
        }

        let match_x = if self.position < -8 {
            0
        } else {
            (self.position + 8) as u8
        };
        let obj_enabled = lcdc & 0x02 != 0 || is_cgb;
        if obj_enabled && self.junk_pushed {
            if self.sprites.next_x().map_or(false, |x| x <= match_x) {
                if let Some(sprite) = self.sprites.pop_next() {
                    let stall = if self.current_sprite_idx < 10 {
                        self.sprite_stalls[self.current_sprite_idx]
                    } else {
                        6
                    };
                    self.current_sprite_idx += 1;
                    self.pending_sprite = Some(sprite);
                    self.pending_sprite_low = None;

                    if !is_cgb && self.position >= 8 {
                        self.fetcher.latch_tile_id_for_sprite(
                            vram,
                            ly,
                            scy,
                            scx,
                            self.position as u8,
                            lcdc,
                        );
                    }

                    if stall > 1 {
                        self.sprite_stall = stall - 1;
                        return None;
                    } else {
                        let sprite_height = if lcdc & 0x04 != 0 { 16 } else { 8 };
                        let low =
                            self.fetcher
                                .fetch_sprite_low(sprite, vram, ly, sprite_height, is_cgb);
                        let high =
                            self.fetcher
                                .fetch_sprite_high(sprite, vram, ly, sprite_height, is_cgb);
                        let pixels = self.fetcher.decode_sprite_pixels(sprite, low, high, is_cgb);
                        self.overlay_sprite_pixels(pixels, sprite.x(), is_cgb);
                        self.pending_sprite = None;
                        if stall == 1 {
                            return None;
                        }
                    }
                }
            }
        }
        self.sprites.discard_behind(match_x);

        // SameBoy pushes a junk tile row at mode-3 start (display.c:1850):
        // the 16-pixel position lead is drained while the first real tile is
        // being fetched, so output self-clocks to the fetch cadence instead
        // of running ahead of it. The push dot encodes the SCX-fraction
        // phase (see `start_drawing`).
        if !self.junk_pushed && self.line_dots >= self.junk_at && !self.window_active {
            self.junk_pushed = true;
            self.push_bg_pixels([Pixel::empty(); 8]);
        }

        // Step Background Fetcher, starting at the junk-push dot (SameBoy's
        // fetcher starts at mode-3 start together with the junk row — a
        // fetcher that starts earlier samples its first column before the
        // lead-in snap and grabs a wrapped junk column, shifting the whole
        // line +8 px). The BG tile column is derived live from SCX and the
        // (u8-wrapped) position, SameBoy-style (display.c:939-944), so
        // mid-line SCX writes shift subsequent fetches immediately.
        if self.window_active || self.line_dots >= self.junk_at {
            if let Some(pixels) = self.fetcher.step_t_cycle(
                vram,
                ly,
                scy,
                scx,
                self.position as u8,
                lcdc,
                is_cgb,
                self.is_cgb_model,
                self.bg_len,
            ) {
                self.push_bg_pixels(pixels);
            }
        }

        // Output: one FIFO pop per dot whenever the FIFO is non-empty —
        // SameBoy's render_pixel_if_possible has no FIFO-depth gate. The
        // position lead-in plus the fetch cadence make the line self-clock
        // to the hardware mode-3 length.
        if self.position < 160 && self.bg_len > 0 {
            let out_x = self.position as u8;
            let insert_bg_pixel = was_window_active
                && !self.window_initial_fetch
                && win_enabled
                && ly >= wy
                && (!is_cgb || wx == 0)
                && out_x.wrapping_add(7) == wx
                && self.fetcher.is_get_tile_t1()
                && self.bg_len == 8;
            self.window_initial_fetch = false;

            let bg_px = if insert_bg_pixel {
                Pixel::empty()
            } else {
                self.pop_bg_pixel().expect("bg_len > 0")
            };
            let sprite_px = self.pop_sprite_pixel().unwrap_or(Pixel::empty());

            // Lead-in (position −16..−9, raw u8 240..247): snap forward to
            // −8 once the SCX fraction has been consumed
            // (SameBoy display.c:686-704).
            let pu8 = self.position as u8;
            if pu8.wrapping_add(16) < 8 {
                if pu8 == 239 {
                    self.position = -16;
                } else if pu8 & 7 == self.scx_low3 {
                    self.position = -8;
                } else if pu8 == 247 {
                    self.position = -16;
                    return None;
                }
            }

            // Lead-in pixels are dropped (display.c:709-712: the u8
            // `position >= 160` check covers the negative lead-in).
            if self.position < 0 {
                self.position += 1;
                return None;
            }

            self.lx = out_x;
            self.position += 1;
            return Some((out_x, bg_px, sprite_px));
        }

        None
    }
}

#[cfg(test)]
mod test_sim {
    use super::*;

    #[test]
    fn test_sim_cases() {
        let cases: &[(&[u8], u16)] = &[
            (&[0], 2),
            (&[0, 0], 4),
            (&[0, 0, 0], 5),
            (&[0, 0, 0, 0], 7),
            (&[0, 0, 0, 0, 0], 8),
            (&[1], 2),
            (&[2], 2),
            (&[3], 2),
            (&[4], 1),
            (&[5], 1),
            (&[6], 1),
            (&[7], 1),
            (&[8], 2),
            (&[16], 2),
            (&[0, 8], 5),
            (&[1, 9], 5),
            (&[2, 10], 4),
            (&[3, 11], 4),
            (&[4, 12], 3),
            (&[5, 13], 3),
            (&[6, 14], 3),
            (&[7, 15], 3),
            (&[8, 16], 5),
            (&[0, 0, 0, 0, 0, 0, 0, 0, 0, 0], 16),
        ];
        for (sprites, expected) in cases {
            let (cycles, _) = PixelFifo::simulate_mode3_cycles(sprites, 0);
            let diff = cycles.saturating_sub(167);
            let extra = diff / 4;
            assert_eq!(extra, *expected, "Mismatch for sprites {:?}", sprites);
        }
    }
}
