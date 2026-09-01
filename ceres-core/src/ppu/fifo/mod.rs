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
        }
    }

    /// Whether the scanline's 160th pixel has been reached (mode 3 over).
    #[must_use]
    pub const fn line_done(&self) -> bool {
        self.position >= 160
    }

    pub fn set_scx(&mut self, scx: u8) {
        // The SCX fraction is sampled live per pop during the lead-in (see
        // `step_dot`), so mid-line writes need no bookkeeping here.
        self.fetcher.set_bg_tile_x(scx);
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

    pub fn start_drawing(&mut self, scx: u8) {
        self.lx = 0;
        self.line_dots = 0;
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
        self.junk_at = 6 - ((k + 1) & 3);
        self.fetcher.reset_bg(scx);
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
        let win_in_x = self.lx.wrapping_add(7) >= wx;
        if win_enabled && ly >= wy && win_in_x {
            if !self.window_active {
                self.window_active = true;
                self.fetcher.reset_window(self.window_line_counter);
                self.bg_head = 0;
                self.bg_tail = 0;
                self.bg_len = 0;
            }
        }

        // Object (sprite) fetch, mirroring SameBoy's mode 3 loop
        // (display.c:1942-2026). Sprites are matched against the PPU's X
        // position + 8 (`x_for_object_match`); sprites behind that point can
        // no longer be fetched this line and are dropped. When a sprite
        // matches, the fetcher first finishes the BG tile row it is on
        // (pixel output pauses meanwhile — never mid-row), then each
        // matching sprite costs a 6-dot fetch that stalls the BG fetcher
        // and pixel output. Unlike the previous exact-lx + ready-state
        // coincidence trigger, this can never silently drop a sprite.
        // Object (sprite) fetch, mirroring SameBoy's mode 3 loop
        // (display.c:1942-2026). Sprites match when `position + 8` reaches
        // their X (`x_for_object_match`), so sprites left of the screen edge
        // are fetched during the lead-in and their off-screen pixels are
        // consumed by dropped lead-in pops. When a sprite matches, the
        // fetcher first finishes the BG tile row it is on (pixel output
        // pauses meanwhile — never mid-row), then each matching sprite
        // costs a 6-dot fetch that stalls the BG fetcher and pixel output.
        // Unlike the previous exact-lx + ready-state coincidence trigger,
        // this can never silently drop a sprite.
        let match_x = self.position + 8;
        self.sprites.discard_behind(match_x);
        let obj_enabled = lcdc & 0x02 != 0 || is_cgb;
        let mut output_paused = false;
        if obj_enabled
            && !self.fetcher.is_fetching_sprite()
            && self.sprites.next_x() == Some(match_x)
        {
            if self.fetcher.is_ready_for_sprite_fetch() {
                let sprite = self.sprites.pop_next().expect("sprite at next_x");
                if std::env::var_os("CERES_TRACE").is_some() && (ly == 0 || ly == 16 || ly == 40) {
                    eprintln!(
                        "SPR x={} tile={} pos={} dot={} slen={} pal={}",
                        sprite.x(),
                        sprite.tile(),
                        sprite.x(),
                        self.position,
                        self.line_dots,
                        self.sprite_len
                    );
                }
                let sprite_height = if lcdc & 0x04 != 0 { 16 } else { 8 };
                let sprite_pixels =
                    self.fetcher
                        .fetch_sprite_data(sprite, vram, ly, sprite_height, is_cgb);
                self.overlay_sprite_pixels(sprite_pixels, sprite.x(), is_cgb);
                // fetch_sprite_data started the 6-dot stall; if more
                // sprites share this X they are fetched on later dots,
                // once the stall expires and the fetcher is still at the
                // tile-row boundary.
            } else {
                // Fetcher mid-row: let this dot finish the row. Pixel
                // output waits so the sprite overlay stays aligned with
                // the sprite's on-screen X.
                output_paused = true;
            }
        }

        // SameBoy pushes a junk tile row at mode-3 start (display.c:1850):
        // the 16-pixel position lead is drained while the first real tile is
        // being fetched, so output self-clocks to the fetch cadence instead
        // of running ahead of it. The push dot encodes the SCX-fraction
        // phase (see `start_drawing`).
        if self.line_dots == self.junk_at && !self.window_active {
            self.push_bg_pixels([Pixel::empty(); 8]);
        }

        // Step Background Fetcher, starting at the junk-push dot (SameBoy's
        // fetcher starts at mode-3 start together with the junk row — a
        // fetcher that starts earlier samples its first column before the
        // lead-in snap and grabs a wrapped junk column, shifting the whole
        // line +8 px). The BG tile column is derived live from SCX and the
        // (u8-wrapped) position, SameBoy-style (display.c:939-944), so
        // mid-line SCX writes shift subsequent fetches immediately.
        if self.line_dots >= self.junk_at {
            if let Some(pixels) = self.fetcher.step_t_cycle(
                vram,
                ly,
                scy,
                scx,
                self.position as u8,
                lcdc,
                is_cgb,
                self.bg_len,
            ) {
                self.push_bg_pixels(pixels);
            }
        }

        // Output: one FIFO pop per dot whenever the FIFO is non-empty —
        // SameBoy's render_pixel_if_possible has no FIFO-depth gate. The
        // position lead-in plus the fetch cadence make the line self-clock
        // to the hardware mode-3 length.
        if !output_paused
            && self.position < 160
            && self.bg_len > 0
            && !self.fetcher.is_fetching_sprite()
        {
            let popped = self.pop_bg_pixel().expect("bg_len > 0");
            let sprite_px = self.pop_sprite_pixel().unwrap_or(Pixel::empty());

            // Lead-in (position −16..−9, raw u8 240..247): snap forward to
            // −8 once the SCX fraction has been consumed
            // (SameBoy display.c:686-704).
            let pu8 = self.position as u8;
            if pu8.wrapping_add(16) < 8 {
                if pu8 == 239 {
                    self.position = -16;
                } else if pu8 & 7 == scx & 7 {
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

            let is_window_glitch = win_enabled
                && ly >= wy
                && self.window_active
                && self.lx > 0
                && wx < 100
                && self.lx.wrapping_add(7) == wx
                && self.fetcher.is_get_tile();
            let bg_px = if is_window_glitch {
                Pixel::empty()
            } else {
                popped
            };

            let out_x = self.position as u8;
            if std::env::var_os("CERES_TRACE").is_some() && out_x == 0 && (ly == 0 || ly == 32) {
                eprintln!("PX0 ly={ly} dot={} k={} junk_at={}", self.line_dots, scx & 7, self.junk_at);
            }
            if std::env::var_os("CERES_TRACE").is_some() && ly == 40 && (60..82).contains(&out_x) {
                eprintln!(
                    "PX x={out_x} dot={} bgcid={} spcid={} spal={}",
                    self.line_dots,
                    bg_px.color_id(),
                    sprite_px.color_id(),
                    sprite_px.palette()
                );
            }
            self.lx = out_x;
            self.position += 1;
            return Some((out_x, bg_px, sprite_px));
        }

        None
    }
}
