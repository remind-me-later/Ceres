//! The timer (DIV, TIMA, TMA and TAC) and the DIV bits that clock the APU's
//! frame sequencer and the serial port.

use super::apu_div_bit;

// TAC bits
pub(super) const TAC_ENABLE_B: u8 = 0x04;
/// The input clock select.
pub(super) const TAC_CLOCK: u8 = 0x03;

/// What the timer reaches of the rest of the machine.
///
/// Most peripherals take what they need from the machine as arguments. A
/// trait like this one, or the CPU's `Bus`, pays for itself only where a
/// second implementation does: here the proofs' `AnyTimerHost`
/// (`proofs.rs`), which checks the TIMA rules from any timer state. Larger
/// peripherals, whose interplay with the rest depends on the order things
/// run in, are better served by plain arguments.
pub(crate) trait TimerHost {
    /// The machine is a CGB.
    fn is_cgb_hardware(&self) -> bool;

    /// The game runs in CGB mode: the timer interrupt comes a T-cycle later.
    fn is_cgb_mode(&self) -> bool;

    /// The CPU runs at double speed.
    fn double_speed(&self) -> bool;

    fn request_timer_interrupt(&mut self);

    /// The bits of the internal counter whose falling edge clocks the
    /// serial port.
    fn serial_div_mask(&self) -> u16;

    /// A falling edge of the serial port's clock.
    fn serial_clock_edge(&mut self);

    /// A falling edge of the APU's DIV bit: the frame sequencer steps.
    /// `clock` is the timer before the edge.
    fn apu_frame_sequencer_step(&mut self, clock: &Clock);

    /// A rising edge of the APU's DIV bit, which arms the envelopes.
    fn apu_envelopes_armed(&mut self);

    /// An M-cycle of the internal counter is about to pass: an envelope
    /// tick the APU delayed lands now, if there is one.
    fn apu_delayed_envelope_tick(&mut self, clock: &Clock);

    /// The APU runs for an M-cycle of the internal counter.
    fn apu_m_cycle(&mut self, clock: &Clock);
}

pub(crate) struct Clock {
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
    pub stopped: bool,
    /// A DIV write is in progress (the APU's sweep glitches depend on it).
    pub during_div_write: bool,
    /// Cycles the frame sequencer of an APU that was already running lags
    /// the DIV counter by: a speed switch to double speed leaves it a cycle
    /// behind when `apu_odd` is set.
    pub apu_delay: u16,
    /// The level of the delayed APU bit right after a DIV write in double
    /// speed, until the next tick: the reset reaches the lagging sequencer a
    /// cycle late, so for that cycle it still sees the bit as it was.
    pub apu_late_bit: Option<bool>,
    /// gambatte's `lastUpdate_ % 2`: odd for an APU powered on in single
    /// speed, even in double speed, flipped by each switch back to single
    /// speed.
    pub apu_odd: bool,
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
            stopped: false,
            during_div_write: false,
            apu_delay: 0,
            apu_late_bit: None,
            apu_odd: true,
        }
    }
}

impl Clock {
    pub(crate) fn tima(&self) -> u8 {
        if (1..=4).contains(&self.tima_reload_pending) {
            0
        } else {
            self.tima
        }
    }

    pub(crate) const fn tma(&self) -> u8 {
        self.tma
    }

    /// Advances the timers by `cpu_t_cycles` T-cycles.
    #[inline]
    pub(crate) fn run(&mut self, host: &mut impl TimerHost, cpu_t_cycles: i32) {
        // The timers (and DIV) are frozen in STOP mode (the CGB's APU is not).
        if self.stopped {
            if host.is_cgb_hardware() {
                host.apu_m_cycle(self);
            }
            return;
        }
        for _ in 0..cpu_t_cycles {
            // The CPU-side DIV-reset signal being held delays the timers
            // by a few T-cycles (set when entering STOP).
            if self.div_cycles < 0 {
                self.div_cycles += 1;
                continue;
            }
            if self.tima_reload_pending > 0 {
                if self.tima_reload_pending <= 4 {
                    self.tima_reload_pending -= 1;
                    if self.tima_reload_pending == 0 {
                        // Transition into the writes-ignore window. The
                        // IRQ is fired by `tima_irq_countdown` (separately
                        // below), not here, so the IRQ fire time is no
                        // longer tied to the reads-0 / writes-ignore
                        // windows.
                        self.tima_reload_pending = 5;
                    }
                } else {
                    // In "Reloaded" state (5, 6, 7, 8).
                    self.tima_reload_pending += 1;
                    if self.tima_reload_pending > 8 {
                        self.tima_reload_pending = 0;
                    }
                }
            }

            // Independent IRQ countdown — fires 3 T-cycles after overflow
            // on DMG and 4 on CGB (see `tima_irq_countdown` field docs and
            // `inc_tima` for the per-model initial value).
            if self.tima_irq_countdown > 0 {
                self.tima_irq_countdown -= 1;
                if self.tima_irq_countdown == 0 {
                    host.request_timer_interrupt();
                }
            }

            let div = self.div.wrapping_add(1);
            if div.trailing_zeros() >= 2 {
                host.apu_delayed_envelope_tick(self);
                self.set_div(host, div);
                host.apu_m_cycle(self);
            } else {
                self.set_div(host, div);
            }
        }
    }

    #[inline]
    fn inc_tima(&mut self, host: &impl TimerHost) {
        self.tima = self.tima.wrapping_add(1);

        if self.tima == 0 {
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
            self.tima = self.tma;
            self.tima_reload_pending = 4;
            self.tima_irq_countdown = if host.is_cgb_mode() { 4 } else { 3 };
        }
    }

    #[must_use]
    const fn is_tac_enabled(&self) -> bool {
        self.tac & TAC_ENABLE_B != 0
    }

    #[must_use]
    pub(crate) const fn read_div(&self) -> u8 {
        self.div.to_le_bytes()[1]
    }

    #[must_use]
    pub(crate) const fn read_tac(&self) -> u8 {
        !(TAC_ENABLE_B | TAC_CLOCK) | self.tac
    }

    /// A speed switch makes the timer see the STOP's DIV reset four cycles
    /// early when it is clocked at 16 cycles or slower: the reset counts as
    /// a falling edge of the tapped bit a bit sooner.
    ///
    /// On a CGB-E (`late_slow_timers`) that holds for the 262144 Hz timer
    /// only; the 65536 and 16384 Hz ones see the reset on time (AGE
    /// `spsw-tima-cgbE`).
    pub(crate) fn speed_switch_catch_up(&mut self, host: &impl TimerHost, late_slow_timers: bool) {
        let clock = self.tac & TAC_CLOCK;
        if self.is_tac_enabled() && clock != 0 && !(late_slow_timers && clock != 1) {
            let mux = Self::sys_clk_tac_mux(self.tac);
            if self.div & mux == 0 && self.div.wrapping_add(4) & mux != 0 {
                self.inc_tima(host);
            }
        }
    }

    /// A write to DIV by the CPU.
    ///
    /// It resets the system clock and the APU's internal phase counter
    /// (gambatte's `sound_unit::reset_cycle_counter` on div write). Without
    /// the reset, the APU length counter / serial transfer can step
    /// immediately after a DIV write, which breaks gambatte's serial/sound
    /// testsuite.
    ///
    /// A frame sequencer that lags DIV (`apu_delay`, in double speed) keeps
    /// its lag: the falling edge the reset causes reaches it a cycle late,
    /// like every other edge (AGE `spsw-ch2-lc-delay`, `TEST_DS_DIV_RESET`).
    pub(crate) fn write_div(&mut self, host: &mut impl TimerHost) {
        self.during_div_write = true;
        if self.apu_delay == 0 {
            self.set_div(host, 0);
        } else {
            let apu_bit = apu_div_bit(host.double_speed());
            let level = self.div.wrapping_sub(self.apu_delay) & apu_bit != 0;
            self.timer_edges(host, 0);
            self.div = 0;
            self.apu_late_bit = Some(level);
        }
        self.during_div_write = false;
    }

    /// The DIV reset of a speed switch (STOP): the sequencer's lag ends with
    /// the old speed.
    pub(crate) fn reset_div_for_speed_switch(&mut self, host: &mut impl TimerHost) {
        self.apu_delay = 0;
        self.apu_late_bit = None;
        self.write_div(host);
    }

    /// The bit of the internal counter whose falling edge clocks TIMA:
    /// 4096, 262144, 65536 or 16384 Hz.
    #[must_use]
    pub(super) const fn sys_clk_tac_mux(tac: u8) -> u16 {
        match tac & TAC_CLOCK {
            0 => 1 << 9,
            1 => 1 << 3,
            2 => 1 << 5,
            _ => 1 << 7,
        }
    }

    // @fact timer-enable-race: enabling the timer does not count.
    pub(crate) fn write_tac(&mut self, host: &impl TimerHost, val: u8) {
        // Timer glitch: the AND gate output falls when (old_enable AND old_div_bit) was 1
        // and (new_enable AND new_div_bit) is 0, causing a spurious TIMA increment.
        if (self.tac & TAC_ENABLE_B) != 0 {
            let old_bit = Self::sys_clk_tac_mux(self.tac);
            if (self.div & old_bit) != 0
                && ((val & TAC_ENABLE_B) == 0 || (self.div & Self::sys_clk_tac_mux(val)) == 0)
            {
                self.inc_tima(host);
            }
        }

        self.tac = val;
    }

    pub(crate) const fn write_tima(&mut self, val: u8) {
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
        if self.tima_reload_pending >= 5 {
            return;
        }
        self.tima_reload_pending = 0;
        self.tima_irq_countdown = 0;
        self.tima = val;
    }

    pub(crate) const fn write_tma(&mut self, val: u8) {
        self.tma = val;
        // If TMA is written during the reload window or the reloaded cycle,
        // the new value is used for TIMA.
        if self.tima_reload_pending != 0 {
            self.tima = val;
        }
    }

    /// The edges of DIV's bits that clock TIMA and the serial port when the
    /// counter goes to `val`.
    fn timer_edges(&mut self, host: &mut impl TimerHost, val: u16) {
        let triggers = self.div & !val;

        // increase TIMA on falling edge of TAC mux
        if self.is_tac_enabled() && (triggers & Self::sys_clk_tac_mux(self.tac) != 0) {
            self.inc_tima(host);
        }

        // advance serial master clock
        if triggers & host.serial_div_mask() != 0 {
            host.serial_clock_edge();
        }
    }

    // only modify div inside this function (and `write_div`)
    #[inline]
    fn set_div(&mut self, host: &mut impl TimerHost, val: u16) {
        let apu_bit = apu_div_bit(host.double_speed());
        self.timer_edges(host, val);

        // The APU's frame sequencer follows the falling edge of an APU_DIV
        // bit; a rising edge arms the envelopes.
        let old = self
            .apu_late_bit
            .take()
            .unwrap_or_else(|| self.div.wrapping_sub(self.apu_delay) & apu_bit != 0);
        let new = val.wrapping_sub(self.apu_delay) & apu_bit != 0;
        if old && !new {
            host.apu_frame_sequencer_step(self);
        } else if !old && new {
            host.apu_envelopes_armed();
        } else {
            // No edge of the APU bit.
        }

        self.div = val;
    }
}
