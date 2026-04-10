use super::*;

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

#[test]
fn test_repro_gbmicro_tima_phase_a() {
    // Tests timer phase exactly as `gbmicrotest/timer_tima_phase_a.s`
    let expected = [0xFE, 0xFF, 0xFF, 0xFF, 0xFF, 0x00, 0x80, 0x80, 0x80, 0x81];

    for (delay, &exp) in expected.iter().enumerate() {
        let mut gb = setup_gb();
        // Setup TMA and TAC
        gb.write_mem(0xFF06, 0x80);
        gb.write_mem(0xFF07, 0x05); // 262144 Hz (16 dots)

        // Set TIMA to 0xFD. As the timer runs during the following CPU instructions,
        // TIMA increments by 1 before DIV is finally reset to 0.
        // Thus, we simulate this initial 0xFE value.
        gb.write_mem(0xFF05, 0xFE);

        // Reset DIV to 0. This synchronizes the timer phase.
        gb.write_mem(0xFF04, 0);

        // Wait `delay` NOPs (4 dots each) + Read overhead (12 dots).
        // Total delay = delay * 4 + 12 dots.
        gb.advance_dots((delay as i32) * 4 + 12);

        assert_eq!(
            gb.read_mem(0xFF05),
            exp,
            "TIMA Phase mismatch at DELAY={}",
            delay
        );
    }
}

#[test]
fn test_repro_gbmicro_tima_inc_256k_a() {
    // Tests timer phase increments as exactly defined by `timer_tima_inc_256k_a.s`
    let expected = [1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4];

    for (delay, &exp) in expected.iter().enumerate() {
        let mut gb = setup_gb();

        gb.write_mem(0xFF04, 0); // DIV

        // In the assembly test, instructions take time between writes:
        // ld a, $00 (8 ticks) + ldh (TIMA), a (12 ticks)
        gb.advance_dots(20);
        gb.write_mem(0xFF05, 0); // TIMA

        // ld a, $34 (8 ticks) + ldh (TMA), a (12 ticks)
        gb.advance_dots(20);
        gb.write_mem(0xFF06, 0x34); // TMA

        // ld a, %00000101 (8 ticks) + ldh (TAC), a (12 ticks)
        gb.advance_dots(20);
        gb.write_mem(0xFF07, 0x05); // TAC: 262144 Hz (16 dots)

        // Wait `delay` NOPs (4 dots each) + Read overhead (12 dots)
        // Note: The read actually happens at the end of the ldh instruction,
        // so it observes the state after 12 dots.
        gb.advance_dots((delay as i32) * 4 + 12);

        assert_eq!(
            gb.read_mem(0xFF05),
            exp,
            "TIMA INC mismatch at DELAY={}",
            delay
        );
    }
}

#[test]
fn test_timer_glitch_tac_bit_flip() {
    let mut gb = setup_gb();

    // 1. Setup: Enable timer, 4096 Hz (bit 9 of DIV)
    // TIMA starts at 0.
    gb.write_mem(0xFF06, 0x00); // TMA
    gb.write_mem(0xFF07, 0x04); // Enable, 4096 Hz
    gb.write_mem(0xFF05, 0x00); // TIMA

    // 2. Advance DIV until bit 9 is 1.
    // DIV starts at 0. Bit 9 becomes 1 at 512 T-cycles.
    gb.advance_dots(512);
    assert!(gb.read_mem(0xFF05) == 0);

    // 3. Disable the timer (bit 2: 1 -> 0).
    // Because DIV bit 9 is high, this creates a falling edge at the AND gate.
    gb.write_mem(0xFF07, 0x00);

    // 4. TIMA should have incremented to 1 due to the glitch.
    assert_eq!(
        gb.read_mem(0xFF05),
        1,
        "TIMA should increment when timer is disabled while DIV bit is high"
    );
}

#[test]
fn test_diagnostic_tima_reload_window_probe() {
    let mut gb = setup_gb();
    gb.write_mem(0xFF04, 0); // DIV = 0
    gb.write_mem(0xFF06, 0x42); // TMA = 0x42
    gb.write_mem(0xFF05, 0xFE); // TIMA = 0xFE
    gb.write_mem(0xFF07, 0x05); // 262144 Hz (16 dots)

    // Advance to 16 dots: TIMA should be 0xFF
    gb.advance_dots(16);
    assert_eq!(gb.read_mem(0xFF05), 0xFF);

    // Advance to 32 dots: TIMA should be 0x00 (Reloading starts)
    gb.advance_dots(16);
    assert_eq!(
        gb.read_mem(0xFF05),
        0x00,
        "TIMA should read 0 during Reloading"
    );

    // Check next 4 dots (Reloading window)
    println!("--- Diagnostic: TIMA Reload Window Probe ---");
    for t in 0..8 {
        let tima = gb.read_mem(0xFF05);
        let reload_pending = gb.clock.tima_reload_pending;
        println!(
            "Tick +{}: TIMA=0x{:02X}, Pending={}",
            t, tima, reload_pending
        );
        gb.advance_dots(1);
    }
    println!("--------------------------------------------");
}
