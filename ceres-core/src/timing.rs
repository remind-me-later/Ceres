use crate::{AudioCallback, Gb};
use core::time::Duration;

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
            div: 8, // SameBoy initializes DIV to 8
            tac: 0,
            tima: 0,
            tima_state: TIMAState::Running,
            tma: 0,
            div_acc: 0,
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
