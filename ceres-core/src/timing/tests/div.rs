use super::*;

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
fn repro_tima_tc00_start_1_cgb() {
    // gambatte tc00_start_1 expects F0 on CGB (no TIMA increment).
    // This test writes TAC=0x04 and reads TIMA after a short delay.
    // With CGB DIV phase 0xABCC, bit 9 hasn't fallen yet at the read point.
    let mut gb = crate::GbBuilder::new(44100, crate::test_util::DummyAudio)
        .with_model(crate::Model::CgbE)
        .build();
    gb.skip_bootrom();
    // ROM: 14 NOPs + clear IF/IE + TIMA=0xF0, TMA=0xF0 + 4 NOPs + TAC=0x04 + JP + read
    gb.advance_dots(14 * 4); // 14 NOPs from PC=0x100
    gb.write_mem(0xFF0F, 0);
    gb.write_mem(0xFFFF, 0);
    gb.write_mem(0xFF06, 0xF0);
    gb.write_mem(0xFF05, 0xF0);
    gb.advance_dots(4 * 4); // 4 NOPs
    gb.write_mem(0xFF07, 0x04); // TAC = enabled, 1024-dot period
    // JP (16T) + NOP (4T) + read preparation
    gb.advance_dots(5 * 4 + 3 * 4); // JP + NOP + approximate read timing
    let tima = gb.read_mem(0xFF05);
    assert_eq!(
        tima, 0xF0,
        "TIMA should not have incremented yet (tc00_start_1)"
    );
}
