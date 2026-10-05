//! The line and frame state machine.

use {
    super::{LINE_LENGTH, LINES, MODE2_LENGTH, mode3::Mode3Flow, model_ge_cgb_d},
    crate::{
        interrupts::Interrupts,
        ppu::{Ppu, STAT_IF_OAM_B, STAT_MODE_B, oam_bug::NO_ROW},
    },
};

impl Ppu {
    #[expect(
        clippy::too_many_lines,
        reason = "SameBoy's display coroutine: one arm per sleep"
    )]
    pub(super) fn step_state_machine(&mut self, ints: &mut Interrupts) {
        // Sleeping: `wait` dots remain, resume when it reaches zero.
        if self.d.wait > 0 {
            self.d.wait -= 1;
            if self.d.wait > 0 {
                return;
            }
        }

        let mut state = self.d.state;
        loop {
            match state {
                // ---- start of a frame after LCD on ----
                0 => {
                    if !self.hw_cgb() {
                        self.sleep(23, 1);
                        return;
                    }
                    state = 23;
                }
                23 => {
                    // Handle mode 2 on the very first line 0.
                    self.d.current_line = 0;
                    self.d.window.line = 0xFF;
                    self.d.window.wy_triggered = false;
                    self.d.position_in_line = 240;
                    self.d.line_has_fractional_scrolling = false;
                    self.d.irq.ly_for_comparison = 0;
                    self.stat &= !STAT_MODE_B;
                    self.d.irq.mode_for_interrupt = -1;
                    self.d.cpu.oam_read_blocked = false;
                    self.d.cpu.vram_read_blocked = false;
                    self.d.cpu.oam_write_blocked = false;
                    self.d.cpu.vram_write_blocked = false;
                    self.d.cpu.cgb_palettes_blocked = false;
                    self.d.cfl = MODE2_LENGTH - 4;
                    self.d.line_clock = 0;
                    self.stat_update(ints);
                    self.sleep(2, MODE2_LENGTH - 4);
                    return;
                }
                2 => {
                    self.d.cpu.oam_write_blocked = true;
                    self.d.cfl += 2;
                    self.stat_update(ints);
                    self.sleep(34, 2);
                    return;
                }
                34 => {
                    self.d.objs.count = 0;
                    self.d.objs.found = 0;
                    // Mode 0 is shorter on the first line 0.
                    self.d.cfl += 8;
                    self.stat = (self.stat & !STAT_MODE_B) | 3;
                    self.d.irq.mode_for_interrupt = 3;
                    self.d.cpu.oam_write_blocked = true;
                    self.d.cpu.oam_read_blocked = true;
                    self.d.cpu.vram_read_blocked = self.double_speed();
                    self.d.cpu.vram_write_blocked = self.double_speed();
                    if !self.hw_cgb() {
                        self.d.cpu.vram_read_blocked = true;
                        self.d.cpu.vram_write_blocked = true;
                    }
                    self.d.cfl += 2;
                    self.sleep(37, 2);
                    return;
                }
                37 => {
                    self.d.cpu.cgb_palettes_blocked = true;
                    self.d.cfl += 3;
                    self.sleep(38, 3);
                    return;
                }
                38 => {
                    self.d.cpu.vram_read_blocked = true;
                    self.d.cpu.vram_write_blocked = true;
                    self.d.window.wx_triggered = false;
                    self.mode3_start();
                    state = 200;
                }

                // ---- lines 0..=143 ----
                35 => {
                    self.d.cpu.oam_write_blocked = self.hw_cgb();
                    self.sleep(6, 1);
                    return;
                }
                6 => {
                    self.ly = self.d.current_line;
                    self.d.cpu.oam_read_blocked =
                        !self.double_speed() || model_ge_cgb_d(self.model);
                    self.d.irq.ly_for_comparison = if self.d.current_line != 0 { -1 } else { 0 };
                    // The OAM STAT interrupt occurs 1 T-cycle before STAT
                    // actually changes, except on line 0.
                    if self.d.current_line != 0 {
                        self.d.irq.mode_for_interrupt = 2;
                        self.stat &= !STAT_MODE_B;
                    } else if !self.hw_cgb() {
                        self.stat &= !STAT_MODE_B;
                    } else {
                        // CGB line 0: STAT keeps its mode bits.
                    }
                    self.stat_update(ints);
                    self.sleep(7, 1);
                    return;
                }
                7 => {
                    self.d.cpu.oam_read_blocked = true;
                    self.d.cpu.oam_write_blocked = true;
                    self.stat = (self.stat & !STAT_MODE_B) | 2;
                    self.d.irq.mode_for_interrupt = 2;
                    self.d.irq.ly_for_comparison = i32::from(self.d.current_line);
                    self.wy_check();
                    self.stat_update(ints);
                    self.d.irq.mode_for_interrupt = -1;
                    self.stat_update(ints);
                    self.d.objs.count = 0;
                    self.d.objs.found = 0;
                    self.d.objs.index = 0;
                    state = 201;
                }
                201 => {
                    // OAM search loop head: CGB adds the object before the sleep.
                    if self.hw_cgb() {
                        self.add_object_from_index(self.d.objs.index);
                    }
                    self.sleep(8, 2);
                    return;
                }
                8 => {
                    if !self.hw_cgb() {
                        self.add_object_from_index(self.d.objs.index);
                        self.d.objs.accessed_oam_row = (self.d.objs.index & !1) * 4 + 8;
                    }
                    if self.d.objs.index == 37 {
                        self.d.cpu.vram_read_blocked = !self.hw_cgb();
                        self.d.cpu.vram_write_blocked = false;
                        self.d.cpu.cgb_palettes_blocked = false;
                        self.d.cpu.oam_write_blocked = self.hw_cgb();
                    }
                    self.d.objs.index += 1;
                    if self.d.objs.index < 40 {
                        state = 201;
                    } else {
                        self.d.cfl = MODE2_LENGTH + 4;
                        self.d.objs.accessed_oam_row = NO_ROW;
                        self.d.objs.found = self.d.objs.count;
                        self.stat = (self.stat & !STAT_MODE_B) | 3;
                        self.d.irq.mode_for_interrupt = 3;
                        self.d.cpu.vram_read_blocked = true;
                        self.d.cpu.vram_write_blocked = true;
                        self.d.cpu.cgb_palettes_blocked = false;
                        self.d.cpu.oam_write_blocked = true;
                        self.d.cpu.oam_read_blocked = true;
                        self.stat_update(ints);
                        self.d.cfl += 3;
                        self.sleep(10, 3);
                        return;
                    }
                }
                10 => {
                    self.d.cpu.cgb_palettes_blocked = true;
                    self.d.cfl += 2;
                    self.sleep(32, 2);
                    return;
                }
                32 => {
                    self.mode3_start();
                    state = 200;
                }

                // ---- mode 3 ----
                200 | 42 | 27 | 41 | 20 | 39 | 40 | 21 => {
                    let entry = if state == 200 { 0 } else { state };
                    match self.mode3(ints, entry) {
                        Mode3Flow::Slept => return,
                        Mode3Flow::Done => {
                            self.mode3_done();
                            return;
                        }
                    }
                }

                // ---- HBlank ----
                22 => {
                    self.stat &= !STAT_MODE_B;
                    self.d.irq.mode_for_interrupt = 0;
                    self.d.cpu.oam_read_blocked = false;
                    self.d.cpu.vram_read_blocked = false;
                    self.d.cpu.oam_write_blocked = false;
                    self.d.cpu.vram_write_blocked = false;
                    self.stat_update(ints);
                    self.d.cfl += 2;
                    self.sleep(33, 2);
                    return;
                }
                33 => {
                    self.d.cpu.cgb_palettes_blocked = !self.double_speed();
                    self.d.hblank_hdma_edge = true;
                    self.d.cfl += 2;
                    self.sleep(36, 2);
                    return;
                }
                36 => {
                    self.d.cpu.cgb_palettes_blocked = false;
                    if self.d.cfl > LINE_LENGTH - 2 {
                        self.d.cfl = 0;
                        self.sleep(43, LINE_LENGTH);
                    } else {
                        let saved = self.d.cfl;
                        self.d.cfl = 0;
                        self.sleep(11, LINE_LENGTH - saved - 2);
                    }
                    return;
                }
                43 | 9 => {
                    // display9: mode 3 abort.
                    self.fill_desynced_line();
                    self.d.objs.count = self.d.objs.found;
                    self.d.current_line += 1;
                    self.wy_check();
                    self.d.cfl = 0;
                    if self.d.current_line != LINES {
                        self.d.cfl = 2;
                        self.sleep(28, 2);
                        return;
                    }
                    let p = self.d.position_in_line;
                    if (156..240).contains(&p) {
                        self.d.irq.delayed_glitch_hblank_interrupt = true;
                    }
                    self.d.position_in_line = 240;
                    self.d.line_has_fractional_scrolling = false;
                    state = 210;
                }
                28 => {
                    self.ly = self.d.current_line;
                    let p = self.d.position_in_line;
                    if (156..240).contains(&p) {
                        self.d.irq.delayed_glitch_hblank_interrupt = true;
                    }
                    self.stat_update(ints);
                    self.d.position_in_line = 241;
                    self.mode3_start();
                    state = 200;
                }
                11 => {
                    self.d.cfl = 0;
                    self.d.line_clock = 0;
                    self.sleep(31, 2);
                    return;
                }
                31 => {
                    if self.d.current_line != LINES - 1 {
                        self.d.irq.mode_for_interrupt = 2;
                    }
                    self.d.current_line += 1;
                    if self.d.current_line < LINES {
                        self.line_start();
                        return;
                    }
                    state = 210;
                }

                // ---- lines 144..=152 ----
                210 => {
                    self.d.irq.ly_for_comparison = -1;
                    self.stat_update(ints);
                    self.sleep(26, 2);
                    return;
                }
                26 => {
                    self.ly = self.d.current_line;
                    if self.d.current_line == LINES
                        && !self.d.irq.stat_interrupt_line
                        && self.stat & STAT_IF_OAM_B != 0
                    {
                        ints.request_lcd();
                    }
                    self.sleep(12, 2);
                    return;
                }
                12 => {
                    if self.d.irq.delayed_glitch_hblank_interrupt {
                        self.d.irq.delayed_glitch_hblank_interrupt = false;
                        self.d.irq.mode_for_interrupt = 0;
                    }
                    self.d.irq.ly_for_comparison = i32::from(self.d.current_line);
                    self.stat_update(ints);
                    self.sleep(24, 1);
                    return;
                }
                24 => {
                    if self.d.current_line == LINES {
                        // Entering VBlank also triggers the OAM interrupt.
                        self.stat &= !STAT_MODE_B;
                        self.stat |= 1;
                        ints.request_vblank();
                        if !self.d.irq.stat_interrupt_line && self.stat & STAT_IF_OAM_B != 0 {
                            ints.request_lcd();
                        }
                        self.d.irq.mode_for_interrupt = 1;
                        self.stat_update(ints);
                        self.present_frame();
                    }
                    self.sleep(13, LINE_LENGTH - 5);
                    return;
                }
                13 => {
                    self.d.current_line += 1;
                    if self.d.current_line < 153 {
                        state = 210;
                    } else {
                        state = 220;
                    }
                }

                // ---- line 153 ----
                220 => {
                    self.d.irq.ly_for_comparison = -1;
                    self.stat_update(ints);
                    self.sleep(19, 2);
                    return;
                }
                19 => {
                    self.ly = 153;
                    self.sleep(14, if model_ge_cgb_d(self.model) { 2 } else { 4 });
                    return;
                }
                14 => {
                    if !model_ge_cgb_d(self.model) && !self.double_speed() {
                        self.ly = 0;
                    }
                    self.d.irq.ly_for_comparison = 153;
                    self.stat_update(ints);
                    self.sleep(15, if model_ge_cgb_d(self.model) { 4 } else { 2 });
                    return;
                }
                15 => {
                    self.ly = 0;
                    self.d.irq.ly_for_comparison =
                        if model_ge_cgb_d(self.model) || self.double_speed() {
                            153
                        } else {
                            -1
                        };
                    self.stat_update(ints);
                    self.sleep(16, 4);
                    return;
                }
                16 => {
                    self.d.irq.ly_for_comparison = 0;
                    self.stat_update(ints);
                    // Writing to LYC during this period on a CGB has side effects.
                    self.sleep(29, 12);
                    return;
                }
                29 => {
                    self.sleep(17, LINE_LENGTH - 24);
                    return;
                }
                17 => {
                    self.d.current_line = 0;
                    self.d.window.wy_triggered = false;
                    self.line_start();
                    return;
                }

                _ => {
                    // Unknown state: restart the machine.
                    state = 0;
                }
            }
        }
    }
}
