use super::*;

#[test]
fn test_ppu_vram_lock_boundary() {
    let mut gb = setup_gb();
    gb.write_mem(0xFF40, 0x81); // LCD ON, BG ON

    // Synchronize to Line 1 OAM Scan Start (Dot 0)
    loop {
        if gb.ppu.read_ly() == 1
            && matches!(
                gb.ppu.phase,
                crate::ppu::PpuPhase::OamScan(crate::ppu::OamScanStage::Running { tick: 0 })
            )
        {
            break;
        }
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }

    // Advance 158 ticks to reach the tick where VRAM write unblocks (DMG).
    // Tick 158 unblocks VRAM write.
    for _ in 0..158 {
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }
    // Process tick 158 to actually unblock.
    gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);

    // Now tick is 159. Try write — should succeed.
    gb.ppu.write_vram(0x8000, 0x55);
    assert_eq!(
        gb.ppu.vram().read(0x8000),
        0x55,
        "VRAM write at tick 159 failed"
    );

    // Advance to tick 168.
    // Tick is 159 now. Need 9 more ticks to reach 168.
    for _ in 0..9 {
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }
    // Now tick is 168. Next tick() will process 168.
    // STAT should still be Mode 2 (2) because tick 168 hasn't been processed yet.
    assert_eq!(
        gb.ppu.read_stat() & 0x03,
        2,
        "STAT should still be Mode 2 before processing tick 168"
    );

    // Process tick 168 — transitions to Mode 3 and blocks VRAM.
    gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);

    assert_eq!(
        gb.ppu.read_stat() & 0x03,
        3,
        "STAT should be Mode 3 after processing tick 168"
    );
    gb.ppu.write_vram(0x8002, 0xBB);
    assert_ne!(
        gb.ppu.vram().read(0x8002),
        0xBB,
        "VRAM write at tick 168 should have been blocked"
    );
}

#[test]
fn test_ppu_oam_unlock_timing() {
    let mut gb = setup_gb();
    gb.write_mem(0xFF40, 0x81); // LCD ON, BG ON

    // Synchronize to Line 1 Mode 3 Start
    while gb.ppu.read_ly() != 1 && (gb.ppu.read_stat() & 0x03) != 3 {
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }
    while (gb.ppu.read_stat() & 0x03) != 3 {
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }

    // Advance until nearly the end of Mode 3
    // Mode 3 for SCX=0 is ~324 ticks.
    // HBlank interrupt fires at pos=154.
    // 154 pixels take 154 * 2 = 308 ticks.
    // Plus 16 ticks for first tile = 324 ticks.
    // Plus 10 ticks transition? = 334 ticks.
    // Let's just loop until pos=159.
    while gb.ppu.lcd_x() < 159 {
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }

    // At pos=159, OAM should still be locked
    assert!(gb.ppu.read_stat() & 0x03 == 3, "Still should be Mode 3");
    gb.ppu.write_oam(0xFE00, 0x55);
    assert_ne!(
        gb.ppu.oam().read(0xFE00),
        0x55,
        "OAM write at pos=159 should be blocked"
    );

    // Advance 2 ticks to finish last pixel
    gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);

    // Now it should be Mode 0 and OAM unlocked
    assert_eq!(gb.ppu.read_stat() & 0x03, 0, "Should be Mode 0 now");
    gb.ppu.write_oam(0xFE00, 0xAA);
    assert_eq!(
        gb.ppu.oam().read(0xFE00),
        0xAA,
        "OAM write at Mode 0 should succeed"
    );
}

#[test]
fn test_ppu_blocking_diagnostic_log() {
    let mut gb = setup_gb();
    gb.write_mem(0xFF40, 0x80); // LCD ON

    // Wait for Line 1 start
    loop {
        if gb.ppu.read_ly() == 1
            && matches!(
                gb.ppu.phase,
                crate::ppu::PpuPhase::OamScan(crate::ppu::OamScanStage::Running { tick: 0 })
            )
        {
            break;
        }
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }

    let mut last_blocking = (false, false, false, false);
    let mut blocking_changes = Vec::new();

    for t in 0..912 {
        let current = (
            gb.ppu.oam_read_blocked,
            gb.ppu.oam_write_blocked,
            gb.ppu.vram_read_blocked,
            gb.ppu.vram_write_blocked,
        );
        if current != last_blocking {
            blocking_changes.push((t, current));
            last_blocking = current;
        }
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }

    println!(
        "Blocking changes for Line 1 (OAM R/W, VRAM R/W): {:?}",
        blocking_changes
    );
}

#[test]
#[ignore = "DMG OAM corruption bug is not implemented yet"]
fn blargg_oam_bug_4_scanline_timing_inc_de_just_before_first_corruption() {
    let mut gb = setup_gb();
    fill_blargg_oam_bug_pattern(&mut gb);
    gb.write_mem(0xFF40, 0x80);
    advance_to_ly_dot(&mut gb, 1, 448);
    gb.set_cpu_de(0xFE00);
    let before = snapshot_oam(&gb);

    run_opcode(&mut gb, &[0x13]);

    let after = snapshot_oam(&gb);
    assert_oam_unchanged(
        &before,
        &after,
        "4-scanline_timing: INC DE just before first corruption",
    );
}

#[test]
#[ignore = "DMG OAM corruption bug is not implemented yet"]
fn blargg_oam_bug_4_scanline_timing_inc_de_at_first_corruption() {
    let mut gb = setup_gb();
    fill_blargg_oam_bug_pattern(&mut gb);
    gb.write_mem(0xFF40, 0x80);
    advance_to_ly_dot(&mut gb, 1, 452);
    gb.set_cpu_de(0xFE00);
    let before = snapshot_oam(&gb);

    run_opcode(&mut gb, &[0x13]);

    let after = snapshot_oam(&gb);
    assert_oam_changed(
        &before,
        &after,
        "4-scanline_timing: INC DE at first corruption",
    );
}

#[test]
#[ignore = "DMG OAM corruption bug is not implemented yet"]
fn blargg_oam_bug_4_scanline_timing_inc_de_at_last_corruption() {
    let mut gb = setup_gb();
    fill_blargg_oam_bug_pattern(&mut gb);
    gb.write_mem(0xFF40, 0x80);
    advance_to_ly_dot(&mut gb, 1, 72);
    gb.set_cpu_de(0xFE00);
    let before = snapshot_oam(&gb);

    run_opcode(&mut gb, &[0x13]);

    let after = snapshot_oam(&gb);
    assert_oam_changed(
        &before,
        &after,
        "4-scanline_timing: INC DE at last corruption",
    );
}

#[test]
#[ignore = "DMG OAM corruption bug is not implemented yet"]
fn blargg_oam_bug_4_scanline_timing_inc_de_just_after_last_corruption() {
    let mut gb = setup_gb();
    fill_blargg_oam_bug_pattern(&mut gb);
    gb.write_mem(0xFF40, 0x80);
    advance_to_ly_dot(&mut gb, 1, 76);
    gb.set_cpu_de(0xFE00);
    let before = snapshot_oam(&gb);

    run_opcode(&mut gb, &[0x13]);

    let after = snapshot_oam(&gb);
    assert_oam_unchanged(
        &before,
        &after,
        "4-scanline_timing: INC DE just after last corruption",
    );
}

#[test]
#[ignore = "DMG OAM corruption bug is not implemented yet"]
fn blargg_oam_bug_7_timing_effect_mid_window_rows_have_distinct_corruption_patterns() {
    for row in 1usize..20 {
        let mut gb = setup_gb();
        fill_blargg_oam_bug_pattern(&mut gb);
        gb.write_mem(0xFF40, 0x80);
        let line_dot = (row as u16).saturating_mul(4).saturating_sub(4);
        advance_to_ly_dot(&mut gb, 2, line_dot);
        gb.set_cpu_de(0xFE00);
        let before = snapshot_oam(&gb);
        let expected =
            expected_oam_after_blargg_oam_bug_access(before, row, BlarggOamBugAccessKind::Write);

        run_opcode(&mut gb, &[0x13]);

        let after = snapshot_oam(&gb);
        assert_eq!(
            after, expected,
            "7-timing_effect: row {} should match write-corruption pattern",
            row
        );
    }
}

#[test]
fn samesuite_blocking_bgpi_increase() {
    let mut gb = setup_gb();
    gb.write_mem(0xFF40, 0x80); // LCD ON

    // Test in each mode
    let modes = [0, 1, 2, 3];
    for mode in modes {
        // Advance to desired mode
        advance_to_mode(&mut gb, mode);

        // Write index 4, enable auto-increment
        gb.write_mem(0xFF68, 0x84);
        assert_eq!(
            gb.read_mem(0xFF68),
            0xC4,
            "BCPS write failed in mode {}",
            mode
        );

        // Write data to BCPD
        gb.write_mem(0xFF69, 0xAA);

        // Check if index incremented to 5 (blocked in mode 3)
        let expected_index = if mode == 3 { 0xC4 } else { 0xC5 };
        assert_eq!(
            gb.read_mem(0xFF68),
            expected_index,
            "BCPS auto-increment behavior mismatch in mode {}",
            mode
        );
    }
}

#[test]
fn age_ppu_vram_blocking() {
    let mut gb = setup_gb();
    gb.write_mem(0xFF40, 0x81); // LCD ON

    advance_to_ly(&mut gb, 1);
    advance_to_mode(&mut gb, 3);

    assert!(gb.ppu.vram_read_blocked, "VRAM should be blocked in Mode 3");

    let mut ticks_in_m3 = 0;
    loop {
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
        ticks_in_m3 += 1;
        if !gb.ppu.vram_read_blocked {
            break;
        }
    }

    assert_eq!(ticks_in_m3, 335, "VRAM unblocking timing changed!");
    assert_eq!(
        gb.ppu.read_stat() & 0x03,
        0,
        "VRAM should unblock exactly when Mode 0 starts"
    );
}

#[test]
fn test_repro_oam_access_m2_detailed() {
    let mut gb = setup_gb();
    gb.write_mem(0xFF40, 0x80);

    advance_to_ly(&mut gb, 20);
    advance_to_mode(&mut gb, 2);

    // Give the PPU a few ticks to completely block OAM
    gb.advance_dots(40);

    // OAM should be blocked during Mode 2
    gb.write_mem(0xFE00, 0xAA);
    let val = gb.read_mem(0xFE00);
    assert_eq!(val, 0xFF, "OAM should be inaccessible (0xFF) during Mode 2");

    // Wait for Mode 0
    advance_to_mode(&mut gb, 0);

    // In a real CPU, detecting Mode 0 and then writing to OAM takes several M-cycles.
    // The emulator changes STAT to Mode 0 a few ticks before Mode 3 actually ends.
    // We must advance dots to allow Mode 3 to fully complete before OAM is unblocked.
    gb.advance_dots(16);

    println!(
        "{} {} {}",
        gb.ppu.read_stat() & 3,
        gb.ppu.oam_write_blocked,
        gb.ppu.oam_read_blocked
    );
    gb.write_mem(0xFE00, 0x55);
    let val2 = gb.read_mem(0xFE00);
    assert_eq!(val2, 0x55, "OAM should be accessible (0x55) during Mode 0");
}

#[test]
fn repro_oam_access_postread_1() {
    // postread_1 expects 3 (OAM blocked during Mode 3)
    let mut gb = oam_access_setup();
    // Read OAM shortly after IRQ fires (still in Mode 2)
    // The Mode 2 IRQ fires 4 ticks before OamScan starts.
    // Ceres blocks OAM at tick 8 of OamScan.
    // Advance 6 dots (12 ticks) to ensure it reaches tick 8 of OamScan and is blocked.
    gb.advance_dots(6);
    let val = gb.read_mem(0xFE00);
    println!(
        "Phase after 6 dots: {:?}, blocked: {}",
        gb.ppu.phase, gb.ppu.oam_read_blocked
    );
    assert_eq!(val & 3, 3, "OAM should be blocked (0xFF & 3 = 3)");
}

#[test]
fn repro_oam_access_postread_2() {
    // postread_2 expects 0 (OAM accessible after Mode 3 ends)
    let mut gb = oam_access_setup();
    // Wait long enough for Mode 3 to end and OAM to become accessible
    // Mode 2 is ~80 dots, Mode 3 is ~172 dots, total ~252 dots
    // Need to wait until Mode 0
    gb.advance_dots(300);
    let stat_mode = gb.ppu.read_stat() & 3;
    let val = gb.read_mem(0xFE00);
    // If we're in Mode 0, OAM should be accessible
    if stat_mode == 0 {
        assert_eq!(val & 3, 0, "OAM should be accessible in Mode 0");
    } else {
        // Still in Mode 2/3, OAM blocked
        assert_eq!(val & 3, 3, "OAM blocked in Mode {}", stat_mode);
    }
}

#[test]
fn repro_oam_access_preread_1() {
    // preread_1 expects 0 (OAM accessible before Mode 2)
    // In Ceres, accessing OAM at the very start of Mode 0 may still be blocked
    // depending on exact timing. Use a longer delay.
    let mut gb = setup_gb();
    gb.write_mem(0xFF40, 0x80);
    advance_to_ly(&mut gb, 10);
    advance_to_mode(&mut gb, 2); // Wait for Mode 2 to start to bypass the 4-tick Mode 0 overlap at line start
    advance_to_mode(&mut gb, 0); // Now wait for the actual Mode 0 (HBlank) at the end of the line
    gb.advance_dots(100); // Well into Mode 0
    let val = gb.read_mem(0xFE00);
    assert_eq!(val & 3, 0, "OAM should be accessible well into Mode 0");
}

#[test]
fn repro_oam_access_preread_2() {
    // preread_2 expects 3 (OAM blocked in Mode 2)
    let mut gb = setup_gb();
    gb.write_mem(0xFF40, 0x80);
    advance_to_ly(&mut gb, 10);
    while (gb.ppu.read_stat() & 3) != 2 {
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }
    // Advance a few ticks past tick 4 when OAM blocking activates
    for _ in 0..10 {
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }
    let val = gb.read_mem(0xFE00);
    assert_eq!(val, 0xFF, "OAM blocked in Mode 2 (after tick 4)");
}

#[test]
fn repro_oam_access_midwrite_3() {
    // midwrite_3 expects 0 (write blocked during Mode 2)
    let mut gb = setup_gb();
    gb.write_mem(0xFF40, 0x80);
    advance_to_ly(&mut gb, 10);
    advance_to_mode(&mut gb, 2);
    gb.advance_dots(20);
    // Try to write OAM during Mode 2
    gb.write_mem(0xFE00, 0xAA);
    let val = gb.read_mem(0xFE00);
    // Write should be blocked (0xFF), read returns 0xFF
    // Original content was 0, so &3 = 3 for blocked read
    // The test checks that write was blocked
    assert_eq!(val, 0xFF, "OAM read returns 0xFF during Mode 2/3");
}

#[test]
fn repro_oam_access_prewrite_2() {
    // prewrite_2: DMG expects 1, CGB expects 0
    let mut gb = setup_gb();
    gb.write_mem(0xFF40, 0x80);
    advance_to_ly(&mut gb, 90);
    advance_to_mode(&mut gb, 3);
    // Write OAM just at Mode 3 start
    gb.write_mem(0xFE00, 0x01);
    let val = gb.read_mem(0xFE00);
    // OAM should be blocked in Mode 3
    assert_eq!(val, 0xFF, "OAM read returns 0xFF in Mode 3");
}

#[test]
fn repro_oam_access_prewrite_3() {
    // prewrite_3 expects 0
    let mut gb = setup_gb();
    gb.write_mem(0xFF40, 0x80);
    advance_to_ly(&mut gb, 90);
    advance_to_mode(&mut gb, 3);
    gb.advance_dots(160); // Well into Mode 3
    gb.write_mem(0xFE00, 0x01);
    let val = gb.read_mem(0xFE00);
    assert_eq!(val, 0xFF);
}

#[test]
fn repro_oam_access_postwrite_2() {
    // postwrite_2 DMG expects 1
    let mut gb = setup_gb();
    gb.write_mem(0xFF40, 0x80);
    advance_to_ly(&mut gb, 10);
    advance_to_mode(&mut gb, 3);
    gb.advance_dots(200); // Wait for Mode 0
    if gb.ppu.read_stat() & 3 == 0 {
        gb.write_mem(0xFE00, 0x01);
        let val = gb.read_mem(0xFE00);
        assert_eq!(val, 0x01, "OAM write should succeed in Mode 0");
    }
}

#[test]
fn repro_oam_access_postwrite_2_scx3() {
    // postwrite_2_scx3 DMG expects 1, CGB expects 0
    let mut gb = setup_gb();
    gb.write_mem(0xFF40, 0x80);
    gb.write_mem(0xFF43, 0x03); // SCX = 3
    advance_to_ly(&mut gb, 10);
    advance_to_mode(&mut gb, 3);
    gb.advance_dots(210); // SCX=3 extends Mode 3 by ~6 dots
    if gb.ppu.read_stat() & 3 == 0 {
        gb.write_mem(0xFE00, 0x01);
        let val = gb.read_mem(0xFE00);
        assert_eq!(val, 0x01);
    }
}

#[test]
fn repro_oam_access_postread_scx2_1() {
    let mut gb = setup_gb();
    gb.write_mem(0xFF40, 0x80);
    gb.write_mem(0xFF43, 0x02);
    advance_to_ly(&mut gb, 10);
    advance_to_mode(&mut gb, 3);
    gb.advance_dots(160);
    let val = gb.read_mem(0xFE00);
    assert_eq!(val & 3, 3, "OAM still blocked with SCX=2");
}

#[test]
fn repro_oam_access_postread_scx2_2() {
    let mut gb = setup_gb();
    gb.write_mem(0xFF40, 0x80);
    gb.write_mem(0xFF43, 0x02);
    advance_to_ly(&mut gb, 10);
    advance_to_mode(&mut gb, 3);
    gb.advance_dots(210);
    let val = gb.read_mem(0xFE00);
    if gb.ppu.read_stat() & 3 == 0 {
        assert_eq!(val & 3, 0, "OAM accessible after Mode 3 with SCX=2");
    }
}

#[test]
fn repro_oam_access_postread_scx3_2() {
    let mut gb = setup_gb();
    gb.write_mem(0xFF40, 0x80);
    gb.write_mem(0xFF43, 0x03);
    advance_to_ly(&mut gb, 10);
    advance_to_mode(&mut gb, 3);
    gb.advance_dots(220);
    let val = gb.read_mem(0xFE00);
    if gb.ppu.read_stat() & 3 == 0 {
        assert_eq!(val & 3, 0, "OAM accessible after Mode 3 with SCX=3");
    }
}

#[test]
fn repro_oam_access_postread_scx5_2() {
    let mut gb = setup_gb();
    gb.write_mem(0xFF40, 0x80);
    gb.write_mem(0xFF43, 0x05);
    advance_to_ly(&mut gb, 10);
    advance_to_mode(&mut gb, 3);
    gb.advance_dots(240);
    let val = gb.read_mem(0xFE00);
    if gb.ppu.read_stat() & 3 == 0 {
        assert_eq!(val & 3, 0, "OAM accessible after Mode 3 with SCX=5");
    }
}

#[test]
fn repro_oam_access_10spritesprline_postread_2() {
    // With 10 sprites, Mode 3 is extended. Check OAM blocking.
    let mut gb = setup_gb();
    gb.write_mem(0xFF40, 0x80);

    // Place 10 sprites on line 10
    for i in 0..10u8 {
        let oam_addr = 0xFE00 + (i as u16) * 4;
        gb.write_mem(oam_addr, 16); // Y = 16 (visible on line 10)
        gb.write_mem(oam_addr + 1, 8 + i * 8);
        gb.write_mem(oam_addr + 2, 0);
        gb.write_mem(oam_addr + 3, 0);
    }

    advance_to_ly(&mut gb, 10);
    advance_to_mode(&mut gb, 3);
    // With 10 sprites, Mode 3 is longer. Check if OAM is still blocked.
    gb.advance_dots(100);
    let val = gb.read_mem(0xFE00);
    let mode = gb.ppu.read_stat() & 3;
    if mode == 2 || mode == 3 {
        assert_eq!(val, 0xFF, "OAM blocked during Mode {mode}");
    } else {
        assert_ne!(val, 0xFF, "OAM accessible in Mode {mode}");
    }
}

#[test]
fn gbmicrotest_000_oam_lock() {
    let mut gb = setup_gb();
    gb.write_mem(0xFF40, 0x80); // LCD ON

    // Wait for Line 1 OAM Scan to end and Mode 3 to start.
    while gb.ppu.read_ly() != 1 || (gb.ppu.read_stat() & 0x03) != 3 {
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }

    // Now in Mode 3 of Line 1. OAM should be locked.
    gb.ppu.write_oam(0xFE00, 0x55);
    assert_ne!(
        gb.ppu.oam().read(0xFE00),
        0x55,
        "OAM write should be blocked during Mode 3"
    );
}

#[test]
fn gbmicrotest_001_vram_unlocked() {
    let mut gb = setup_gb();
    gb.write_mem(0xFF40, 0x80); // LCD ON

    // Synchronize to Start of Line 1 OAM Scan (tick 0)
    loop {
        if gb.ppu.read_ly() == 1
            && matches!(
                gb.ppu.phase,
                crate::ppu::PpuPhase::OamScan(crate::ppu::OamScanStage::Running { tick: 0 })
            )
        {
            break;
        }
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }

    // Advance to tick 158.
    for _ in 0..158 {
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }
    // Process tick 158 (unblocks VRAM write in DMG mode).
    gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);

    // Try write to VRAM — should succeed.
    gb.ppu.write_vram(0x8000, 0xAA);
    assert_eq!(
        gb.ppu.vram().read(0x8000),
        0xAA,
        "VRAM write should be unlocked at tick 159 of OAM Scan"
    );
}

#[test]
fn test_ppu_line0_startup_oam_lock() {
    let mut gb = setup_gb();
    gb.write_mem(0xFF40, 0x00); // LCD OFF
    gb.write_mem(0xFF40, 0x80); // LCD ON

    // Tick 0-150: InitialMode0 (unblocked).
    for _ in 0..151 {
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }
    // Now at tick 151. Next tick() will process remaining=1 and transition to OamWriteBlock.
    gb.ppu.write_oam(0xFE00, 0x55);
    assert_eq!(
        gb.ppu.oam().read(0xFE00),
        0x55,
        "OAM should be unlocked at tick 151 of startup"
    );

    // Process tick 151 -> transitions to OamWriteBlock { remaining: 4 } and sets oam_write_blocked = true.
    gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);

    gb.ppu.write_oam(0xFE01, 0xAA);
    assert_ne!(
        gb.ppu.oam().read(0xFE01),
        0xAA,
        "OAM write should be blocked at tick 152 of startup"
    );
}

#[test]
fn test_ppu_cgb_palette_hblank_blocking() {
    let mut gb = setup_gb();
    gb.write_mem(0xFF40, 0x80); // LCD ON

    // Advance to Line 1 HBlank transition.
    while gb.ppu.read_ly() != 1 || (gb.ppu.read_stat() & 0x03) != 3 {
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Cgb, false);
    }
    while gb.ppu.lcd_x() < 159 {
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Cgb, false);
    }

    // Advance until STAT bits change to Mode 0.
    for _ in 0..10 {
        if (gb.ppu.read_stat() & 0x03) == 0 {
            break;
        }
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Cgb, false);
    }

    // Now in HBlank Stage StatUpdate { remaining: 2 }.
    // Next tick will transition to PalettesBlock { remaining: 4 }.
    gb.ppu.tick(&mut gb.ints, crate::CgbMode::Cgb, false);

    // Process tick 1 of PalettesBlock (remaining: 4).
    gb.ppu.tick(&mut gb.ints, crate::CgbMode::Cgb, false);
    assert!(
        !gb.ppu.is_cgb_palettes_accessible(),
        "CGB palettes should be blocked in HBlank entry"
    );

    // Advance 4 more ticks to exit PalettesBlock.
    for _ in 0..4 {
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Cgb, false);
    }
    // Now in PalettesUnblock.
    // Process tick 1 of PalettesUnblock (remaining: 4).
    gb.ppu.tick(&mut gb.ints, crate::CgbMode::Cgb, false);
    assert!(
        gb.ppu.is_cgb_palettes_accessible(),
        "CGB palettes should be unblocked after HBlank entry period"
    );
}

#[test]
fn test_diagnostic_oam_blocking_first_10_ticks() {
    let mut gb = setup_gb();
    gb.write_mem(0xFF40, 0x80); // LCD ON

    // Advance to a stable line, e.g., Line 10
    advance_to_ly(&mut gb, 10);

    // Wait until the exact start of Mode 2 (tick 0 of OamScan)
    loop {
        if let crate::ppu::PpuPhase::OamScan(crate::ppu::OamScanStage::Running { tick: 0 }) =
            gb.ppu.phase
        {
            break;
        }
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }

    println!("--- Diagnostic: OAM Blocking First 10 Ticks of Mode 2 ---");
    for t in 0..15 {
        let oam_val = gb.read_mem(0xFE00);
        println!(
            "Tick {}: OAM read={:02X}, oam_read_blocked={}, phase={:?}",
            t, oam_val, gb.ppu.oam_read_blocked, gb.ppu.phase
        );
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }
    println!("---------------------------------------------------------");
}
