//! Mode 3: the loop that fetches objects and pushes pixels to the LCD.

use {
    super::{LINES, fetcher::FetcherStep, model_ge_cgb_d},
    crate::{
        interrupts::Interrupts,
        ppu::{PX_WIDTH, Ppu, STAT_MODE_B},
    },
};

pub(super) enum Mode3Flow {
    Slept,
    Done,
}

impl Ppu {
    pub(super) fn mode3_start(&mut self) {
        self.d.window.no_pixel_insertion_glitch = false;
        self.d.bg_fifo.clear();
        self.d.oam_fifo.clear();
        // Fill the FIFO with 8 pixels of "junk", it's going to be dropped anyway.
        self.d.bg_fifo.push_bg_row(0, 0, 0, false, false);
        self.d.lcd_x = 0;
        self.d.fetcher.step = FetcherStep::GetTileT1;
    }

    /// Runs the mode-3 loop from `entry` until it sleeps or finishes.
    /// Entries: 0 = top of an iteration, 42/27/41/20/39/40/21 = resume
    /// points after the corresponding SameBoy sleep.
    #[expect(
        clippy::too_many_lines,
        reason = "SameBoy's mode 3 loop: one arm per resume point"
    )]
    pub(super) fn mode3(&mut self, ints: &mut Interrupts, mut entry: u8) -> Mode3Flow {
        loop {
            match entry {
                0 => {
                    if self.mode3_window() {
                        return Mode3Flow::Slept;
                    }
                    entry = 100;
                }
                42 => {
                    self.mode3_window_activated();
                    entry = 100;
                }
                100 => {
                    // Insert a pixel right at the FIFO's end.
                    let hw = self.hw_cgb();
                    if self.wx == self.d.position_in_line.wrapping_add(7)
                        && (!hw || self.wx == 0)
                        && self.d.window.wx_triggered
                        && !self.d.window.being_fetched
                        && self.d.fetcher.step == FetcherStep::GetTileT1
                        && self.d.bg_fifo.size == 8
                    {
                        self.d.insert_bg_pixel = true;
                    }

                    // Handle objects.
                    while self.d.objs.count != 0
                        && self.d.objs.x[self.d.objs.count - 1] < self.x_for_object_match()
                    {
                        self.d.objs.count -= 1;
                    }
                    self.d.obj_fetch.active = true;
                    entry = 101;
                }
                101 => {
                    let n = self.d.objs.count;
                    entry = if n != 0
                        && (self.lcdc & 0x02 != 0 || self.hw_cgb())
                        && self.d.objs.x[n - 1] == self.x_for_object_match()
                    {
                        102
                    } else {
                        130
                    };
                }
                102 => {
                    if self.d.fetcher.step < FetcherStep::DataHighT2 || self.d.bg_fifo.size == 0 {
                        self.advance_fetcher();
                        self.d.cfl += 1;
                        self.sleep(27, 1);
                        return Mode3Flow::Slept;
                    }
                    entry = 103;
                }
                27 => entry = if self.d.obj_fetch.aborted { 130 } else { 102 },
                103 => {
                    self.advance_fetcher();
                    self.d.cfl += 1;
                    self.sleep(41, 1);
                    return Mode3Flow::Slept;
                }
                41 => entry = if self.d.obj_fetch.aborted { 130 } else { 104 },
                104 => {
                    self.advance_fetcher();
                    let base = u16::from(self.d.objs.indices[self.d.objs.count - 1]) * 4;
                    self.d.objs.y_bus = self.oam_read(base + 2);
                    self.d.obj_fetch.flags = self.oam_read(base + 3);
                    self.d.cfl += 2;
                    self.sleep(20, 2);
                    return Mode3Flow::Slept;
                }
                20 => entry = if self.d.obj_fetch.aborted { 130 } else { 105 },
                105 => {
                    let n = self.d.objs.count;
                    self.d.obj_fetch.line_address = self.object_line_address(
                        self.d.objs.y[n - 1],
                        self.d.objs.y_bus,
                        self.d.obj_fetch.flags,
                    );
                    self.d.obj_fetch.data[0] = self.vram_read(self.d.obj_fetch.line_address);
                    self.d.cfl += 2;
                    self.sleep(39, 2);
                    return Mode3Flow::Slept;
                }
                39 => entry = if self.d.obj_fetch.aborted { 130 } else { 106 },
                106 => {
                    self.d.obj_fetch.active = false;
                    self.d.cfl += 1;
                    let n = self.d.objs.count;
                    self.d.obj_fetch.line_address = self.object_line_address(
                        self.d.objs.y[n - 1],
                        self.d.objs.y_bus,
                        self.d.obj_fetch.flags,
                    );
                    self.d.obj_fetch.data[1] = self.vram_read(self.d.obj_fetch.line_address + 1);
                    self.sleep(40, 1);
                    return Mode3Flow::Slept;
                }
                40 => {
                    let n = self.d.objs.count;
                    let flags = self.d.obj_fetch.flags;
                    let palette = if self.cgb_mode_on() {
                        flags & 0x7
                    } else {
                        u8::from(flags & 0x10 != 0)
                    };
                    let priority = if self.opri_index_priority() {
                        self.d.objs.indices[n - 1]
                    } else {
                        0
                    };
                    let (low, high) = (self.d.obj_fetch.data[0], self.d.obj_fetch.data[1]);
                    self.d.oam_fifo.overlay_object_row(
                        low,
                        high,
                        palette,
                        flags & 0x80 != 0,
                        priority,
                        flags & 0x20 != 0,
                    );
                    self.d.fetcher.sel_glitch_data = if self.d.bus.vram_ppu_blocked {
                        0xFF
                    } else {
                        self.vram_raw(self.d.obj_fetch.line_address + 1)
                    };
                    self.d.objs.count -= 1;
                    entry = 101;
                }
                130 => {
                    // abort_fetching_object:
                    self.d.obj_fetch.aborted = false;
                    self.d.obj_fetch.active = false;
                    if let Some(out) = self.render_pixel_if_possible() {
                        self.output_pixel(out);
                    }
                    self.advance_fetcher();
                    if self.d.position_in_line == 160 {
                        return Mode3Flow::Done;
                    }
                    self.d.cfl += 1;
                    self.sleep(21, 1);
                    return Mode3Flow::Slept;
                }
                21 => {
                    if self.d.window.wx_166_interrupt_glitch {
                        self.d.irq.mode_for_interrupt = 0;
                        self.stat_update(ints);
                    }
                    entry = 0;
                }
                _ => unreachable!(),
            }
        }
    }

    /// Code after the mode-3 loop breaks (`skip_slow_mode_3`).
    pub(super) fn mode3_done(&mut self) {
        self.d.position_in_line = 240;
        self.d.line_has_fractional_scrolling = false;

        if self.d.fetcher.step == FetcherStep::DataHighT1
            || self.d.fetcher.step == FetcherStep::DataHighT2
        {
            // Make sure current_tile_data[1] holds the last tile data byte read.
            self.d.fetcher.data[1] = self.d.fetcher.data[0];
        }

        // The PPU and LCD desynced: fill the rest of the line with the last colour.
        self.fill_desynced_line();

        if self.d.current_line == 143 {
            self.d.window.line = 0xFF;
        }
        if !self.hw_cgb() && self.d.window.wy_triggered && self.lcdc & 0x20 != 0 && self.wx == 166 {
            self.d.window.wx_triggered = true;
            self.d.window.tile_x = 1;
            self.d.window.line = self.d.window.line.wrapping_add(1);
        } else {
            self.d.window.wx_triggered = false;
        }

        if !self.double_speed() {
            self.stat &= !STAT_MODE_B;
            self.d.irq.mode_for_interrupt = 0;
            self.d.cpu.oam_read_blocked = model_ge_cgb_d(self.model);
            self.d.cpu.vram_read_blocked = false;
            self.d.cpu.oam_write_blocked = false;
            self.d.cpu.vram_write_blocked = false;
        }

        self.d.cfl += 1;
        self.sleep(22, 1);
    }

    pub(super) fn fill_desynced_line(&mut self) {
        while self.d.lcd_x < 160 {
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
