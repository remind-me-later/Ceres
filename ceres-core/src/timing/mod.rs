use crate::{AudioCallback, Gb};
use core::time::Duration;

#[cfg(test)]
mod tests;

/// T-cycles per frame (4MHz rate).
pub const DOTS_PER_FRAME: i32 = 70224;
/// T-cycles per second (4MHz).
pub const DOTS_PER_SEC: i32 = 1 << 22;
pub const FRAME_DURATION: Duration = Duration::new(0, 16_742_706); // DOTS_PER_FRAME / DOTS_PER_SEC

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
            div: 8, // SameBoy initializes DIV to 8
            tac: 0,
            tima: 0,
            tima_state: TIMAState::Running,
            tma: 0,
            div_acc: 1,
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
        let ticks = if self.key1.is_enabled() {
            t_cycles
        } else {
            t_cycles * 2
        };
        self.advance_ticks(ticks);
    }

    #[inline]
    pub fn advance_ticks(&mut self, ticks: i32) {
        for _ in 0..ticks {
            self.advance_tick();
        }
    }

    #[inline]
    fn advance_tick(&mut self) {
        let double_speed = self.key1.is_enabled();

        if double_speed {
            // CPU/Timer/DMA run at 8MHz
            self.run_timers(1);
            self.dma.advance_dots(1);
            self.run_dma();

            // APU/PPU/RTC run at 4MHz (dots)
            self.tick_acc ^= 1;
            if self.tick_acc == 0 {
                self.ppu.run(
                    1,
                    &mut self.ints,
                    self.cgb_mode,
                    double_speed,
                    self.dma.is_active(),
                    self.dma.current_src(),
                    self.dma.current_dst(),
                    self.hdma.is_active(),
                );
                self.apu.run(1);
                self.cart.run_rtc(1);
                self.dots_ran += 1;
                self.total_dots += 1;
            }
        } else {
            // CPU/Timer/DMA/APU/PPU/RTC all effectively run at 4MHz (dots)
            // But we tick PPU at 8MHz granularity for sub-dot accuracy.
            self.ppu.run(
                1,
                &mut self.ints,
                self.cgb_mode,
                double_speed,
                self.dma.is_active(),
                self.dma.current_src(),
                self.dma.current_dst(),
                self.hdma.is_active(),
            );

            self.tick_acc ^= 1;
            if self.tick_acc == 0 {
                self.run_timers(1);
                self.dma.advance_dots(1);
                self.run_dma();
                self.apu.run(1);
                self.cart.run_rtc(1);
                self.dots_ran += 1;
                self.total_dots += 1;
            }
        }
    }

    #[inline]
    pub fn advance_dots_no_timers(&mut self, t_cycles: i32) {
        let ticks = if self.key1.is_enabled() {
            t_cycles
        } else {
            t_cycles * 2
        };
        self.advance_ticks_no_timers(ticks);
    }

    #[inline]
    pub fn advance_ticks_no_timers(&mut self, ticks: i32) {
        for _ in 0..ticks {
            self.advance_tick_no_timers();
        }
    }

    #[inline]
    fn advance_tick_no_timers(&mut self) {
        let double_speed = self.key1.is_enabled();

        if double_speed {
            // CPU/DMA run at 8MHz
            self.dma.advance_dots(1);
            self.run_dma();

            // APU/PPU/RTC run at 4MHz
            self.tick_acc ^= 1;
            if self.tick_acc == 0 {
                self.ppu.run(
                    1,
                    &mut self.ints,
                    self.cgb_mode,
                    double_speed,
                    self.dma.is_active(),
                    self.dma.current_src(),
                    self.dma.current_dst(),
                    self.hdma.is_active(),
                );
                self.apu.run(1);
                self.cart.run_rtc(1);
                self.dots_ran += 1;
                self.total_dots += 1;
            }
        } else {
            // CPU/DMA/APU/PPU/RTC all effectively run at 4MHz
            self.ppu.run(
                1,
                &mut self.ints,
                self.cgb_mode,
                double_speed,
                self.dma.is_active(),
                self.dma.current_src(),
                self.dma.current_dst(),
                self.hdma.is_active(),
            );

            self.tick_acc ^= 1;
            if self.tick_acc == 0 {
                self.dma.advance_dots(1);
                self.run_dma();
                self.apu.run(1);
                self.cart.run_rtc(1);
                self.dots_ran += 1;
                self.total_dots += 1;
            }
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
        self.clock.div_acc += dots;
        while self.clock.div_acc >= 4 {
            self.clock.div_acc -= 4;
            self.advance_tima_state();
            self.set_system_clk(self.clock.div.wrapping_add(4));
        }
    }

    #[inline]
    pub fn write_div(&mut self) {
        self.set_system_clk(0);
        self.clock.div_acc = 0; // Reset phase
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
