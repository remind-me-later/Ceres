use crate::{AudioCallback, Gb, apu::ApuCtx, ppu::Mode};
use core::{cmp::Ordering, time::Duration};

/// T-cycles per frame (4MHz rate).
pub const DOTS_PER_FRAME: i32 = 70224;
/// T-cycles per second (4MHz).
pub const DOTS_PER_SEC: i32 = 1 << 22;
pub const FRAME_DURATION: Duration = Duration::new(0, 16_742_706); // DOTS_PER_FRAME / DOTS_PER_SEC

pub struct Clock {
    pub div: u16,
    pub tac: u8,
    pub tima: u8,
    pub tma: u8,
    /// T-cycles remaining until TIMA is reloaded from TMA.
    /// `0` means no reload is pending; `1..=4` are the reads-0 window;
    /// `5..=8` are the writes-ignore window.
    pub tima_reload_pending: u8,
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
    pub tima_irq_countdown: u8,
    pub div_cycles: i32,
    pub div_state: u8,
    pub tima_reload_state: u8,
    pub stopped: bool,
    /// A DIV write is in progress (the APU's sweep glitches depend on it).
    pub during_div_write: bool,
}

impl Default for Clock {
    fn default() -> Self {
        Self {
            div: 8,
            tac: 0,
            tima: 0,
            tma: 0,
            tima_reload_pending: 0,
            tima_irq_countdown: 0,
            div_cycles: 0,
            div_state: 0,
            tima_reload_state: 0,
            stopped: false,
            during_div_write: false,
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
    #[inline]
    pub fn advance_dots(&mut self, cpu_t_cycles: i32) {
        if cpu_t_cycles <= 0 {
            return;
        }
        self.advance_cycles(cpu_t_cycles);
    }

    /// Port of SameBoy's `GB_advance_cycles`: the speed switch phases are
    /// handled first, then the timers, and the rest of the machine only
    /// runs once the post-switch freeze is over.
    fn advance_cycles(&mut self, mut cycles: i32) {
        if self.speed_switch.countdown != 0 {
            let countdown = self.speed_switch.countdown;
            match countdown.cmp(&cycles) {
                Ordering::Equal => {
                    self.key1.toggle_double_speed();
                    self.speed_switch.countdown = 0;
                }
                Ordering::Greater => self.speed_switch.countdown -= cycles,
                Ordering::Less => {
                    cycles -= countdown;
                    self.speed_switch.countdown = 0;
                    self.advance_cycles(countdown);
                    self.key1.toggle_double_speed();
                }
            }
        }

        self.apu.reset_pcm_mask();

        // The OAM DMA is clocked by the CPU clock, whatever the speed.
        self.dma.add_cycles(cycles);

        // Cycle-accurate timer advancement (per T-cycle, for accurate TIMA
        // reload timing).
        self.run_timers(cycles);

        if self.speed_switch.halt_countdown != 0 {
            self.speed_switch.halt_countdown -= cycles;
            if self.speed_switch.halt_countdown <= 0 {
                self.speed_switch.halt_countdown = 0;
                self.speed_switch.unhalt = true;
                // The halt ends right here for the DMAs: they run in this very
                // step (the CPU itself notices on its next one).
                let hblank = matches!(self.ppu.mode(), Mode::HBlank);
                self.hdma.set_cpu_halted(false, hblank);
                self.ppu.set_cpu_idle(self.clock.stopped);
            }
        }

        if self.speed_switch.freeze != 0 {
            let freeze = self.speed_switch.freeze;
            if freeze >= cycles {
                self.speed_switch.freeze -= cycles;
                return;
            }
            cycles -= freeze;
            self.speed_switch.freeze = 0;
        }

        // Advance the PPU (see `Ppu::tick_t_cycle`).
        let double_speed = self.key1.is_enabled();
        self.ppu
            .set_dma_lookahead(self.dma_dest_after(cycles), cycles);
        // A frame lasts the same time whatever the CPU speed.
        self.dots_ran += cycles * if double_speed { 1 } else { 2 };
        for _ in 0..cycles {
            self.ppu
                .tick_t_cycle(&mut self.ints, self.cgb_mode, double_speed);
        }

        if self.ppu.take_hblank_hdma_edge() {
            self.hdma.hblank_edge(self.clock.stopped);
        }

        self.run_dma();

        // The clock runs in 8 MHz units, whatever the CPU speed.
        self.cart
            .run_rtc(cycles.cast_unsigned() * if double_speed { 1 } else { 2 });
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
            self.clock.tima_irq_countdown = if matches!(self.cgb_mode, crate::CgbMode::Cgb) {
                4
            } else {
                3
            };
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

    /// What the APU needs to know about the machine.
    pub(crate) const fn apu_ctx(&self) -> ApuCtx {
        ApuCtx {
            address_bus: self.address_bus,
            div_counter: self.clock.div,
            double_speed: self.key1.is_enabled(),
            during_div_write: self.clock.during_div_write,
            pc: self.cpu.pc(),
            stopped: self.clock.stopped,
        }
    }

    /// Runs the APU for the 2 MHz ticks of one DIV step (one M-cycle).
    fn run_apu_step(&mut self) {
        let ctx = self.apu_ctx();
        self.apu.tick(&ctx, if ctx.double_speed { 1 } else { 2 });
    }

    #[inline]
    pub fn run_timers(&mut self, cpu_t_cycles: i32) {
        // The timers (and DIV) are frozen in STOP mode (the CGB's APU is not).
        if self.clock.stopped {
            if self.is_cgb() {
                self.run_apu_step();
            }
            return;
        }
        for _ in 0..cpu_t_cycles {
            // The CPU-side DIV-reset signal being held delays the timers
            // by a few T-cycles (set when entering STOP).
            if self.clock.div_cycles < 0 {
                self.clock.div_cycles += 1;
                continue;
            }
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

            let div = self.clock.div.wrapping_add(1);
            if div.trailing_zeros() >= 2 {
                if self.apu.pending_envelope_tick() {
                    let ctx = self.apu_ctx();
                    self.apu.delayed_envelope_tick(&ctx);
                }
                self.set_system_clk(div);
                self.run_apu_step();
            } else {
                self.set_system_clk(div);
            }
        }
    }

    /// A speed switch makes the timer see the STOP's DIV reset four cycles
    /// early when it is clocked at 16 cycles or slower: the reset counts as
    /// a falling edge of the tapped bit a bit sooner.
    pub(crate) fn tima_speed_change_catch_up(&mut self) {
        if self.key1.is_requested() && self.is_tac_enabled() && self.clock.tac & 3 != 0 {
            let mux = Self::sys_clk_tac_mux(self.clock.tac);
            let div = self.clock.div;
            if div & mux == 0 && div.wrapping_add(4) & mux != 0 {
                self.inc_tima();
            }
        }
    }

    #[inline]
    pub fn write_div(&mut self) {
        // Writing DIV resets the system clock and the APU's internal phase
        // counter (gambatte's `sound_unit::reset_cycle_counter` on div write).
        // Without this, the APU length counter / serial transfer can step
        // immediately after a DIV write, which breaks gambatte's
        // serial/sound testsuite.
        self.clock.during_div_write = true;
        self.set_system_clk(0);
        self.clock.during_div_write = false;
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
            if (self.clock.div & old_bit) != 0
                && ((val & 4) == 0 || (self.clock.div & Self::sys_clk_tac_mux(val)) == 0)
            {
                self.inc_tima();
            }
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
        if triggers & self.serial.div_mask() != 0 {
            self.serial.master_edge(&mut self.ints);
        }

        // The APU's frame sequencer follows the falling edge of an APU_DIV
        // bit; a rising edge arms the envelopes.
        if triggers & apu_bit != 0 {
            let ctx = self.apu_ctx();
            self.apu.div_event(&ctx);
        } else if !self.clock.div & val & apu_bit != 0 {
            self.apu.div_secondary_event();
        } else {
            // No edge of the APU bit.
        }

        self.clock.div = val;
    }

    #[inline]
    pub fn write_div_reg(&mut self) {
        self.write_div();
    }
}
