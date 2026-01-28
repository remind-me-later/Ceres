use crate::{AudioCallback, Gb};
use core::time::Duration;

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
        // Timer glitch: if timer is enabled and the muxed bit was 1,
        // and now it's disabled or the new muxed bit is 0, TIMA increments.
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

#[cfg(test)]
mod tests {
    use crate::test_util::setup_gb;

    #[test]
    fn test_tima_reload_delay() {
        let mut gb = setup_gb();
        gb.write_mem(0xFF04, 0); // Reset DIV to synchronize phase
        gb.write_mem(0xFF40, 0); // LCD off
        gb.write_mem(0xFF06, 0x42); // TMA = 0x42
        gb.write_mem(0xFF05, 0xFE); // TIMA = 0xFE
        gb.write_mem(0xFF07, 0x05); // TAC = 5 (Enabled, 262144 Hz -> every 16 dots)

        // Wait for increment to 0xFF
        for _ in 0..4 {
            gb.advance_dots(4);
        }
        assert_eq!(gb.read_mem(0xFF05), 0xFF);

        // Wait for overflow
        for _ in 0..4 {
            gb.advance_dots(4);
        }
        // Now TIMA should have overflowed.
        // Pan Docs: During the M-cycle after TIMA overflows, TIMA remains 00 (not TMA).
        assert_eq!(gb.read_mem(0xFF05), 0x00);

        // Next M-cycle it should be reloaded to TMA
        gb.advance_dots(4);
        assert_eq!(gb.read_mem(0xFF05), 0x42);
    }

    #[test]
    fn test_div_increment_phase() {
        let mut gb = setup_gb();
        gb.write_mem(0xFF04, 0); // Reset DIV

        // Ceres Assumption: div_acc = 0 after reset.
        // T=0: div_acc=0
        // T=1: div_acc=1
        // T=2: div_acc=2
        // T=3: div_acc=3
        // T=4: div_acc=4 -> INCREMENT!

        // Initial state after write_div:
        assert_eq!(gb.read_div(), 0);

        gb.advance_dots(3);
        assert_eq!(gb.read_div(), 0, "DIV incremented too early (at T=3)");

        gb.advance_dots(1);
        // After 4 dots total, internal counter should be 4.
        // DIV register reads internal counter >> 8.
        // To see an increment in the upper byte, we need 256 dots.

        // Let's test the internal counter if we can, or just loop.
        for _ in 0..(256 - 4) / 4 {
            gb.advance_dots(4);
        }
        // At T=256, internal DIV should be 256, so DIV register should be 1.
        assert_eq!(gb.read_div(), 1, "DIV should be 1 after 256 dots");
    }

    #[test]
    fn test_tima_increment_cpu_sync() {
        let mut gb = setup_gb();
        // Setup: TIMA increments every 64 dots (TAC = 4, 4096Hz)
        // Mux bit for TAC=4 is bit 9 of DIV.
        // Bit 9 falls every 1024 dots.
        gb.write_mem(0xFF04, 0); // Reset DIV
        gb.write_mem(0xFF06, 0); // TMA = 0
        gb.write_mem(0xFF05, 0); // TIMA = 0
        gb.write_mem(0xFF07, 0x04); // TAC = 4 (Enabled, 4096 Hz -> every 1024 dots)

        // DIV increments every 4 dots.
        // DIV reaches 512 (bit 9 becomes 1) at 512 * 4 = 2048 dots.
        // DIV reaches 1024 (bit 9 becomes 0) at 1024 * 4 = 4096 dots.
        // TIMA increments at T=4096.

        // Advance to T=4092
        for _ in 0..4092 / 4 {
            gb.advance_dots(4);
        }

        // Reset TIMA to 0 after setup to simplify testing the next increment
        gb.write_mem(0xFF05, 0);

        // Now we are at T=4092. TIMA should be 0.
        assert_eq!(gb.read_mem(0xFF05), 0);

        // Next M-cycle is T=4092 to T=4096.
        // In a real CPU read (2+2 timing):
        // 1. advance_dots(2) -> T=4094. TIMA still 0.
        // 2. read_mem() -> should see 0.
        // 3. advance_dots(2) -> T=4096. TIMA becomes 1.
        gb.advance_dots(2);
        assert_eq!(gb.read_mem(0xFF05), 0, "TIMA incremented too early");
        gb.advance_dots(2);

        // Next M-cycle is T=4096 to T=4100.
        // 1. advance_dots(2) -> T=4098. TIMA is already 1.
        // 2. read_mem() -> should see 1.
        gb.advance_dots(2);
        assert_eq!(gb.read_mem(0xFF05), 1, "TIMA should have incremented");
    }

    #[test]
    fn test_timer_glitch_tac_stop() {
        let mut gb = setup_gb();
        gb.write_mem(0xFF04, 0); // Reset DIV
        // Advance DIV to a point where bit 9 is 1 (T=512)
        for _ in 0..512 / 4 {
            gb.advance_dots(4);
        }

        gb.write_mem(0xFF06, 0x00); // TMA = 0
        gb.write_mem(0xFF05, 0x42); // TIMA = 0x42
        gb.write_mem(0xFF07, 0x04); // TAC = 4 (Enabled, 4096Hz -> bit 9)

        assert_eq!(gb.read_mem(0xFF05), 0x42);

        // Falling edge glitch: Disable timer while muxed bit is 1
        gb.write_mem(0xFF07, 0x00); // TAC = 0 (Disabled)

        assert_eq!(
            gb.read_mem(0xFF05),
            0x43,
            "Timer glitch did not trigger TIMA increment"
        );
    }
}
