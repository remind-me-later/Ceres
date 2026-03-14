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
    pub(crate) tima_state: TIMAState,
    pub(crate) tma: u8,
    /// Accumulator for dots to handle SameBoy-accurate DIV timing.
    /// Initialized to 1 to match SameBoy's 3-cycle initial sleep (4 - 3 = 1).
    pub(crate) div_acc: i32,
}

impl Default for Clock {
    fn default() -> Self {
        Self {
            div: 1, // Match SameBoy's 3-cycle delay (4 - 3 = 1)
            tac: 0,
            tima: 0,
            tima_state: TIMAState::Running,
            tma: 0,
            div_acc: 1, // Match SameBoy's 3-cycle delay
        }
    }
}

impl Clock {
    pub fn tima(&self) -> u8 {
        match self.tima_state {
            TIMAState::Reloading => 0,
            _ => self.tima,
        }
    }

    pub const fn tma(&self) -> u8 {
        self.tma
    }
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TIMAState {
    Reloading,
    Reloaded,
    #[default]
    Running,
}

impl<A: AudioCallback> Gb<A> {
    /// Advance all components by the given number of T-cycles (4MHz).
    /// This is the main timing entry point called by the CPU.
    #[inline]
    pub fn advance_dots(&mut self, t_cycles: i32) {
        // Timers run at T-cycle rate (4MHz), affected by speed boost
        self.run_timers(t_cycles);
        self.advance_dots_no_timers(t_cycles);
    }

    #[inline]
    pub fn advance_dots_no_timers(&mut self, t_cycles: i32) {
        // DMA is affected by speed boost, runs at T-cycle rate
        self.dma.advance_dots(t_cycles);

        let double_speed = self.key1.is_enabled();

        // PPU runs at 8MHz (2× T-cycles) for sub-T-cycle precision.
        // In double speed mode, the CPU runs at 8MHz but PPU stays at 4MHz,
        // so we don't double the cycles.
        // SameBoy: timing.c line 481-483
        //   if (unlikely(!gb->cgb_double_speed)) {
        //       cycles <<= 1;
        //   }
        let ppu_cycles = if double_speed {
            t_cycles // Double speed: PPU sees T-cycles as-is
        } else {
            t_cycles * PPU_CYCLES_PER_T_CYCLE // Normal speed: double for 8MHz
        };

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

        // APU runs at T-cycle rate, not affected by speed boost for timing
        // but the actual T-cycle count changes in double speed
        let apu_cycles = if double_speed { t_cycles / 2 } else { t_cycles };
        self.apu.run(apu_cycles);
        self.cart.run_rtc(apu_cycles);

        self.dots_ran += apu_cycles;

        #[expect(clippy::cast_sign_loss)]
        {
            self.total_dots += apu_cycles as u64;
        }
    }

    fn advance_tima_state(&mut self) {
        match self.clock.tima_state {
            TIMAState::Reloading => {
                self.ints.request_timer();
                self.clock.tima_state = TIMAState::Reloaded;
            }
            TIMAState::Reloaded => {
                self.clock.tima_state = TIMAState::Running;
            }
            TIMAState::Running => (),
        }
    }

    const fn inc_tima(&mut self) {
        self.clock.tima = self.clock.tima.wrapping_add(1);

        if self.clock.tima == 0 {
            self.clock.tima = self.clock.tma;
            self.clock.tima_state = TIMAState::Reloading;
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
    pub fn run_timers(&mut self, dots: i32) {
        for _ in 0..dots {
            self.clock.div_acc += 1;
            if self.clock.div_acc == 4 {
                self.clock.div_acc = 0;
                self.advance_tima_state();
            }

            self.set_system_clk(self.clock.div.wrapping_add(1));
        }
    }

    #[inline]
    pub fn write_div(&mut self) {
        self.set_system_clk(0);
        self.clock.div_acc = 1; // Reset to match SameBoy's 3-cycle delay
        self.clock.div = 1;
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
        //
        // This is based on the expected results of the mooneye rapid_toggle test and
        // SameBoy's GB_emulate_timer_glitch implementation. Both DMG and CGB behave the
        // same for this case (the Pan Docs note that disabling the timer glitch does not
        // happen on CGB refers to a different scenario not tested by rapid_toggle).
        //
        // References:
        //   - Pan Docs "Timer Obscure Behaviour"
        //   - SameBoy Core/timing.c: GB_emulate_timer_glitch
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
    pub const fn write_tima(&mut self, val: u8) {
        match self.clock.tima_state {
            TIMAState::Reloaded => (),
            TIMAState::Reloading => {
                self.clock.tima = val;
                self.clock.tima_state = TIMAState::Running;
            }
            TIMAState::Running => self.clock.tima = val,
        }
    }

    #[inline]
    pub const fn write_tma(&mut self, val: u8) {
        self.clock.tma = val;
        match self.clock.tima_state {
            TIMAState::Reloading | TIMAState::Reloaded => self.clock.tima = val,
            TIMAState::Running => (),
        }
    }

    // only modify div inside this function
    // TODO: this could be optimized
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
