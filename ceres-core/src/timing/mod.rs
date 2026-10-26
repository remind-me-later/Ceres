#[cfg(kani)]
pub(crate) mod netlist_proofs;
#[cfg(kani)]
mod proofs;
mod timer;

pub(crate) use timer::Clock;

use crate::{
    AudioCallback, CgbMode, Gb, Model,
    apu::{Apu, ApuCtx},
    interrupts::Interrupts,
    ppu::Mode,
    serial::Serial,
};
use core::{cmp::Ordering, time::Duration};
use timer::TimerHost;

// Time is counted in three units:
// - a dot is a 4 MHz cycle, the PPU's pixel clock;
// - a T-cycle is a CPU cycle: a dot in single speed, half a dot in double
//   speed;
// - an 8 MHz unit is half a dot, SameBoy's tick, which the frame and the
//   RTC count in.

/// Dots per frame.
pub(crate) const DOTS_PER_FRAME: i32 = 70224;
/// Dots per second.
pub(crate) const DOTS_PER_SEC: i32 = 1 << 22;
/// 8 MHz units per frame.
pub(crate) const UNITS_PER_FRAME: i32 = 2 * DOTS_PER_FRAME;

/// The bit of the internal counter (DIV is its high byte) whose falling edge
/// clocks the APU's frame sequencer: DIV bit 4, bit 5 in double speed.
#[must_use]
pub(crate) const fn apu_div_bit(double_speed: bool) -> u16 {
    if double_speed { 0x2000 } else { 0x1000 }
}
pub const FRAME_DURATION: Duration = Duration::new(0, 16_742_706); // DOTS_PER_FRAME / DOTS_PER_SEC

/// What the APU needs to know about the machine, with the timer as `clock`.
const fn apu_ctx(clock: &Clock, double_speed: bool, pc: u16, address_bus: u16) -> ApuCtx {
    ApuCtx {
        address_bus,
        div_counter: clock.div,
        double_speed,
        during_div_write: clock.during_div_write,
        pc,
        stopped: clock.stopped,
    }
}

/// The rest of the machine as the timer reaches it: `Gb` borrowed apart
/// from its `Clock`, so the timer can run while the host is in use.
struct TimerPorts<'a, A: AudioCallback> {
    apu: &'a mut Apu<A>,
    serial: &'a mut Serial,
    ints: &'a mut Interrupts,
    model: Model,
    cgb_mode: CgbMode,
    double_speed: bool,
    pc: u16,
    address_bus: u16,
}

impl<A: AudioCallback> TimerPorts<'_, A> {
    const fn apu_ctx(&self, clock: &Clock) -> ApuCtx {
        apu_ctx(clock, self.double_speed, self.pc, self.address_bus)
    }
}

impl<A: AudioCallback> TimerHost for TimerPorts<'_, A> {
    #[inline]
    fn is_cgb_hardware(&self) -> bool {
        self.model.is_cgb_hardware()
    }

    #[inline]
    fn is_cgb_mode(&self) -> bool {
        matches!(self.cgb_mode, CgbMode::Cgb)
    }

    #[inline]
    fn double_speed(&self) -> bool {
        self.double_speed
    }

    #[inline]
    fn request_timer_interrupt(&mut self) {
        self.ints.request_timer();
    }

    #[inline]
    fn serial_div_mask(&self) -> u16 {
        self.serial.div_mask()
    }

    #[inline]
    fn serial_clock_edge(&mut self) {
        self.serial.master_edge(self.ints);
    }

    #[inline]
    fn apu_frame_sequencer_step(&mut self, clock: &Clock) {
        let ctx = self.apu_ctx(clock);
        self.apu.div_event(&ctx);
    }

    #[inline]
    fn apu_envelopes_armed(&mut self) {
        self.apu.div_secondary_event();
    }

    #[inline]
    fn apu_delayed_envelope_tick(&mut self, clock: &Clock) {
        if self.apu.pending_envelope_tick() {
            let ctx = self.apu_ctx(clock);
            self.apu.delayed_envelope_tick(&ctx);
        }
    }

    #[inline]
    fn apu_m_cycle(&mut self, clock: &Clock) {
        let ctx = self.apu_ctx(clock);
        self.apu.tick(&ctx, if ctx.double_speed { 1 } else { 2 });
    }
}

impl<A: AudioCallback> Gb<A> {
    /// Advances the machine by `t_cycles` CPU T-cycles (half a dot each in
    /// double speed). This is the main timing entry point.
    #[inline]
    pub fn advance_t_cycles(&mut self, t_cycles: i32) {
        if t_cycles <= 0 {
            return;
        }
        #[cfg(feature = "debug")]
        let since_units = self.clock_units;
        #[cfg(feature = "debug")]
        {
            let units_per_cycle = if self.key1.is_enabled() { 1 } else { 2 };
            self.clock_units += u64::from(t_cycles.cast_unsigned()) * units_per_cycle;
        }
        self.advance_cycles(t_cycles);
        #[cfg(feature = "debug")]
        self.record_cut(since_units);
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
                    self.entered_double_speed();
                    self.speed_switch.countdown = 0;
                }
                Ordering::Greater => self.speed_switch.countdown -= cycles,
                Ordering::Less => {
                    cycles -= countdown;
                    self.speed_switch.countdown = 0;
                    self.advance_cycles(countdown);
                    self.key1.toggle_double_speed();
                    self.entered_double_speed();
                }
            }
        }

        self.apu.reset_pcm_mask();

        // The OAM DMA is clocked by the CPU clock, whatever the speed.
        self.dma.set_cycles(cycles);

        // Cycle-accurate timer advancement (per T-cycle, for accurate TIMA
        // reload timing).
        self.run_timers(cycles);

        if self.speed_switch.halt_countdown != 0 {
            self.speed_switch.halt_countdown -= cycles;
            if self.speed_switch.halt_countdown <= 0 {
                self.speed_switch.halt_countdown = 0;
                self.speed_switch.unhalt = true;
                // The halt ends right here for the DMAs: they run in this very
                // step (the CPU itself notices on its next one). On the CGB-C
                // it lasts until the CPU resumes (gambatte's unhalt).
                if !self.ppu.cgb_c_timing() {
                    let hblank = matches!(self.ppu.mode(), Mode::HBlank);
                    self.hdma.set_cpu_halted(false, hblank);
                    self.ppu.set_cpu_idle(self.clock.stopped);
                }
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
            .set_dma_lookahead(self.dma_phase_after(cycles), cycles);
        // A frame lasts the same time whatever the CPU speed: count 8 MHz units.
        self.units_ran += cycles * if double_speed { 1 } else { 2 };
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

    #[must_use]
    #[inline]
    pub const fn read_div(&self) -> u8 {
        self.clock.read_div()
    }

    #[must_use]
    #[inline]
    pub const fn read_tac(&self) -> u8 {
        self.clock.read_tac()
    }

    /// What the APU needs to know about the machine.
    pub(crate) const fn apu_ctx(&self) -> ApuCtx {
        apu_ctx(
            &self.clock,
            self.key1.is_enabled(),
            self.cpu.pc(),
            self.address_bus,
        )
    }

    /// The timer, and the rest of the machine as it reaches it.
    const fn timer(&mut self) -> (&mut Clock, TimerPorts<'_, A>) {
        let ports = TimerPorts {
            apu: &mut self.apu,
            serial: &mut self.serial,
            ints: &mut self.ints,
            model: self.model,
            cgb_mode: self.cgb_mode,
            double_speed: self.key1.is_enabled(),
            pc: self.cpu.pc(),
            address_bus: self.address_bus,
        };
        (&mut self.clock, ports)
    }

    #[inline]
    pub fn run_timers(&mut self, cpu_t_cycles: i32) {
        let (clock, mut ports) = self.timer();
        clock.run(&mut ports, cpu_t_cycles);
    }

    /// A speed switch makes the timer see the STOP's DIV reset a little
    /// early (see `Clock::speed_switch_catch_up`).
    pub(crate) fn tima_speed_change_catch_up(&mut self) {
        if self.key1.is_requested() {
            let late_slow_timers = self.model == Model::CgbE;
            let (clock, ports) = self.timer();
            clock.speed_switch_catch_up(&ports, late_slow_timers);
        }
    }

    #[inline]
    pub fn write_div(&mut self) {
        let (clock, mut ports) = self.timer();
        clock.write_div(&mut ports);
    }

    /// The DIV reset that STOP does when the CPU switches speed.
    pub(crate) fn reset_div_for_speed_switch(&mut self) {
        let (clock, mut ports) = self.timer();
        clock.reset_div_for_speed_switch(&mut ports);
    }

    #[inline]
    pub fn write_tac(&mut self, val: u8) {
        let (clock, ports) = self.timer();
        clock.write_tac(&ports, val);
    }

    #[inline]
    pub const fn write_tima(&mut self, val: u8) {
        self.clock.write_tima(val);
    }

    #[inline]
    pub const fn write_tma(&mut self, val: u8) {
        self.clock.write_tma(val);
    }

    /// The CPU just switched to double speed.
    fn entered_double_speed(&mut self) {
        self.clock.apu_delay = u16::from(self.apu.is_enabled() && self.clock.apu_odd);
    }

    /// The APU is powered on: a sequencer started now has no phase to
    /// correct, and its parity follows the current speed.
    pub(crate) const fn apu_powered_on(&mut self) {
        self.clock.apu_delay = 0;
        self.clock.apu_odd = !self.key1.is_enabled();
    }

    /// The CPU just switched back to single speed.
    pub(crate) const fn left_double_speed(&mut self) {
        self.clock.apu_delay = 0;
        self.clock.apu_odd = !self.clock.apu_odd;
    }
}
