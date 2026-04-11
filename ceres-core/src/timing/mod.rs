use crate::{AudioCallback, Gb};
use core::time::Duration;

#[cfg(test)]
mod tests;

/// T-cycles per frame (4MHz rate).
pub const DOTS_PER_FRAME: i32 = 70224;
/// T-cycles per second (4MHz).
pub const DOTS_PER_SEC: i32 = 1 << 22;
pub const FRAME_DURATION: Duration = Duration::new(0, 16_742_706); // DOTS_PER_FRAME / DOTS_PER_SEC

/// PPU cycles per T-cycle.
/// Set to 1 for T-cycle mode (4MHz), or 2 for 8MHz sub-T-cycle precision.
/// NOTE: Currently using 8MHz mode (2) for SameBoy-accurate sub-T-cycle timing.
pub const PPU_CYCLES_PER_T_CYCLE: i32 = 2;

pub struct Clock {
    pub(crate) div: u16,
    pub(crate) tac: u8,
    pub(crate) tima: u8,
    pub(crate) tma: u8,
    /// T-cycles remaining until TIMA is reloaded from TMA.
    /// 0 means no reload is pending.
    pub(crate) tima_reload_pending: u8,
}

impl Default for Clock {
    fn default() -> Self {
        Self {
            div: 0,
            tac: 0,
            tima: 0,
            tma: 0,
            tima_reload_pending: 0,
        }
    }
}

impl Clock {
    pub fn tima(&self) -> u8 {
        if (1..=5).contains(&self.tima_reload_pending) {
            0
        } else {
            self.tima
        }
    }

    pub const fn tma(&self) -> u8 {
        self.tma
    }
}

impl<A: AudioCallback> Gb<A> {
    /// Advance all components by the given number of CPU T-cycles.
    /// This is the main timing entry point called by the CPU.
    #[inline]
    pub fn advance_dots(&mut self, cpu_t_cycles: i32) {
        self.run_timers(cpu_t_cycles);
        self.advance_dots_no_timers(cpu_t_cycles);
    }

    #[inline]
    pub fn flush_pending_dots(&mut self) {
        if self.pending_dots > 0 {
            let dots = self.pending_dots;
            self.pending_dots = 0;
            self.advance_dots(dots);
        }
    }

    #[inline]
    pub fn advance_dots_no_timers(&mut self, cpu_t_cycles: i32) {
        // DMA runs at T-cycle rate
        self.dma.advance_dots(cpu_t_cycles);

        let double_speed = self.key1.is_enabled();

        // Calculate real-time 4MHz dots elapsed.
        // In double speed, 1 CPU T-cycle = 0.5 real dots.
        let real_dots = if double_speed {
            cpu_t_cycles / 2
        } else {
            cpu_t_cycles
        };

        // PPU runs at 8MHz (2× real-time dots)
        let ppu_cycles = real_dots * PPU_CYCLES_PER_T_CYCLE;

        let dma_active = self.dma.is_active();
        let dma_src = self.dma.current_src();
        let dma_dst = self.dma.current_dst();
        let hdma_active = self.hdma.is_active();

        self.ppu.run(
            ppu_cycles,
            &mut self.ints,
            self.cgb_mode,
            double_speed,
            dma_active,
            dma_src,
            dma_dst,
            hdma_active,
        );
        self.run_dma();

        // APU runs at 4MHz real-time rate
        self.apu.run(real_dots);
        self.cart.run_rtc(real_dots);

        self.dots_ran += real_dots;

        #[expect(clippy::cast_sign_loss)]
        {
            self.total_dots += real_dots as u64;
        }
    }

    fn inc_tima(&mut self) {
        self.clock.tima = self.clock.tima.wrapping_add(1);

        if self.clock.tima == 0 {
            // TIMA overflow: reload will happen after 5 T-cycles
            self.clock.tima_reload_pending = 5;
        }
    }

    #[must_use]
    const fn is_tac_enabled(&self) -> bool {
        self.clock.tac & 4 != 0
    }

    #[must_use]
    #[inline]
    pub const fn read_div(&self) -> u8 {
        ((self.clock.div >> 8) & 0xFF) as u8
    }

    #[must_use]
    #[inline]
    pub const fn read_tac(&self) -> u8 {
        0xF8 | self.clock.tac
    }

    #[inline]
    pub fn run_timers(&mut self, cpu_t_cycles: i32) {
        for _ in 0..cpu_t_cycles {
            if self.clock.tima_reload_pending > 0 {
                if self.clock.tima_reload_pending <= 5 {
                    self.clock.tima_reload_pending -= 1;
                    if self.clock.tima_reload_pending == 0 {
                        self.clock.tima = self.clock.tma;
                        self.ints.request_timer();
                        // State 6-9: Already reloaded, TIMA writes ignored for 4 T-cycles (1 M-cycle)
                        self.clock.tima_reload_pending = 6;
                    }
                } else {
                    // In "Reloaded" state (6, 7, 8, 9).
                    self.clock.tima_reload_pending += 1;
                    if self.clock.tima_reload_pending > 9 {
                        self.clock.tima_reload_pending = 0;
                    }
                }
            }

            self.set_system_clk(self.clock.div.wrapping_add(1));
        }
    }

    #[inline]
    pub fn write_div(&mut self) {
        self.set_system_clk(0);
    }

    #[must_use]
    const fn sys_clk_tac_mux(tac: u8) -> u16 {
        match tac & 3 {
            0 => 1 << 9,
            1 => 1 << 3,
            2 => 1 << 5,
            _ => 1 << 7,
        }
    }

    #[inline]
    pub fn write_tac(&mut self, val: u8) {
        // Timer glitch: the AND gate output falls when (old_enable AND old_div_bit) was 1
        // and (new_enable AND new_div_bit) is 0, causing a spurious TIMA increment.
        if (self.clock.tac & 4) != 0 {
            let old_bit = Self::sys_clk_tac_mux(self.clock.tac);
            if (self.clock.div & old_bit) != 0 {
                if (val & 4) == 0 || (self.clock.div & Self::sys_clk_tac_mux(val)) == 0 {
                    self.inc_tima();
                }
            }
        }

        self.clock.tac = val;
    }

    #[inline]
    pub fn write_tima(&mut self, val: u8) {
        // Writing to TIMA during the "Reloaded" state (1 M-cycle after reload) is ignored.
        if self.clock.tima_reload_pending >= 6 {
            return;
        }
        // Writing to TIMA during the reloading window cancels the reload.
        self.clock.tima = val;
        self.clock.tima_reload_pending = 0;
    }

    #[inline]
    pub fn write_tma(&mut self, val: u8) {
        self.clock.tma = val;
        // If TMA is written during the reload window or the reloaded cycle,
        // the new value is used for TIMA.
        if self.clock.tima_reload_pending != 0 {
            self.clock.tima = val;
        }
    }

    // only modify div inside this function
    fn set_system_clk(&mut self, val: u16) {
        let triggers = self.clock.div & !val;
        let apu_bit = if self.key1.is_enabled() {
            0x2000
        } else {
            0x1000
        };

        // increase TIMA on falling edge of TAC mux
        if self.is_tac_enabled() && (triggers & Self::sys_clk_tac_mux(self.clock.tac) != 0) {
            self.inc_tima();
        }

        // advance serial master clock
        if triggers & u16::from(self.serial.div_mask()) != 0 {
            self.serial.run_master(&mut self.ints);
        }

        // advance APU on falling edge of APU_DIV bit
        if triggers & apu_bit != 0 {
            self.apu.step_div_apu();
        }

        self.clock.div = val;
    }

    #[inline]
    pub fn write_div_reg(&mut self) {
        self.write_div();
    }
}
