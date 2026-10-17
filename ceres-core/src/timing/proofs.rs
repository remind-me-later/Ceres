//! Kani proofs of the timer's TIMA rules (`cargo kani -p ceres-core`).
//!
//! Each proof starts the timer from any state that matters to the rule, on a
//! host that counts the timer interrupts and ignores the APU and the serial
//! port, and checks the rule as the hardware tests describe it.

use super::timer::{Clock, TAC_CLOCK, TAC_ENABLE_B, TimerHost};

/// A machine of either kind, at either speed, that counts the timer
/// interrupts.
struct AnyTimerHost {
    cgb_mode: bool,
    double_speed: bool,
    serial_div_mask: u16,
    timer_interrupts: u32,
}

impl AnyTimerHost {
    fn new() -> Self {
        Self {
            cgb_mode: kani::any(),
            double_speed: kani::any(),
            serial_div_mask: kani::any(),
            timer_interrupts: 0,
        }
    }
}

impl TimerHost for AnyTimerHost {
    fn is_cgb_hardware(&self) -> bool {
        self.cgb_mode
    }

    fn is_cgb_mode(&self) -> bool {
        self.cgb_mode
    }

    fn double_speed(&self) -> bool {
        self.double_speed
    }

    fn request_timer_interrupt(&mut self) {
        self.timer_interrupts += 1;
    }

    fn serial_div_mask(&self) -> u16 {
        self.serial_div_mask
    }

    fn serial_clock_edge(&mut self) {}

    fn apu_frame_sequencer_step(&mut self, _: &Clock) {}

    fn apu_envelopes_armed(&mut self) {}

    fn apu_delayed_envelope_tick(&mut self, _: &Clock) {}

    fn apu_m_cycle(&mut self, _: &Clock) {}
}

/// A running timer with any DIV, TMA and clock select, and no reload or
/// interrupt on its way.
fn any_timer(tima: u8, enabled: bool) -> Clock {
    let tac: u8 = kani::any::<u8>() & TAC_CLOCK;
    Clock {
        div: kani::any(),
        tac: if enabled { tac | TAC_ENABLE_B } else { tac },
        tima,
        tma: kani::any(),
        ..Clock::default()
    }
}

/// A timer whose TIMA overflows on the next T-cycle: it is 0xFF and the
/// selected DIV bit falls.
fn about_to_overflow() -> Clock {
    let clock = any_timer(0xFF, true);
    let mux = Clock::sys_clk_tac_mux(clock.tac);
    kani::assume(clock.div & mux != 0 && clock.div.wrapping_add(1) & mux == 0);
    clock
}

fn run(clock: &mut Clock, host: &mut AnyTimerHost, t_cycles: u8) {
    for _ in 0..t_cycles {
        clock.run(host, 1);
    }
}

/// After an overflow TIMA reads 0 for 4 T-cycles, then TMA (mooneye
/// `tima_reload`).
#[kani::proof]
#[kani::unwind(6)]
fn tima_reads_zero_then_tma() {
    let mut host = AnyTimerHost::new();
    let mut clock = about_to_overflow();
    clock.run(&mut host, 1);
    for _ in 0..4 {
        assert!(clock.tima() == 0);
        clock.run(&mut host, 1);
    }
    assert!(clock.tima() == clock.tma());
}

/// The interrupt comes 3 T-cycles after an overflow, 4 in CGB mode.
#[kani::proof]
#[kani::unwind(6)]
fn timer_interrupt_after_3_or_4() {
    let mut host = AnyTimerHost::new();
    let mut clock = about_to_overflow();
    clock.run(&mut host, 1);
    let delay = if host.cgb_mode { 4 } else { 3 };
    for t in 1..=4 {
        clock.run(&mut host, 1);
        assert!(host.timer_interrupts == u32::from(t >= delay));
    }
}

/// A TIMA write in the 4 T-cycles after the reload is ignored; one after
/// them lands (mooneye `tima_write_reloading`).
#[kani::proof]
#[kani::unwind(10)]
fn tima_writes_ignored_after_reload() {
    let mut host = AnyTimerHost::new();
    let mut clock = about_to_overflow();
    clock.run(&mut host, 1);
    let late: u8 = kani::any();
    kani::assume(late <= 4);
    run(&mut clock, &mut host, 4 + late);
    let val = kani::any();
    clock.write_tima(val);
    let expected = if late < 4 { clock.tma() } else { val };
    assert!(clock.tima() == expected);
}

/// A TIMA write while it reads 0 cancels the reload and an interrupt that
/// has not come yet (gambatte `tc01_late_tima_irq`).
#[kani::proof]
#[kani::unwind(10)]
fn tima_write_cancels_reload() {
    let mut host = AnyTimerHost::new();
    let mut clock = about_to_overflow();
    clock.run(&mut host, 1);
    let early: u8 = kani::any();
    kani::assume(early < 4);
    run(&mut clock, &mut host, early);
    let interrupts = host.timer_interrupts;
    let val = kani::any();
    clock.write_tima(val);
    assert!(clock.tima() == val);
    // No DIV edge can reach TIMA again in the next 8 T-cycles.
    run(&mut clock, &mut host, 8);
    assert!(clock.tima() == val);
    assert!(host.timer_interrupts == interrupts);
}

/// Writing DIV resets the counter, so a selected bit that was set falls and
/// clocks TIMA.
#[kani::proof]
fn div_write_falling_edge_increments_tima() {
    let mut host = AnyTimerHost::new();
    let tima: u8 = kani::any();
    kani::assume(tima != 0xFF);
    let enabled = kani::any();
    let mut clock = any_timer(tima, enabled);
    let fell = enabled && clock.div & Clock::sys_clk_tac_mux(clock.tac) != 0;
    clock.write_div(&mut host);
    assert!(clock.div == 0);
    assert!(clock.tima() == tima + u8::from(fell));
}
