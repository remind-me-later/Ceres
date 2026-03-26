use crate::test_util::setup_gb;
use crate::{GbBuilder, Model};

#[cfg(test)]
fn setup_cgb() -> crate::Gb<crate::test_util::DummyAudio> {
    GbBuilder::new(44100, crate::test_util::DummyAudio)
        .with_model(Model::CgbE)
        .build()
}

#[cfg(test)]
fn advance_to_ly(gb: &mut crate::Gb<crate::test_util::DummyAudio>, ly: u8) {
    while gb.read_mem(0xFF44) != ly {
        gb.advance_dots(4);
    }
}

#[cfg(test)]
fn advance_to_mode(gb: &mut crate::Gb<crate::test_util::DummyAudio>, mode: u8) {
    while gb.read_mem(0xFF41) & 3 != mode {
        gb.advance_dots(4);
    }
}

#[test]
fn test_div_write_glitch_tc01() {
    let mut gb = setup_gb();
    gb.write_mem(0xFF04, 0);
    gb.write_mem(0xFF06, 0x00);
    gb.write_mem(0xFF05, 0x00);
    gb.write_mem(0xFF07, 0x05); // increments every 16 dots (bit 3)

    // internal div = 0. bit 3 is 0.
    // Advance internal div to 8. bit 3 becomes 1.
    gb.advance_dots(8);
    assert_eq!(gb.read_mem(0xFF05), 0x00);

    // Reset div to 0. bit 3 goes 1 -> 0 (falling edge).
    // Should trigger TIMA increment.
    gb.write_mem(0xFF04, 0);
    assert_eq!(gb.read_mem(0xFF05), 0x01);
}

#[test]
fn test_div_write_glitch_tima_increment() {
    let mut gb = setup_gb();
    gb.write_mem(0xFF04, 0);
    gb.write_mem(0xFF06, 0x00);
    gb.write_mem(0xFF05, 0xFF);
    gb.write_mem(0xFF07, 0x05); // increments every 16 dots

    // bit 3 is 0.
    gb.advance_dots(8);
    // bit 3 is 1.
    assert_eq!(gb.read_mem(0xFF05), 0xFF);

    // Reset div. falling edge. TIMA overflows.
    gb.write_mem(0xFF04, 0);
    // Reload happens after 4 dots (in Ceres).
    assert_eq!(gb.read_mem(0xFF05), 0x00, "Should be 0 during reload");
    gb.advance_dots(4);
    assert_eq!(gb.read_mem(0xFF05), 0x00, "Should be TMA (0) after reload");
}

#[test]
fn test_div_reset_3_cycle_delay() {
    let mut gb = setup_gb();
    // In Ceres, write_div calls set_system_clk(0) immediately.
    gb.write_mem(0xFF04, 0);
    assert_eq!(gb.read_div(), 0);
}

#[test]
fn test_div_increment_phase() {
    let mut gb = setup_gb();
    gb.write_mem(0xFF04, 0);
    // DIV increments every 256 dots.
    gb.advance_dots(255);
    assert_eq!(gb.read_div(), 0);
    gb.advance_dots(1);
    assert_eq!(gb.read_div(), 1);
}

#[test]
fn test_late_tc01_4() {
    let mut gb = setup_gb();
    gb.write_mem(0xFF04, 0);
    gb.write_mem(0xFF06, 0x00);
    gb.write_mem(0xFF05, 0xFF);
    gb.write_mem(0xFF07, 0x04); // increments every 1024 dots

    // bit 9 falls at 1024 dots.
    gb.advance_dots(1024);
    // TIMA becomes 0 during reload window.
    assert_eq!(gb.read_mem(0xFF05), 0x00);

    // During the 4-dot reload window, writing to TIMA should cancel the reload.
    // We are currently at T=1024. Reload window is [1024, 1028).
    gb.write_mem(0xFF05, 0x42);
    assert_eq!(gb.read_mem(0xFF05), 0x42);

    // Reload should be cancelled.
    gb.advance_dots(4);
    assert_eq!(gb.read_mem(0xFF05), 0x42);
}

#[test]
fn test_late_tc01_5_write() {
    let mut gb = setup_gb();
    gb.write_mem(0xFF04, 0);
    gb.write_mem(0xFF06, 0xFE);
    gb.write_mem(0xFF05, 0xFF);
    gb.write_mem(0xFF07, 0x05); // increments every 16 dots

    // Advance to exactly when overflow happens
    gb.advance_dots(16);

    // Advance 3 dots into the 4-dot reload window. [16, 20)
    gb.advance_dots(3); // T=19
    gb.write_mem(0xFF05, 0x00);

    // Advance 1 dot to finish the original reload window.
    gb.advance_dots(1); // T=20

    // The write at T=19 should have cancelled the reload and set TIMA to 0.
    assert_eq!(gb.read_mem(0xFF05), 0x00);
}

#[test]
fn test_late_tc01_6_write() {
    let mut gb = setup_gb();
    gb.write_mem(0xFF04, 0);
    gb.write_mem(0xFF06, 0xFE);
    gb.write_mem(0xFF05, 0xFF);
    gb.write_mem(0xFF07, 0x05);

    gb.advance_dots(16);
    // Reloading state [16, 20).
    // Reloaded state [20, 24).
    // Write at end of Reloading (T=19) should WORK.
    gb.advance_dots(3);
    gb.write_mem(0xFF05, 0x42);
    assert_eq!(gb.read_mem(0xFF05), 0x42);
}

#[test]
fn test_start_3_timing() {
    let mut gb = setup_gb();
    gb.write_mem(0xFF04, 0);
    gb.write_mem(0xFF06, 0x00);
    gb.write_mem(0xFF05, 0xF0);

    gb.write_mem(0xFF07, 0x04); // 4096Hz
    gb.advance_dots(1023);
    assert_eq!(gb.read_mem(0xFF05), 0xF0);
    gb.advance_dots(1);
    assert_eq!(gb.read_mem(0xFF05), 0xF1);
}

#[test]
fn test_start_3_timing_with_read_cpu() {
    let mut gb = setup_gb();
    gb.write_cpu(0xFF04, 0); // Sets DIV to 0, then advances 4 dots. total_dots = 4. DIV = 4.
    gb.write_cpu(0xFF06, 0x00); // 8
    gb.write_cpu(0xFF05, 0xF0); // 12
    gb.write_cpu(0xFF07, 0x04); // 16. Enabled.

    // Increment happens at DIV 1024.
    // DIV starts at 0 at Dot 8 (during write_cpu to 0xFF04).
    // Increment happens at Dot 8 + 1024 = 1032.

    // Advance to Dot 1028.
    gb.advance_dots(1028 - 16);
    assert_eq!(gb.total_dots(), 1028);

    // read_cpu at 1028 will read, then advance to 1032.
    // At 1028, DIV is 1020.
    let val = gb.read_cpu(0xFF05);
    // FIXME: This currently returns 0xF1 (241) instead of 0xF0 (240).
    // The integration test gambatte_tima_tc00_start_3_dmg08_outF0 also fails with this error.
    assert_eq!(val, 0xF0);
    assert_eq!(gb.total_dots(), 1032);

    // Next read_cpu at 1032 will read, then advance to 1036.
    // At 1032, DIV is 1024. TIMA has incremented!
    let val = gb.read_cpu(0xFF05);
    assert_eq!(val, 0xF1);
}

#[test]
fn test_irq_ds_1() {
    let mut gb = setup_cgb();
    gb.write_mem(0xFF04, 0);
    gb.write_mem(0xFF06, 0x00);
    gb.write_mem(0xFF05, 0xFF);
    gb.write_mem(0xFF07, 0x04);

    gb.advance_dots(1024);
    assert_eq!(gb.ints.read_if() & 0x04, 0, "IRQ should fire after reload");
    gb.advance_dots(4);
    assert_eq!(gb.ints.read_if() & 0x04, 0x04);
}

#[test]
fn test_irq_ds_timing_boundary() {
    let mut gb = setup_cgb();
    gb.write_mem(0xFF04, 0);
    gb.write_mem(0xFF06, 0x00);
    gb.write_mem(0xFF05, 0xFF);
    gb.write_mem(0xFF07, 0x04);

    gb.advance_dots(1024); // T=1024: Reloading starts
    assert_eq!(gb.ints.read_if() & 0x04, 0);
    gb.advance_dots(3); // T=1027: Still Reloading
    assert_eq!(gb.ints.read_if() & 0x04, 0);
    gb.advance_dots(1); // T=1028: Transition to Reloaded, IRQ should fire
    assert_eq!(gb.ints.read_if() & 0x04, 0x04);
}

#[test]
fn test_repro_speedchange_double_to_normal_dots() {
    let mut gb = setup_gb();
    gb.change_model_and_soft_reset(Model::CgbE);

    // 1. Enter double speed
    let addr = 0xC000;
    gb.set_cpu_pc(addr);
    gb.write_mem(addr, 0x10); // STOP
    gb.write_mem(addr + 1, 0x00);
    gb.write_mem(0xFF4D, 0x01);
    gb.run_cpu();
    assert!(gb.key1.is_enabled());

    // 2. Request speed change (Double -> Normal)
    gb.set_cpu_pc(addr);
    gb.write_mem(0xFF4D, 0x01);

    let start_dots = gb.total_dots();
    gb.run_cpu();
    let end_dots = gb.total_dots();

    let elapsed = end_dots - start_dots;
    // STOP takes 32768 M-cycles.
    // In double speed, 1 M-cycle = 2 dots.
    // FETCH (2 cycles) = 4 dots.
    // STOP (1 cycle) = 2 dots.
    // DELAY (32768 cycles) = 65536 dots.
    // Total: 4 + 2 + 65536 = 65542 dots.
    assert_eq!(elapsed, 65542);
}

#[test]
fn test_repro_late_tc00_5_write() {
    let mut gb = setup_gb();
    gb.write_mem(0xFF04, 0);
    gb.write_mem(0xFF06, 0xFE);
    gb.write_mem(0xFF05, 0xFF);
    gb.write_mem(0xFF07, 0x04);

    gb.advance_dots(1024);
    gb.advance_dots(3);
    gb.write_mem(0xFF05, 0x00);
    gb.advance_dots(1);

    assert_eq!(gb.read_mem(0xFF05), 0x00);
}

#[test]
fn test_repro_late_tc00_4_write() {
    let mut gb = setup_gb();
    gb.write_mem(0xFF04, 0);
    gb.write_mem(0xFF06, 0x00);
    gb.write_mem(0xFF05, 0xFF);
    gb.write_mem(0xFF07, 0x04);

    gb.advance_dots(1024);
    gb.advance_dots(2);
    gb.write_mem(0xFF05, 0xFF);
    gb.advance_dots(2);

    assert_eq!(gb.read_mem(0xFF05), 0xFF);
}

#[test]
fn test_repro_late_tc00_6_write() {
    let mut gb = setup_gb();
    gb.write_mem(0xFF04, 0);
    gb.write_mem(0xFF06, 0xFE);
    gb.write_mem(0xFF05, 0xFF);
    gb.write_mem(0xFF07, 0x04);

    gb.advance_dots(1024);
    gb.advance_dots(4);
    gb.advance_dots(1);
    gb.write_mem(0xFF05, 0xFF);
    gb.advance_dots(3);

    assert_eq!(gb.read_mem(0xFF05), 0xFE);
}

#[test]
fn test_repro_late_tc00_8_write() {
    let mut gb = setup_gb();
    gb.write_mem(0xFF04, 0);
    gb.write_mem(0xFF06, 0x00);
    gb.write_mem(0xFF05, 0xFF);
    gb.write_mem(0xFF07, 0x04);

    gb.advance_dots(1024);
    gb.advance_dots(8);
    gb.write_mem(0xFF05, 0xFF);

    assert_eq!(gb.read_mem(0xFF05), 0xFF);
}

#[test]
fn test_timer_startup_exhaustive_dmg() {
    let mut gb = setup_gb();
    gb.write_mem(0xFF04, 0);
    gb.write_mem(0xFF05, 0xF0);
    gb.write_mem(0xFF06, 0x00);
    gb.write_mem(0xFF07, 0x04);

    for i in 0..2000 {
        let tima = gb.read_mem(0xFF05);
        if i < 1024 {
            assert_eq!(tima, 0xF0, "T={}", i);
        } else if i < 2048 {
            assert_eq!(tima, 0xF1, "T={}", i);
        }
        gb.advance_dots(1);
    }
}

#[test]
fn test_timer_startup_exhaustive_cgb() {
    let mut gb = setup_cgb();
    gb.write_mem(0xFF04, 0);
    gb.write_mem(0xFF05, 0xF0);
    gb.write_mem(0xFF06, 0x00);
    gb.write_mem(0xFF07, 0x04);

    for i in 0..2000 {
        let tima = gb.read_mem(0xFF05);
        if i < 1024 {
            assert_eq!(tima, 0xF0, "T={}", i);
        } else if i < 2048 {
            assert_eq!(tima, 0xF1, "T={}", i);
        }
        gb.advance_dots(1);
    }
}

#[test]
fn test_repro_div_inc_dmg() {
    let mut gb = setup_gb();
    assert_eq!(gb.read_div(), 0);
    for i in 0..255 {
        gb.advance_dots(1);
        assert_eq!(gb.read_div(), 0, "T={}", i);
    }
    gb.advance_dots(1);
    assert_eq!(gb.read_div(), 1);
}

#[test]
fn test_repro_div_inc_cgb() {
    let mut gb = setup_cgb();
    assert_eq!(gb.read_div(), 0);
    for i in 0..255 {
        gb.advance_dots(1);
        assert_eq!(gb.read_div(), 0, "T={}", i);
    }
    gb.advance_dots(1);
    assert_eq!(gb.read_div(), 1);
}

#[test]
fn test_tima_increment_cpu_sync() {
    let mut gb = setup_gb();
    gb.write_mem(0xFF04, 0);
    gb.write_mem(0xFF05, 0);
    gb.write_mem(0xFF07, 0x05); // 16 dots

    gb.advance_dots(15);
    assert_eq!(gb.read_mem(0xFF05), 0);
    gb.advance_dots(1);
    assert_eq!(gb.read_mem(0xFF05), 1);
}

#[test]
fn test_tima_reload_delay() {
    let mut gb = setup_gb();
    gb.write_mem(0xFF04, 0);
    gb.write_mem(0xFF06, 0x42);
    gb.write_mem(0xFF05, 0xFF);
    gb.write_mem(0xFF07, 0x05);

    gb.advance_dots(16);
    assert_eq!(gb.read_mem(0xFF05), 0, "Should read 0 during reload window");
    gb.advance_dots(3);
    assert_eq!(gb.read_mem(0xFF05), 0);
    gb.advance_dots(1);
    assert_eq!(
        gb.read_mem(0xFF05),
        0x42,
        "Should read TMA after reload window"
    );
}

#[test]
fn test_tima_write_during_reloading_cancels_reload() {
    let mut gb = setup_gb();
    gb.write_mem(0xFF04, 0);
    gb.write_mem(0xFF06, 0x42);
    gb.write_mem(0xFF05, 0xFF);
    gb.write_mem(0xFF07, 0x05);

    gb.advance_dots(16);
    gb.write_mem(0xFF05, 0x12);
    assert_eq!(gb.read_mem(0xFF05), 0x12);
    gb.advance_dots(8);
    assert_eq!(gb.read_mem(0xFF05), 0x12);
}

#[test]
fn test_tima_write_during_reloaded_is_ignored() {
    let mut gb = setup_gb();
    gb.write_mem(0xFF04, 0);
    gb.write_mem(0xFF06, 0x42);
    gb.write_mem(0xFF05, 0xFF);
    gb.write_mem(0xFF07, 0x05);

    gb.advance_dots(20); // Just entered Reloaded
    gb.write_mem(0xFF05, 0x12);
    assert_eq!(gb.read_mem(0xFF05), 0x42);
    gb.advance_dots(4);
    assert_eq!(gb.read_mem(0xFF05), 0x42);
}

#[test]
fn test_tma_write_during_reloading_updates_tima() {
    let mut gb = setup_gb();
    gb.write_mem(0xFF04, 0);
    gb.write_mem(0xFF06, 0x00);
    gb.write_mem(0xFF05, 0xFF);
    gb.write_mem(0xFF07, 0x05);

    gb.advance_dots(16); // T=16: Reloading starts
    gb.write_mem(0xFF06, 0x42); // TMA = 0x42, should update internal TIMA but read as 0
    assert_eq!(gb.read_mem(0xFF05), 0);
    gb.advance_dots(4); // T=20: Reloaded starts
    assert_eq!(gb.read_mem(0xFF05), 0x42);
}

#[test]
fn test_tma_write_during_reloaded_updates_tima() {
    let mut gb = setup_gb();
    gb.write_mem(0xFF04, 0);
    gb.write_mem(0xFF06, 0x00);
    gb.write_mem(0xFF05, 0xFF);
    gb.write_mem(0xFF07, 0x05);

    gb.advance_dots(20);
    gb.write_mem(0xFF06, 0x42);
    assert_eq!(gb.read_mem(0xFF05), 0x42);
}

#[test]
fn test_tma_written_before_overflow_takes_effect_on_next_reload() {
    let mut gb = setup_gb();
    gb.write_mem(0xFF04, 0);
    gb.write_mem(0xFF06, 0x11);
    gb.write_mem(0xFF05, 0xFF);
    gb.write_mem(0xFF07, 0x05);

    gb.write_mem(0xFF06, 0x42);
    gb.advance_dots(16);
    assert_eq!(gb.read_mem(0xFF05), 0);
    gb.advance_dots(4);
    assert_eq!(gb.read_mem(0xFF05), 0x42);
}

#[test]
fn test_timer_glitch_tac_stop() {
    let mut gb = setup_gb();
    gb.write_mem(0xFF04, 0);
    gb.write_mem(0xFF05, 0);
    gb.write_mem(0xFF07, 0x05); // enabled, bit 3

    gb.advance_dots(8); // bit 3 is 1
    gb.write_mem(0xFF07, 0x01); // disabled
    assert_eq!(gb.read_mem(0xFF05), 1, "Glitch should increment TIMA");
}

#[test]
fn test_timer_rapid_toggle_cgb_disable_glitch_fires() {
    let mut gb = setup_cgb();
    gb.write_mem(0xFF04, 0);
    gb.write_mem(0xFF05, 0);
    gb.write_mem(0xFF07, 0x05);

    gb.advance_dots(8);
    gb.write_mem(0xFF07, 0x01);
    assert_eq!(gb.read_mem(0xFF05), 1);
}

#[test]
fn test_tac_readback_high_bits() {
    let mut gb = setup_gb();
    gb.write_mem(0xFF07, 0x00);
    assert_eq!(gb.read_mem(0xFF07), 0xF8);
    gb.write_mem(0xFF07, 0x07);
    assert_eq!(gb.read_mem(0xFF07), 0xFF);
}

#[test]
fn test_start_2_timing() {
    let mut gb = setup_cgb();
    gb.write_mem(0xFF04, 0);
    gb.write_mem(0xFF06, 0x00);
    gb.write_mem(0xFF05, 0xF0);
    gb.write_mem(0xFF07, 0x04);

    gb.advance_dots(1023);
    assert_eq!(gb.read_mem(0xFF05), 0xF0);
    gb.advance_dots(1);
    assert_eq!(gb.read_mem(0xFF05), 0xF1);
}

// -----------------------------------------------------------------------
// Repro tests for failing gambatte timer tests
//
// These tests document timer behaviors relevant to the 47 failing gambatte
// timer integration tests. The core issue is that Ceres doesn't properly
// complete the TIMA reload cycle in all cases.
// -----------------------------------------------------------------------

/// Helper: verify timer increments TIMA by 1 after exactly one period.
fn check_timer_period(tac: u8, period: i32, init_tima: u8) {
    let mut gb = setup_gb();
    gb.write_mem(0xFF04, 0);
    gb.write_mem(0xFF06, init_tima.wrapping_sub(1));
    gb.write_mem(0xFF05, init_tima);
    gb.write_mem(0xFF07, tac);
    gb.advance_dots(period - 1);
    assert_eq!(gb.read_mem(0xFF05), init_tima);
    gb.advance_dots(1);
    assert_eq!(gb.read_mem(0xFF05), init_tima.wrapping_add(1));
}

#[test]
fn repro_timer_period_tc00() {
    check_timer_period(0x04, 1024, 0x00);
}
#[test]
fn repro_timer_period_tc01() {
    check_timer_period(0x05, 16, 0x00);
}
#[test]
fn repro_timer_period_tc02() {
    check_timer_period(0x06, 64, 0x00);
}
#[test]
fn repro_timer_period_tc03() {
    check_timer_period(0x07, 256, 0x00);
}

#[test]
fn repro_timer_stop_prevents_increment() {
    let mut gb = setup_gb();
    gb.write_mem(0xFF04, 0);
    gb.write_mem(0xFF05, 0x00);
    gb.write_mem(0xFF07, 0x04); // TAC=0x04, 1024-dot period
    gb.advance_dots(511); // Just before first increment (bit 9 rises at 512)
    gb.write_mem(0xFF07, 0x00); // Disable timer
    gb.advance_dots(100);
    assert_eq!(
        gb.read_mem(0xFF05),
        0x00,
        "Timer stopped should not increment TIMA"
    );
}

// TODO: repro test for DIV not resetting on TAC write (affects 47 gambatte tests)
// The timer doesn't reset its internal DIV counter when TAC is written,
// which is the root cause of most stop/start test failures.

#[test]
fn repro_timer_div_write_glitch() {
    let mut gb = setup_gb();
    gb.write_mem(0xFF04, 0);
    gb.write_mem(0xFF05, 0x00);
    gb.write_mem(0xFF07, 0x05);
    gb.advance_dots(8);
    gb.write_mem(0xFF04, 0);
    assert_eq!(gb.read_mem(0xFF05), 0x01);
}

#[test]
fn repro_timer_tac_disable_glitch() {
    let mut gb = setup_gb();
    gb.write_mem(0xFF04, 0);
    gb.write_mem(0xFF05, 0x00);
    gb.write_mem(0xFF07, 0x05);
    gb.advance_dots(8);
    gb.write_mem(0xFF07, 0x01);
    assert_eq!(gb.read_mem(0xFF05), 0x01);
}

#[test]
fn repro_timer_reload_delay() {
    let mut gb = setup_gb();
    gb.write_mem(0xFF04, 0);
    gb.write_mem(0xFF06, 0x42);
    gb.write_mem(0xFF05, 0xFF);
    gb.write_mem(0xFF07, 0x05);
    gb.advance_dots(16);
    assert_eq!(gb.read_mem(0xFF05), 0x00);
    gb.advance_dots(3);
    assert_eq!(gb.read_mem(0xFF05), 0x00);
    gb.advance_dots(1);
    assert_eq!(gb.read_mem(0xFF05), 0x42);
}

#[test]
fn repro_timer_write_during_reload_cancels() {
    let mut gb = setup_gb();
    gb.write_mem(0xFF04, 0);
    gb.write_mem(0xFF06, 0x42);
    gb.write_mem(0xFF05, 0xFF);
    gb.write_mem(0xFF07, 0x05);
    gb.advance_dots(16);
    gb.write_mem(0xFF05, 0x12);
    assert_eq!(gb.read_mem(0xFF05), 0x12);
    gb.advance_dots(8);
    assert_eq!(gb.read_mem(0xFF05), 0x12);
}

#[test]
fn repro_timer_tma_write_during_reload() {
    let mut gb = setup_gb();
    gb.write_mem(0xFF04, 0);
    gb.write_mem(0xFF06, 0x00);
    gb.write_mem(0xFF05, 0xFF);
    gb.write_mem(0xFF07, 0x05);
    gb.advance_dots(16);
    gb.write_mem(0xFF06, 0x42);
    assert_eq!(gb.read_mem(0xFF05), 0x00);
    gb.advance_dots(4);
    assert_eq!(gb.read_mem(0xFF05), 0x42);
}

#[test]
fn repro_timer_tma_write_during_reloaded() {
    let mut gb = setup_gb();
    gb.write_mem(0xFF04, 0);
    gb.write_mem(0xFF06, 0x00);
    gb.write_mem(0xFF05, 0xFF);
    gb.write_mem(0xFF07, 0x05);
    gb.advance_dots(20);
    gb.write_mem(0xFF06, 0x42);
    assert_eq!(gb.read_mem(0xFF05), 0x42);
}

#[test]
fn repro_timer_irq_fires_on_reload() {
    let mut gb = setup_gb();
    gb.write_mem(0xFF04, 0);
    gb.write_mem(0xFF06, 0x00);
    gb.write_mem(0xFF05, 0xFF);
    gb.write_mem(0xFF07, 0x04);
    gb.advance_dots(1024);
    assert_eq!(gb.ints.read_if() & 0x04, 0);
    gb.advance_dots(4);
    assert_eq!(gb.ints.read_if() & 0x04, 0x04);
}

#[test]
fn repro_timer_multiple_periods() {
    let mut gb = setup_gb();
    gb.write_mem(0xFF04, 0);
    gb.write_mem(0xFF05, 0x00);
    gb.write_mem(0xFF07, 0x05);
    for expected in 1..=10u8 {
        gb.advance_dots(16);
        assert_eq!(
            gb.read_mem(0xFF05),
            expected,
            "TIMA should be {expected} after {expected} periods"
        );
    }
}

// TODO: repro test for DIV not resetting on TAC write (affects 47 gambatte tests)
// The timer doesn't reset its internal DIV counter when TAC is written,
// which is the root cause of most stop/start test failures.
