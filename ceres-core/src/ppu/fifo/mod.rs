pub mod fetcher;
pub mod pixel;
pub mod sprite;

pub use self::pixel::Pixel;

use self::fetcher::TileFetcher;
use self::sprite::SpriteBuffer;
use crate::ppu::oam::Oam;
use crate::ppu::vram::Vram;

pub struct PixelFifo {
    bg_fifo: [Pixel; 16],
    bg_head: usize,
    bg_tail: usize,
    bg_len: usize,

    sprite_fifo: [Pixel; 8],
    sprite_head: usize,
    sprite_tail: usize,
    sprite_len: usize,

    fetcher: TileFetcher,
    sprites: SpriteBuffer,
    scx_discard: u8,
    lx: u8,
    line_dots: u16,
    window_active: bool,
    window_line_counter: u8,
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

            sprite_fifo: [Pixel::empty(); 8],
            sprite_head: 0,
            sprite_tail: 0,
            sprite_len: 0,

            fetcher: TileFetcher::new(),
            sprites: SpriteBuffer::new(),
            scx_discard: 0,
            lx: 0,
            line_dots: 0,
            window_active: false,
            window_line_counter: 0,
        }
    }

    #[must_use]
    pub const fn lx(&self) -> u8 {
        self.lx
    }

    pub fn start_scanline(
        &mut self,
        oam: &Oam,
        ly: u8,
        scx: u8,
        _scy: u8,
        lcdc: u8,
        is_cgb: bool,
        opri: bool,
    ) {
        self.clear();
        self.lx = 0;
        self.line_dots = 0;
        self.scx_discard = scx & 7;

        let sprite_height = if lcdc & 0x04 != 0 { 16 } else { 8 };
        self.sprites.scan_line(oam, ly, sprite_height, is_cgb, opri);

        self.fetcher.reset_bg();
    }

    fn clear(&mut self) {
        self.bg_head = 0;
        self.bg_tail = 0;
        self.bg_len = 0;

        self.sprite_head = 0;
        self.sprite_tail = 0;
        self.sprite_len = 0;

        self.sprites.clear();
    }

    fn push_bg_pixels(&mut self, pixels: [Pixel; 8]) {
        for px in pixels {
            self.bg_fifo[self.bg_tail] = px;
            self.bg_tail = (self.bg_tail + 1) % 16;
            self.bg_len += 1;
        }
    }

    fn overlay_sprite_pixels(&mut self, sprite_pixels: [Pixel; 8], sprite_x: u8, is_cgb: bool) {
        let cut = if sprite_x < 8 { (8 - sprite_x) as usize } else { 0 };
        // Pad sprite FIFO up to 8 if needed
        while self.sprite_len < 8 {
            self.sprite_fifo[self.sprite_tail] = Pixel::empty();
            self.sprite_tail = (self.sprite_tail + 1) % 8;
            self.sprite_len += 1;
        }

        for (i, new_px) in sprite_pixels.into_iter().skip(cut).enumerate() {
            let slot = (self.sprite_head + i) % 8;
            let current = &mut self.sprite_fifo[slot];

            if new_px.color_id() != 0 {
                if current.color_id() == 0 {
                    *current = new_px;
                } else if !is_cgb && new_px.sprite_priority() < current.sprite_priority() {
                    *current = new_px;
                }
            }
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
        self.sprite_head = (self.sprite_head + 1) % 8;
        self.sprite_len -= 1;
        Some(px)
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
        // Check window trigger
        let win_enabled = lcdc & 0x20 != 0;
        if win_enabled && !self.window_active && ly >= wy && self.lx.wrapping_add(7) >= wx {
            self.window_active = true;
            self.fetcher.reset_window(self.window_line_counter);
            self.bg_head = 0;
            self.bg_tail = 0;
            self.bg_len = 0;
        }

        // Check sprite trigger at current lx
        let obj_enabled = lcdc & 0x02 != 0 || is_cgb;
        if obj_enabled && !self.fetcher.is_fetching_sprite() {
            if let Some(sprite) = self.sprites.take_sprite_at(self.lx) {
                let sprite_height = if lcdc & 0x04 != 0 { 16 } else { 8 };
                let sprite_pixels =
                    self.fetcher
                        .fetch_sprite_data(sprite, vram, ly, sprite_height, is_cgb);
                self.overlay_sprite_pixels(sprite_pixels, sprite.x(), is_cgb);
            }
        }

        // Step Background Fetcher
        if let Some(pixels) = self
            .fetcher
            .step_t_cycle(vram, ly, scx, scy, self.lx, lcdc, is_cgb, self.bg_len)
        {
            self.push_bg_pixels(pixels);
        }

        // Output pixel if FIFO has pixels ready and not stalled by sprite fetch
        if self.bg_len > 8 && !self.fetcher.is_fetching_sprite() {
            if let Some(bg_px) = self.pop_bg_pixel() {
                let sprite_px = self.pop_sprite_pixel().unwrap_or(Pixel::empty());

                if self.scx_discard > 0 {
                    self.scx_discard -= 1;
                    return None;
                }

                if self.lx < 160 {
                    let out_x = self.lx;
                    self.lx += 1;
                    return Some((out_x, bg_px, sprite_px));
                }
            }
        }

        None
    }
}
