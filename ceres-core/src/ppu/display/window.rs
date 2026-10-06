//! The window: the WY trigger, the WX start and their glitches.

use {
    super::{fetcher::FetcherStep, state::State},
    crate::ppu::Ppu,
};

#[expect(
    clippy::struct_excessive_bools,
    reason = "Independent trigger and glitch flags"
)]
#[derive(Clone)]
pub struct Window {
    /// The window line being drawn (0xFF before the first one).
    pub line: u8,
    pub tile_x: u8,
    /// The window started on this line.
    pub wx_triggered: bool,
    /// WY matched LY in this frame.
    pub wy_triggered: bool,
    pub being_fetched: bool,
    pub wy_check_scheduled: bool,
    /// SameBoy's `wy_check_modulo`: time since the LCD was turned on modulo 8,
    /// in units of half a dot (a T-cycle in double speed, two per dot in
    /// single speed).
    pub wy_units: u8,
    pub wy_just_checked: bool,
    pub cgb_wx_glitch: bool,
    pub no_pixel_insertion_glitch: bool,
    pub wx_166_interrupt_glitch: bool,
    /// A CPU write to WX is landing (SameBoy's `wx_just_changed`).
    pub wx_just_changed: bool,
}

impl Default for Window {
    fn default() -> Self {
        Self {
            line: 0xFF,
            tile_x: 0,
            wx_triggered: false,
            wy_triggered: false,
            being_fetched: false,
            wy_check_scheduled: false,
            wy_units: 0,
            wy_just_checked: false,
            cgb_wx_glitch: false,
            no_pixel_insertion_glitch: false,
            wx_166_interrupt_glitch: false,
            wx_just_changed: false,
        }
    }
}

impl Ppu {
    pub(super) fn wy_check(&mut self) {
        if self.lcdc & 0x80 == 0 {
            return;
        }
        let comparison =
            if (!self.hw_cgb() || self.double_speed()) && self.d.irq.ly_for_comparison != -1 {
                i32::from(self.d.irq.ly_for_comparison.to_le_bytes()[0])
            } else {
                i32::from(self.d.current_line)
            };
        if self.lcdc & 0x20 != 0 && i32::from(self.wy) == comparison {
            self.d.window.wy_triggered = true;
        }
    }

    /// Time passes by `units` half-dots. SameBoy runs the scheduled WY check
    /// on a grid of 8 of them (counted from the moment the LCD was turned on),
    /// which sits at a different offset in each speed and hardware.
    pub(in crate::ppu) fn advance_wy_units(&mut self, units: u8) {
        for _ in 0..units {
            self.d.window.wy_units = (self.d.window.wy_units + 1) & 7;
            if self.d.window.wy_check_scheduled && !self.d.window.wy_triggered {
                let offset = if self.double_speed {
                    6
                } else if self.hw_cgb() {
                    0
                } else {
                    2
                };
                if (self.d.window.wy_units + offset).trailing_zeros() >= 3 {
                    self.d.window.wy_check_scheduled = false;
                    self.wy_check();
                    if self.d.state == State::Mode3Pixel && self.hw_cgb() && !self.double_speed() {
                        self.d.window.wy_just_checked = true;
                    }
                }
            }
        }
    }

    pub(super) fn update_wx_glitch(&mut self) {
        if !self.hw_cgb() {
            return;
        }
        if self.lcdc & 0x20 == 0 || !self.d.window.wy_triggered {
            self.d.window.cgb_wx_glitch = false;
            return;
        }
        let position = self.d.position_in_line;
        if self.wx == 0 {
            // (position + 16 <= 8) in u8 arithmetic
            self.d.window.cgb_wx_glitch = position.wrapping_add(16) <= 8
                || (position == 249 && self.d.line_has_fractional_scrolling);
            return;
        }
        self.d.window.cgb_wx_glitch = position
            .wrapping_add(7)
            .wrapping_add(u8::from(self.d.window.being_fetched))
            == self.wx;
    }

    /// Window activation check at the top of a mode-3 iteration. Returns
    /// `true` if the engine went to sleep (state 42).
    pub(super) fn mode3_window(&mut self) -> bool {
        self.d.window.wx_166_interrupt_glitch = false;
        if self.d.window.wy_just_checked {
            self.d.window.wy_just_checked = false;
        } else if !self.d.window.wx_triggered && self.d.window.wy_triggered && self.lcdc & 0x20 != 0
        {
            let position = self.d.position_in_line;
            let hw = self.hw_cgb();
            let should_activate = if self.wx == 0 {
                position == 249
                    || position == 240 && self.scx & 7 != 0
                    || (241..=248).contains(&position)
            } else if u16::from(self.wx) < 166 + u16::from(hw) {
                if self.wx == position.wrapping_add(7) {
                    true
                } else if !hw
                    && self.wx == position.wrapping_add(6)
                    && !self.d.window.wx_just_changed
                {
                    // LCD-PPU horizontal desync on DMG units.
                    if self.is_dmg_family() && self.d.lcd_x > 0 {
                        self.d.lcd_x -= 1;
                    }
                    true
                } else {
                    false
                }
            } else {
                false
            };

            if should_activate {
                self.d.window.line = self.d.window.line.wrapping_add(1);
                self.d.window.tile_x = 0;
                self.d.bg_fifo.clear();
                if self.wx == 0 && self.scx & 7 != 0 && !hw {
                    self.d.cfl += 1;
                    self.sleep(State::Mode3WindowDelay, 1);
                    return true;
                } else if self.wx == 166 {
                    self.d.window.wx_166_interrupt_glitch = true;
                } else {
                    // Nothing special about this window start.
                }
                self.mode3_window_activated();
            } else if !hw && self.wx == 166 && self.wx == position.wrapping_add(7) {
                self.d.window.line = self.d.window.line.wrapping_add(1);
            } else {
                // The window does not start here.
            }
        } else {
            // The window is already on, or is not being checked.
        }
        false
    }

    pub(super) const fn mode3_window_activated(&mut self) {
        self.d.window.wx_triggered = true;
        self.d.fetcher.step = FetcherStep::GetTileT1;
        self.d.window.being_fetched = true;
    }

    /// Called by the CPU's LCDC write handler: disabling the window while a
    /// window tile is being fetched suppresses the pixel-insertion glitch.
    pub fn note_window_disable(&mut self, old: u8, new: u8) {
        if old & 0x20 != 0 && new & 0x20 == 0 && self.d.window.being_fetched {
            self.d.window.no_pixel_insertion_glitch = true;
        }
    }
}
