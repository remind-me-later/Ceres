//! Mode 3: the loop that fetches objects and pushes pixels to the LCD.

use {
    super::{
        Display, LINES, background::Popped, fetcher::FetcherStep, fifo::Item, model_ge_cgb_d,
        state::State, window::Window,
    },
    crate::ppu::{
        ATTR_CGB_PALETTE, ATTR_DMG_PALETTE_B, ATTR_PRIORITY_B, ATTR_X_FLIP_B, LCDC_OBJ_EN_B,
        LCDC_WIN_EN_B, Oam, PX_WIDTH, Ppu, STAT_MODE_B,
    },
    core::mem,
};

/// Dots the DMG's window start at WX = 166 adds before an object fetch at
/// the line's end: its first tile (6) and the next one, to the data (5).
const WINDOW_TILES_BEFORE_OBJECT: i32 = 11;

pub(super) enum Mode3Flow {
    Slept,
    Done,
}

/// The steps of the mode 3 loop. Each object takes 6 dots or more to fetch.
#[derive(Clone, Copy)]
enum Step {
    /// The top of the loop: the window may start here.
    WindowCheck,
    /// Drops the objects that were passed and starts the object fetch.
    Objects,
    NextObject,
    WaitForTile,
    ReadObjectAttributes,
    ReadObjectLow,
    ReadObjectHigh,
    PushObject,
    /// Pushes a pixel to the LCD (and the end of an aborted object fetch).
    OutputPixel,
    AfterPixel,
}

/// What mode 3's loop asks of the PPU besides the display's state: the
/// window, the fetchers' reads, the object's row and the pixels. `Ppu` does
/// them; the proofs against the netlist (`background::check`) run the loop
/// with the window idle and nothing read.
pub(super) trait Mode3Ppu {
    /// The display's state.
    fn d(&mut self) -> &mut Display;
    /// The objects are drawn (LCDC's bit 1, always on the CGB).
    fn objects_enabled(&self) -> bool;
    /// SCX as the pixel mixer sees it.
    fn mixer_scx(&self) -> u8;
    /// The window's check at the top of the loop: whether it started, and
    /// the loop sleeps (`Mode3WindowDelay`).
    fn window_check(&mut self) -> bool;
    /// The window starts after its delay.
    fn window_activated(&mut self);
    /// A blank pixel inserted at the FIFO's end, as the window starts there.
    fn window_pixel_insertion(&mut self);
    /// The tile fetcher's read of this dot, and the window's glitches its
    /// step takes (`Background::advance`).
    fn fetcher_read(&mut self);
    fn fetcher_glitches(&self) -> (bool, Option<u8>);
    /// The object fetch's reads: its attributes, its row's two bytes.
    fn read_object_attributes(&mut self);
    fn read_object_low(&mut self);
    fn read_object_high(&mut self);
    /// The fetched object's row onto the object FIFO.
    fn push_object(&mut self);
    /// The background FIFO's pixel, mixed and drawn.
    fn mix_pixel(&mut self, item: Item, popped: Popped);
    /// The HBlank interrupt's source, before the STAT mode (`timing.rs`).
    fn signal_mode3_end(&mut self);

    /// The tile fetcher's read, then its step.
    fn advance_fetcher(&mut self) {
        self.fetcher_read();
        let (cgb_wx_glitch, insertion_wx) = self.fetcher_glitches();
        self.d().bg.advance(cgb_wx_glitch, insertion_wx);
    }
}

/// Runs the mode 3 loop from `resume` (the sleep that ended) until it sleeps
/// again or the line is drawn.
#[expect(
    clippy::too_many_lines,
    reason = "SameBoy's mode 3 loop: one arm per step"
)]
pub(super) fn mode3<P: Mode3Ppu>(p: &mut P, resume: State) -> Mode3Flow {
    // On the DMG, disabling objects aborts an object fetch (see
    // `abort_object_fetch_on_obj_disable`).
    let unless_aborted = |ppu: &mut P, step| {
        if ppu.d().obj_fetch.aborted {
            Step::OutputPixel
        } else {
            step
        }
    };
    let mut step = match resume {
        State::Mode3WindowDelay => {
            p.window_activated();
            Step::Objects
        }
        State::Mode3ObjectWait => unless_aborted(p, Step::WaitForTile),
        State::Mode3ObjectFetch => unless_aborted(p, Step::ReadObjectAttributes),
        State::Mode3ObjectAttributes => unless_aborted(p, Step::ReadObjectLow),
        State::Mode3ObjectLow => unless_aborted(p, Step::ReadObjectHigh),
        State::Mode3ObjectHigh => Step::PushObject,
        State::Mode3Pixel => Step::AfterPixel,
        _ => Step::WindowCheck,
    };
    loop {
        step = match step {
            Step::WindowCheck => {
                if p.window_check() {
                    return Mode3Flow::Slept;
                }
                Step::Objects
            }
            Step::Objects => {
                p.window_pixel_insertion();
                // Drop the objects left of the current position.
                let d = p.d();
                while d.objs.next().is_some_and(|e| e.x < d.bg.object_match_x()) {
                    d.objs.count -= 1;
                }
                d.obj_fetch.active = true;
                Step::NextObject
            }
            Step::NextObject => {
                let enabled = p.objects_enabled();
                let d = p.d();
                if let Some(obj) = d.objs.next()
                    && enabled
                    && obj.x == d.bg.object_match_x()
                {
                    if obj.x == 167 {
                        // The HBlank interrupt does not wait for the fetch of an
                        // object at the right edge, unlike the STAT mode.
                        p.signal_mode3_end();
                    }
                    Step::WaitForTile
                } else {
                    Step::OutputPixel
                }
            }
            Step::WaitForTile => {
                let d = p.d();
                if mem::take(&mut d.window.started_after_line) {
                    // The window's start restarted the fetcher: the
                    // object waits for the window's first tile and the
                    // next one (the line ends after it, whatever the
                    // fetcher holds).
                    d.sleep_in_line(State::Mode3ObjectFetch, 1 + WINDOW_TILES_BEFORE_OBJECT);
                    return Mode3Flow::Slept;
                }
                // The background fetcher finishes its tile first.
                let next = if d.bg.fetcher.step < FetcherStep::DataHighT2 || d.bg.fifo.is_empty() {
                    State::Mode3ObjectWait
                } else {
                    State::Mode3ObjectFetch
                };
                p.advance_fetcher();
                p.d().sleep_in_line(next, 1);
                return Mode3Flow::Slept;
            }
            Step::ReadObjectAttributes => {
                p.advance_fetcher();
                p.read_object_attributes();
                p.d().sleep_in_line(State::Mode3ObjectAttributes, 2);
                return Mode3Flow::Slept;
            }
            Step::ReadObjectLow => {
                p.read_object_low();
                p.d().sleep_in_line(State::Mode3ObjectLow, 2);
                return Mode3Flow::Slept;
            }
            Step::ReadObjectHigh => {
                p.d().obj_fetch.active = false;
                p.read_object_high();
                p.d().sleep_in_line(State::Mode3ObjectHigh, 1);
                return Mode3Flow::Slept;
            }
            Step::PushObject => {
                p.push_object();
                p.d().objs.count -= 1;
                Step::NextObject
            }
            Step::OutputPixel => {
                let enabled = p.objects_enabled();
                let scx = p.mixer_scx();
                let d = p.d();
                d.obj_fetch.aborted = false;
                d.obj_fetch.active = false;
                // An object at X 0, not fetched yet, holds the pixels back.
                let held = enabled && d.objs.next().is_some_and(|e| e.x == 0);
                if !held && let Some((item, popped)) = d.bg.pop(scx, d.window.being_fetched) {
                    p.mix_pixel(item, popped);
                }
                p.advance_fetcher();
                if p.d().bg.position == PX_WIDTH {
                    return Mode3Flow::Done;
                }
                p.d().sleep_in_line(State::Mode3Pixel, 1);
                return Mode3Flow::Slept;
            }
            Step::AfterPixel => {
                if p.d().window.wx_166_interrupt_glitch {
                    p.signal_mode3_end();
                }
                Step::WindowCheck
            }
        };
    }
}

impl Mode3Ppu for Ppu {
    #[inline]
    fn d(&mut self) -> &mut Display {
        &mut self.d
    }

    #[inline]
    fn objects_enabled(&self) -> bool {
        self.lcdc & LCDC_OBJ_EN_B != 0 || self.is_cgb_hardware()
    }

    #[inline]
    fn mixer_scx(&self) -> u8 {
        self.mixer_scx
    }

    fn window_check(&mut self) -> bool {
        self.mode3_window()
    }

    fn window_activated(&mut self) {
        self.mode3_window_activated();
    }

    fn window_pixel_insertion(&mut self) {
        // Insert a pixel right at the FIFO's end.
        let hw = self.is_cgb_hardware();
        if self.wx == self.d.bg.position.wrapping_add(7)
            && (!hw || self.wx == 0)
            && self.d.window.wx_triggered
            && !self.d.window.being_fetched
            && self.d.bg.fetcher.step == FetcherStep::GetTileT1
            && self.d.bg.fifo.len() == 8
        {
            self.d.bg.insert_blank = true;
        }
    }

    fn fetcher_read(&mut self) {
        self.read_tile();
    }

    #[inline]
    fn fetcher_glitches(&self) -> (bool, Option<u8>) {
        (self.d.window.cgb_wx_glitch, self.insertion_glitch_wx())
    }

    fn read_object_attributes(&mut self) {
        let base = u16::from(self.d.objs.fetched().index) * Oam::ENTRY_SIZE;
        // The tile and the attributes go through the latches the search
        // reads Y and X into, and stay there.
        self.d.obj_fetch.tile = self.oam_read_fetch(base + Oam::ENTRY_TILE);
        self.d.obj_fetch.flags = self.oam_read_fetch(base + Oam::ENTRY_ATTRIBUTES);
        self.d.objs.y_bus = self.d.obj_fetch.tile;
        self.d.objs.x_bus = self.d.obj_fetch.flags;
    }

    fn read_object_low(&mut self) {
        self.d.obj_fetch.line_address = self.current_object_line_address();
        self.d.obj_fetch.data[0] = self.vram_read(self.d.obj_fetch.line_address);
    }

    fn read_object_high(&mut self) {
        self.d.obj_fetch.line_address = self.current_object_line_address();
        self.d.obj_fetch.data[1] = self.vram_read(self.d.obj_fetch.line_address + 1);
    }

    fn push_object(&mut self) {
        self.overlay_object_row();
    }

    fn mix_pixel(&mut self, item: Item, popped: Popped) {
        if let Some(out) = self.mixed_pixel(item, popped) {
            #[cfg(feature = "debug")]
            self.trace_pixel(out.lx);
            self.output_pixel(out);
        }
    }

    fn signal_mode3_end(&mut self) {
        self.timing_mode3_end();
    }
}

impl Ppu {
    pub(super) fn mode3_start(&mut self) {
        self.d.window.no_pixel_insertion_glitch = false;
        self.d.bg.start();
        self.d.oam_fifo.clear();
        self.d.lcd_x = 0;
        self.d.window.started_after_line = false;
        // The DMG's WX match at the last line's end is held (PYCO, NUNU)
        // until this line's first pixel; LCDC bit 5 holds the window off
        // until then.
        if mem::take(&mut self.d.window.wx_166_carry) && self.lcdc & LCDC_WIN_EN_B != 0 {
            self.d.window.wx_triggered = true;
            self.d.window.tile_x = 1;
            self.d.window.line = self.d.window.line.wrapping_add(1);
        }
    }

    fn current_object_line_address(&self) -> u16 {
        self.object_line_address(
            self.d.objs.fetched().y,
            self.d.obj_fetch.tile,
            self.d.obj_fetch.flags,
        )
    }

    /// Overlays the fetched object row on the object FIFO.
    fn overlay_object_row(&mut self) {
        let flags = self.d.obj_fetch.flags;
        let palette = if self.cgb_mode_on() {
            flags & ATTR_CGB_PALETTE
        } else {
            u8::from(flags & ATTR_DMG_PALETTE_B != 0)
        };
        let priority = if self.opri_index_priority() {
            self.d.objs.fetched().index
        } else {
            0
        };
        let [low, high] = self.d.obj_fetch.data;
        self.d.oam_fifo.overlay_object_row(
            low,
            high,
            palette,
            flags & ATTR_PRIORITY_B != 0,
            priority,
            flags & ATTR_X_FLIP_B != 0,
        );
        self.d.bg.fetcher.sel_glitch_data = if self.d.bus.vram_ppu_blocked {
            0xFF
        } else {
            self.vram_raw(self.d.obj_fetch.line_address + 1)
        };
    }

    /// Code after the mode-3 loop breaks (`skip_slow_mode_3`).
    pub(super) fn mode3_done(&mut self) {
        self.d.bg.end_line();

        // The PPU and LCD desynced: fill the rest of the line with the last colour.
        self.fill_desynced_line();

        if self.d.current_line == LINES - 1 {
            self.d.window.line = Window::NO_LINE;
        }
        self.d.window.wx_triggered = false;
        self.d.window.wx_166_carry =
            !self.is_cgb_hardware() && self.d.window.wy_triggered && self.wx == 166;

        if !self.double_speed() {
            self.stat &= !STAT_MODE_B;
            self.d.cpu.oam_read_blocked = model_ge_cgb_d(self.model);
            self.d.cpu.vram_read_blocked = false;
            self.d.cpu.oam_write_blocked = false;
            self.d.cpu.vram_write_blocked = false;
        }

        self.sleep_in_line(State::HBlankStart, 1);
    }

    pub(super) fn fill_desynced_line(&mut self) {
        while self.d.lcd_x < PX_WIDTH {
            if self.d.current_line < LINES {
                let x = u32::from(self.d.lcd_x);
                let base = u32::from(self.d.current_line) * u32::from(PX_WIDTH);
                if x == 0 {
                    let rgb = self.desync_color();
                    self.rgb_buf.set_px(base, rgb);
                } else {
                    let rgb = self.rgb_buf.px(base + x - 1);
                    self.rgb_buf.set_px(base + x, rgb);
                }
            }
            self.d.lcd_x += 1;
        }
    }
}
