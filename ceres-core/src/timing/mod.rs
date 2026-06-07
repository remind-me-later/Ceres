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
    pub(crate) tma: u8,
    /// T-cycles remaining until TIMA is reloaded from TMA.
    /// `0` means no reload is pending; `1..=4` are the reads-0 window;
    /// `5..=8` are the writes-ignore window.
    pub(crate) tima_reload_pending: u8,
    /// Independent countdown for the timer IRQ fire time. `0` means
    /// no IRQ pending. On overflow we set this to `3` for DMG and `4`
    /// for CGB, then the IRQ fires when it reaches 0.
    ///
    /// DMG: matches gambatte's `Tima::updateTima` which sets
    /// `tmatime_ = lastUpdate_ + 3` (libgambatte/src/tima.cpp:99).
    ///
    /// CGB: matches gambatte's `Memory::ackIrq` which does
    /// `updateTimaIrq(cc + 2 + isCgb())` for the *next* IRQ event
    /// (libgambatte/src/memory.cpp:439), and SameBoy's per-M-cycle
    /// state machine which advances one M-cycle (= 4 T-cycles) per
    /// overflow. Both effectively fire the IRQ one T-cycle later on
    /// CGB than on DMG.
    ///
    /// Kept separate from `tima_reload_pending` so the reads-0 window
    /// (4 T-cycles, required by mooneye `tima_reload.s`) can coexist
    /// with the model-specific IRQ fire time.
    pub(crate) tima_irq_countdown: u8,
    pub(crate) div_cycles: i32,
    pub(crate) div_state: u8,
    pub(crate) tima_reload_state: u8,
    pub(crate) stopped: bool,
}

impl Default for Clock {
    fn default() -> Self {
        Self {
            div: 0,
            tac: 0,
            tima: 0,
            tma: 0,
            tima_reload_pending: 0,
            tima_irq_countdown: 0,
            div_cycles: 0,
            div_state: 0,
            tima_reload_state: 0,
            stopped: false,
        }
    }
}

impl Clock {
    pub fn tima(&self) -> u8 {
        if (1..=4).contains(&self.tima_reload_pending) {
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
    ///
    /// Uses the scanline PPU: timers advance per T-cycle (for the cycle-accurate
    /// TIMA reload state machine), but the PPU is fed the full dot budget at
    /// once and handles its own mode transitions internally.
    #[inline]
    pub fn advance_dots(&mut self, cpu_t_cycles: i32) {
        if cpu_t_cycles <= 0 {
            return;
        }

        // Cycle-accurate timer advancement (per T-cycle, for accurate TIMA
        // reload timing).
        self.run_timers(cpu_t_cycles);

        // DMA advances per dot.
        self.dma.advance_dots(cpu_t_cycles);

        // Convert CPU T-cycles to PPU dots. The scanline renderer's
        // Mode::dots() constants are in the same units as the dots we pass
        // here. In double-speed mode the CPU runs at 2× but the PPU dot
        // budget per real-time unit is fixed, so we halve the dot count
        // passed to the PPU (mirrors 854dbf9 behaviour).
        let double_speed = self.key1.is_enabled();
        let mut ppu_dots = cpu_t_cycles;
        if double_speed {
            ppu_dots >>= 1;
        }

        self.ppu.run(ppu_dots, &mut self.ints, self.cgb_mode);
        self.run_dma();

        self.apu.run(ppu_dots);
        self.cart.run_rtc(ppu_dots);

        self.dots_ran += ppu_dots;
    }

    fn inc_tima(&mut self) {
        self.clock.tima = self.clock.tima.wrapping_add(1);

        if self.clock.tima == 0 {
            // TIMA overflow.
            //
            // The reads-0 / writes-ignore state machine (`tima_reload_pending`)
            // holds for 4+4 T-cycles, as required by the mooneye
            // `tima_reload.s` test (it samples at 4-T-cycle granularity
            // and expects TIMA reads to be 0 for 4 cycles, then TMA).
            //
            // The IRQ fire time is decoupled via `tima_irq_countdown`,
            // which is set to 3 on DMG and 4 on CGB so it matches
            // gambatte's `Tima::updateTima` (DMG, libgambatte/src/tima.cpp:99)
            // and gambatte's `Memory::ackIrq` plus SameBoy's per-M-cycle
            // state machine (CGB, libgambatte/src/memory.cpp:439). See
            // the field docs on `tima_irq_countdown`.
            self.clock.tima = self.clock.tma;
            self.clock.tima_reload_pending = 4;
            self.clock.tima_irq_countdown =
                if matches!(self.cgb_mode, crate::CgbMode::Cgb) { 4 } else { 3 };
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
                if self.clock.tima_reload_pending <= 4 {
                    self.clock.tima_reload_pending -= 1;
                    if self.clock.tima_reload_pending == 0 {
                        // Transition into the writes-ignore window. The
                        // IRQ is fired by `tima_irq_countdown` (separately
                        // below), not here, so the IRQ fire time is no
                        // longer tied to the reads-0 / writes-ignore
                        // windows.
                        self.clock.tima_reload_pending = 5;
                    }
                } else {
                    // In "Reloaded" state (5, 6, 7, 8).
                    self.clock.tima_reload_pending += 1;
                    if self.clock.tima_reload_pending > 8 {
                        self.clock.tima_reload_pending = 0;
                    }
                }
            }

            // Independent IRQ countdown — fires 3 T-cycles after overflow
            // on DMG and 4 on CGB (see `tima_irq_countdown` field docs and
            // `inc_tima` for the per-model initial value).
            if self.clock.tima_irq_countdown > 0 {
                self.clock.tima_irq_countdown -= 1;
                if self.clock.tima_irq_countdown == 0 {
                    self.ints.request_timer();
                }
            }

            self.set_system_clk(self.clock.div.wrapping_add(1));
        }
    }

    #[inline]
    pub fn write_div(&mut self) {
        // Writing DIV resets the system clock and the APU's internal phase
        // counter (gambatte's `sound_unit::reset_cycle_counter` on div write).
        // Without this, the APU length counter / serial transfer can step
        // immediately after a DIV write, which breaks gambatte's
        // serial/sound testsuite.
        self.set_system_clk(0);
        self.apu.reset_div_phase();
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

        if (val & 4) == 0 {
            self.clock.tima_reload_pending = 0;
        }

        self.clock.tac = val;
    }

    #[inline]
    pub fn write_tima(&mut self, val: u8) {
        // Writing to TIMA during the "Reloaded" state (writes-ignore window,
        // `tima_reload_pending >= 5`) is dropped on the floor. Writing
        // during the "Reloading" window (the 4-T-cycle reads-0 window) or
        // outside the state machine cancels both the pending reload and the
        // pending IRQ — matching gambatte's `Tima::setTima`:
        //
        //   if (tmatime_ - cc < 4) tmatime_ = disabled_time;
        //
        // (libgambatte/src/tima.cpp:116-117). This is what the gambatte
        // testsuite's `tc01_late_tima_irq_1` and the mooneye
        // `timer_tima_write_reloading` test depend on.
        if self.clock.tima_reload_pending >= 5 {
            return;
        }
        self.clock.tima_reload_pending = 0;
        self.clock.tima_irq_countdown = 0;
        self.clock.tima = val;
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
