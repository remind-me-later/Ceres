use super::*;

#[test]
fn test_ppu_sprite_scan_timing() {
    let mut gb = setup_gb();
    // Sprite 0 at Y=17 (line 1), X=8, Tile=0
    gb.write_mem(0xFE00, 17);
    gb.write_mem(0xFE01, 8);
    gb.write_mem(0xFE02, 0);
    gb.write_mem(0xFE03, 0);

    gb.write_mem(0xFF40, 0x82); // LCD ON, OBJ ON

    // Wait for line 1 OAM Scan start (Line 0 has no OAM scan after power on)
    loop {
        if gb.ppu.read_ly() == 1
            && matches!(
                gb.ppu.phase,
                crate::ppu::PpuPhase::OamScan(crate::ppu::OamScanStage::Scanning { tick: 0 })
            )
        {
            break;
        }
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }

    // At line 1, tick 0, sprite_buffer should be empty (it was cleared at mode start)
    assert_eq!(gb.ppu.sprite_buffer_len(), 0);

    // Tick until tick 11 (DMG scans entry 0 at tick 8 + 2 = 10, observable at tick 11)
    // Tick 0 -> 1
    // Tick 1 -> 2
    // ...
    // Tick 10 -> 11
    for _ in 0..11 {
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }

    // At tick 11, sprite 0 should be scanned
    assert_eq!(
        gb.ppu.sprite_buffer_len(),
        1,
        "Sprite 0 should be scanned at tick 11 of line 1 (actual phase: {:?})",
        gb.ppu.phase
    );
}

#[test]
fn test_ppu_sprite_visibility_at_gambatte_timing() {
    let mut gb = setup_gb();
    // Clear OAM first
    for i in 0..160 {
        gb.write_mem(0xFE00 + i, 0);
    }

    // Sprite 0 at Y=17, X=8 (visible on Line 1)
    gb.write_mem(0xFE00, 17);
    gb.write_mem(0xFE01, 8);
    gb.write_mem(0xFE02, 0);
    gb.write_mem(0xFE03, 0);

    gb.write_mem(0xFF40, 0x82); // LCD ON, OBJ ON

    // Wait for Line 1 OAM scan start
    loop {
        if gb.ppu.read_ly() == 1
            && matches!(
                gb.ppu.phase,
                crate::ppu::PpuPhase::OamScan(crate::ppu::OamScanStage::Scanning { tick: 0 })
            )
        {
            break;
        }
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }

    // Advance to Mode 3 start
    while (gb.ppu.read_stat() & 0x03) != 3 {
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }

    // Sprite fetcher should start in Mode 3
    let mut fetcher_started = false;
    for _ in 0..1000 {
        if gb.ppu.sprite_fetcher_state != SpriteFetcherState::Idle {
            fetcher_started = true;
            break;
        }
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }

    assert!(fetcher_started, "Sprite fetcher should start in Mode 3");
}

#[test]
fn mooneye_intr_2_0_timing_sprites() {
    let mut gb = setup_gb();
    gb.write_mem(0xFF40, 0); // LCD OFF

    // Setup 1 sprite at X=8 (visible left edge), Y=82 (on scanline 66)
    gb.write_mem(0xFE00, 82);
    gb.write_mem(0xFE01, 8);

    gb.write_mem(0xFF40, 0x82); // LCD ON + OBJ ON

    // Wait until LY=66
    advance_to_ly(&mut gb, 66);
    advance_to_mode(&mut gb, 3);

    gb.write_mem(0xFF41, 0x20); // Mode 2 interrupt
    gb.ints.write_if(0);

    // Measure Mode 2
    let mut _mode2_tick = 0;
    for t in 0..10_000 {
        if gb.ints.read_if() & 0x02 != 0 {
            _mode2_tick = t;
            break;
        }
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }

    gb.ints.acknowledge_interrupt(0x02);
    gb.write_mem(0xFF41, 0x08); // Mode 0 interrupt

    let mut mode0_tick = 0;
    for t in 0..10_000 {
        if gb.ints.read_if() & 0x02 != 0 {
            mode0_tick = t;
            break;
        }
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }

    // Base duration for Mode 2 -> Mode 0 is ~504 ticks.
    // 1 Sprite adds roughly ~12 ticks (depending on exact sprite fetch).
    assert!(
        mode0_tick > 510,
        "Mode 0 interrupt was not delayed by sprites (took {} ticks, expected > 510)",
        mode0_tick
    );
}

#[test]
fn age_ppu_mode3_duration_sprites() {
    let mut gb = setup_gb();
    gb.write_mem(0xFF40, 0); // LCD OFF

    // Setup 10 sprites on line 66
    for i in 0..10 {
        let addr = 0xFE00 + (i * 4);
        gb.write_mem(addr, 82); // Y = 82 (line 66)
        gb.write_mem(addr + 1, 8 + (i as u8 * 8)); // X
    }

    gb.write_mem(0xFF40, 0x82); // LCD ON + OBJ ON

    advance_to_ly(&mut gb, 66);
    advance_to_mode(&mut gb, 3);

    let mut duration = 0;
    loop {
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
        duration += 1;
        if (gb.ppu.read_stat() & 0x03) != 3 {
            break;
        }
    }

    // 10 non-overlapping sprites should add 110 dots (220 ticks).
    // 336 + 220 = 556.
    assert_eq!(duration, 564, "Sprite Mode 3 penalty timing changed!");
}

#[test]
fn test_repro_sprite_m3_penalty_1_sprite() {
    let mut gb = setup_gb();
    gb.change_model_and_soft_reset(Model::CgbE);

    // Clear OAM
    for i in 0..160 {
        gb.ppu.write_oam_by_dma(0xFE00 + i, 0);
    }

    // Place 1 sprite at X=8, Y=16 (visible on Line 0)
    gb.ppu.write_oam_by_dma(0xFE00, 16); // Y
    gb.ppu.write_oam_by_dma(0xFE01, 8); // X
    gb.ppu.write_oam_by_dma(0xFE02, 0); // tile
    gb.ppu.write_oam_by_dma(0xFE03, 0); // attrs

    // LCDC = 0x82: LCD on, OBJ enable
    gb.write_mem(0xFF40, 0x82);

    // Wait for Mode 3 of Line 1 to start.
    // Line 0 skips OAM scan after LCD-on, so we must wait for Line 1 to see sprite penalty.
    loop {
        if gb.read_mem(0xFF44) == 1 && (gb.read_mem(0xFF41) & 0x03) == 3 {
            break;
        }
        gb.ppu.tick(&mut gb.ints, CgbMode::Cgb, false);
    }

    // Baseline Mode 3 (no sprites, no scroll, no window) is exactly 172 dots (344 ticks).
    // A single sprite at X=8 adds 11 ticks (DMG/CGB normal speed).
    // So Mode 3 should last at least 344 + 11 = 355 ticks.
    // If we advance 350 ticks, it SHOULD still be in Mode 3.
    for _ in 0..350 {
        gb.ppu.tick(&mut gb.ints, CgbMode::Cgb, false);
    }

    let mode = gb.read_mem(0xFF41) & 0x03;
    assert_eq!(
        mode, 3,
        "PPU should still be in Mode 3 at tick 350 with 1 sprite penalty (Mode 3 baseline=344), got Mode {}",
        mode
    );
}

#[test]
fn test_ppu_mode3_duration_4sprites_penalty() {
    let mut gb = setup_gb();
    gb.write_mem(0xFF40, 0x82); // LCD ON, OBJ enable

    // Place 4 sprites all at X=8, 16, 24, 32 so they are fully fetched.
    for i in 0..4 {
        let base = i * 4;
        gb.ppu.write_oam_by_dma(0xFE00 + base, 16); // Y = 16 (LY 0)
        gb.ppu.write_oam_by_dma(0xFE00 + base + 1, 8 + i as u8 * 8); // X
        gb.ppu.write_oam_by_dma(0xFE00 + base + 2, 0); // tile
        gb.ppu.write_oam_by_dma(0xFE00 + base + 3, 0); // attrs
    }

    advance_to_ly(&mut gb, 1);
    let duration = mode3_duration_ticks(&mut gb, 1, crate::CgbMode::Dmg, false);

    // Base duration 344 + (4 * 11 dots) * 2 T-cycles/dot = 344 + 88 = 432
    assert_eq!(
        duration, 432,
        "Mode-3 duration with 4 full sprites should be 432 T-ticks, got {}",
        duration
    );
}

#[test]
fn test_ppu_mode3_duration_with_sprites() {
    let mut gb = setup_gb();
    gb.write_mem(0xFF40, 0x83); // LCD ON, BG ON, OBJ ON

    // Clear OAM.
    for i in (0..160).step_by(4) {
        gb.write_mem(0xFE00 + i, 0); // Y=0 (hidden)
    }

    // Baseline: No sprites.
    gb.write_mem(0xFF43, 0); // SCX = 0
    advance_to_ly(&mut gb, 144); // Wait for VBlank to ensure clean state
    advance_to_ly(&mut gb, 10);

    // Wait for Mode 3 start.
    while (gb.ppu.read_stat() & 0x03) != 3 {
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }
    let start_tick = gb.ppu.dots_in_line();

    // Wait for Mode 0 start.
    while (gb.ppu.read_stat() & 0x03) != 0 {
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }
    let end_tick = gb.ppu.dots_in_line();
    let baseline_duration = end_tick - start_tick;

    // One sprite at X=16.
    gb.ppu.write_oam_by_dma(0xFE00, 20); // Y = 20 (Line 4)
    gb.ppu.write_oam_by_dma(0xFE01, 16); // X = 16
    gb.ppu.write_oam_by_dma(0xFE02, 0);
    gb.ppu.write_oam_by_dma(0xFE03, 0);

    // Advance to VBlank, then to line 4.
    while gb.ppu.read_ly() != 144 {
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }
    advance_to_ly(&mut gb, 4);

    // Check sprite buffer after OAM scan (tick 168)
    while gb.ppu.dots_in_line() < 168 {
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }
    println!(
        "Sprite buffer count after OAM scan on Line 4: {}",
        gb.ppu.sprite_buffer_len()
    );

    // Wait for Mode 3 start.
    while (gb.ppu.read_stat() & 0x03) != 3 {
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }
    let start_tick = gb.ppu.dots_in_line();

    // Wait for Mode 0 start.
    while (gb.ppu.read_stat() & 0x03) != 0 {
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }
    let end_tick = gb.ppu.dots_in_line();
    let one_sprite_duration = end_tick - start_tick;

    println!(
        "Baseline duration: {}, One sprite duration: {}",
        baseline_duration, one_sprite_duration
    );

    assert!(
        one_sprite_duration > baseline_duration,
        "Mode 3 should be longer with one sprite (baseline: {}, one sprite: {})",
        baseline_duration,
        one_sprite_duration
    );
}

#[test]
fn test_ppu_sprite_background_priority() {
    let mut gb = setup_gb();
    // LCD ON, BG ON, OBJ ON
    gb.write_mem(0xFF40, 0x83);
    gb.write_mem(0xFF47, 0xE4); // BGP: 11 10 01 00
    gb.write_mem(0xFF48, 0xE4); // OBP0: 11 10 01 00

    // Set a background tile at (0,0) with color 1.
    // Tile map at 0x9800.
    gb.ppu.write_vram(0x9800, 1);
    // Tile data for tile 1 at 0x8010.
    // Row 0: all pixels color 1.
    gb.ppu.write_vram(0x8010, 0xFF);
    gb.ppu.write_vram(0x8011, 0x00);

    // Set a sprite at X=8, Y=16 (covers first tile of line 0).
    // Sprite tile 2 at 0x8020.
    // Row 0: all pixels color 2.
    gb.ppu.write_vram(0x8020, 0x00);
    gb.ppu.write_vram(0x8021, 0xFF);

    gb.ppu.write_oam(0xFE00, 16); // Y = 16
    gb.ppu.write_oam(0xFE01, 8); // X = 8
    gb.ppu.write_oam(0xFE02, 2); // Tile = 2
    gb.ppu.write_oam(0xFE03, 0); // Flags: Priority = Above BG

    // Advance to line 0, middle of first tile.
    while gb.ppu.read_ly() != 0 || gb.ppu.lcd_x() < 4 {
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }

    // Above BG priority: Sprite color 2 should win over BG color 1.
    let pixel_data = gb.ppu.rgba_buf().pixel_data();
    let px = (pixel_data[0], pixel_data[1], pixel_data[2]);
    // Mono shade for color 2 is 2 (dark gray).
    let expected_rgb = color_palette::GRAYSCALE_PALETTE[2];
    assert_eq!(px, expected_rgb, "Sprite (Above BG) should win over BG");

    // Change sprite to Behind BG priority.
    // Reset and run again.
    let mut gb = setup_gb();
    gb.write_mem(0xFF40, 0x83);
    gb.write_mem(0xFF47, 0xE4);
    gb.write_mem(0xFF48, 0xE4);
    gb.ppu.write_vram(0x9800, 1);
    gb.ppu.write_vram(0x8010, 0xFF);
    gb.ppu.write_vram(0x8011, 0x00);
    gb.ppu.write_vram(0x8020, 0x00);
    gb.ppu.write_vram(0x8021, 0xFF);
    gb.ppu.write_oam(0xFE00, 16);
    gb.ppu.write_oam(0xFE01, 8);
    gb.ppu.write_oam(0xFE02, 2);
    gb.ppu.write_oam(0xFE03, 0x80); // Behind BG

    while gb.ppu.read_ly() != 0 || gb.ppu.lcd_x() < 4 {
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }

    // Behind BG priority: BG color 1 should win over Sprite color 2.
    let pixel_data = gb.ppu.rgba_buf().pixel_data();
    let px = (pixel_data[0], pixel_data[1], pixel_data[2]);
    let expected_rgb = color_palette::GRAYSCALE_PALETTE[1];
    assert_eq!(
        px, expected_rgb,
        "BG should win over Sprite (Behind BG) when BG is non-zero"
    );
}

#[test]
fn test_ppu_sprite_background_shift_repro() {
    let mut gb = setup_gb();

    // Clear OAM
    for i in 0..160 {
        gb.write_mem(0xFE00 + i, 0);
    }

    // Set up BG Tile 0 (all black) and Tile 1 (all white)
    // Actually let's just make the BG a sequence of colors so we can see shifts.
    for i in 0..16 {
        gb.write_mem(0x8000 + i, 0x00); // Tile 0: Color 0
        gb.write_mem(0x8010 + i, 0xFF); // Tile 1: Color 3
    }

    // Tile map: alternating 0 and 1
    for i in 0..32 {
        gb.write_mem(0x9800 + i, (i % 2) as u8);
    }

    // Set up a sprite at Y=17 (LY=1), X=24, Tile=2 (all Color 1)
    for i in 0..16 {
        gb.write_mem(0x8020 + i, 0x55); // Tile 2: Color 1 (if palette is setup)
    }
    gb.write_mem(0xFE00, 17);
    gb.write_mem(0xFE01, 24);
    gb.write_mem(0xFE02, 2);
    gb.write_mem(0xFE03, 0);

    // Setup palettes
    gb.write_mem(0xFF47, 0xE4); // BGP: 11 10 01 00 (Color 3=Black, 0=White)
    gb.write_mem(0xFF48, 0xE4); // OBP0

    // SCX = 0
    gb.write_mem(0xFF43, 0);

    // Run without sprite (move sprite off-screen)
    gb.write_mem(0xFE00, 0);
    gb.write_mem(0xFF40, 0x83); // LCD ON, BG ON, OBJ ON

    // Wait for end of Line 1
    while gb.ppu.read_ly() != 2 {
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }

    // Capture background pixels
    let mut expected_bg = Vec::new();
    let row_start = 1 * 160;
    for x in 0..160 {
        let rgb = gb.ppu.rgba_buf().pixel_data()
            [((row_start + x) as usize * 4)..((row_start + x) as usize * 4 + 3)]
            .to_vec();
        expected_bg.push(rgb);
    }

    // Reset and run WITH sprite
    let mut gb = setup_gb();
    for i in 0..160 {
        gb.write_mem(0xFE00 + i, 0);
    }
    for i in 0..16 {
        gb.write_mem(0x8000 + i, 0x00);
        gb.write_mem(0x8010 + i, 0xFF);
        gb.write_mem(0x8020 + i, 0x55);
    }
    for i in 0..32 {
        gb.write_mem(0x9800 + i, (i % 2) as u8);
    }
    gb.write_mem(0xFF47, 0xE4);
    gb.write_mem(0xFF48, 0xE4);
    gb.write_mem(0xFF43, 0);

    gb.write_mem(0xFE00, 17);
    gb.write_mem(0xFE01, 24);
    gb.write_mem(0xFE02, 2);
    gb.write_mem(0xFE03, 0);
    gb.write_mem(0xFF40, 0x83);

    while gb.ppu.read_ly() != 2 {
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }

    // Verify background didn't shift
    let mut actual_bg = Vec::new();
    for x in 0..160 {
        let rgb = gb.ppu.rgba_buf().pixel_data()
            [((row_start + x) as usize * 4)..((row_start + x) as usize * 4 + 3)]
            .to_vec();
        actual_bg.push(rgb);
    }

    // Sprite should be at X=16 to 23 (since X=24 means lcd_x = 24 - 8 = 16)
    // The background everywhere else should match exactly
    let mut diffs = 0;
    for x in 0..160 {
        if x < 16 || x >= 24 {
            if actual_bg[x as usize] != expected_bg[x as usize] {
                println!(
                    "Mismatch at X={}: Expected {:?}, got {:?}",
                    x, expected_bg[x as usize], actual_bg[x as usize]
                );
                diffs += 1;
            }
        }
    }
    assert_eq!(diffs, 0, "Background shifted after sprite!");
}

#[test]
fn test_ppu_mode3_duration_formula_sprites() {
    let mut gb = setup_gb();
    // LCD ON, BG ON, OBJ ON
    gb.ppu.write_lcdc(0x83, &mut gb.ints);

    // Baseline: SCX=0, No sprites.
    // Hardware Mode 2 + Mode 3 duration is 512 ticks.
    // However, STAT Mode 2 is delayed by 4 ticks (starts at tick 4).
    // And STAT Mode 0 happens exactly when Mode 3 ends.
    // So the measured STAT duration is 512 - 4 = 508 ticks.
    // Plus 1 tick due to how dots_in_line is polled = 509 ticks.

    // 1 Sprite at X=8. Should add 11 dots (22 ticks) of penalty.
    // Total = 509 + 22 = 531 ticks.
    gb.ppu.write_oam(0, 26); // Y=26 (Visible on LY=10: 26 - 16 = 10)
    gb.ppu.write_oam(1, 8); // X=8
    gb.ppu.write_oam(2, 0); // Tile 0
    gb.ppu.write_oam(3, 0); // Attrs

    advance_to_ly(&mut gb, 10);
    while gb.ppu.mode() != crate::ppu::Mode::OamScan {
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }
    let start_ticks = gb.ppu.dots_in_line() - 1;
    while gb.ppu.mode() != crate::ppu::Mode::HBlank {
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }
    let duration = gb.ppu.dots_in_line() - start_ticks;

    assert_eq!(
        duration, 531,
        "Mode 3 duration with 1 sprite at X=8 should be 531 ticks (509 + 22)"
    );
}

#[test]
fn test_diagnostic_sprite_fetcher_stall_ticks() {
    let mut gb = setup_gb();
    // LCD ON, Sprites ON
    gb.ppu.write_lcdc(0x82, &mut gb.ints);
    // Sprite 0 at X=8 (first possible position), LY=10
    gb.ppu.write_oam(0xFE00, 10 + 16); // Y=10
    gb.ppu.write_oam(0xFE01, 8); // X=8
    gb.ppu.write_oam(0xFE02, 0); // Tile 0
    gb.ppu.write_oam(0xFE03, 0); // Flags

    advance_to_ly(&mut gb, 10);
    advance_to_mode(&mut gb, 3);

    println!("--- Diagnostic: Sprite Fetcher Stall Ticks (1 sprite at X=8) ---");

    let mut total_m3_ticks = 0;
    let mut stall_ticks = 0;
    while (gb.ppu.read_stat() & 0x03) == 3 {
        if gb.ppu.fetcher_suspended {
            stall_ticks += 1;
        }
        total_m3_ticks += 1;
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }

    println!("Total Mode 3 ticks: {}", total_m3_ticks);
    println!("Total Stall ticks: {}", stall_ticks);
    println!("Base (Total - Stall): {}", total_m3_ticks - stall_ticks);
    println!("---------------------------------------------------------------");
}

#[test]
fn test_diagnostic_mode3_sprite_penalty_scaling() {
    println!("--- Diagnostic: Mode 3 Sprite Penalty Scaling ---");
    for num_sprites in [0, 1, 5, 10] {
        let mut gb = setup_gb();
        gb.write_mem(0xFF40, 0x82); // LCD ON, OBJ ON

        // Place sprites at X=167
        for i in 0..num_sprites {
            let base = i as u16 * 4;
            gb.ppu.write_oam_by_dma(0xFE00 + base, 16); // Y = 16 (LY 0)
            gb.ppu.write_oam_by_dma(0xFE00 + base + 1, 167); // X = 167
        }

        // Wait for LY=1 OAM Scan
        while gb.ppu.read_ly() != 1 || (gb.ppu.read_stat() & 0x03) != 2 {
            gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
        }

        // Wait for Mode 3 start
        while (gb.ppu.read_stat() & 0x03) == 2 {
            gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
        }
        let m3_start = gb.ppu.dots_in_line();
        println!("m3_start = {}", m3_start);

        // Wait for Mode 0 start
        while (gb.ppu.read_stat() & 0x03) == 3 {
            gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
        }
        let m3_end = gb.ppu.dots_in_line();
        println!("m3_end = {}", m3_end);

        println!(
            "Sprites: {:2}, Mode 3 Duration: {} ticks",
            num_sprites,
            m3_end - m3_start
        );
    }
    println!("--------------------------------------------------");
}

#[test]
fn test_diagnostic_sprite_fetcher_state_machine() {
    println!("--- Diagnostic: Sprite Fetcher State Machine Trace ---");
    let mut gb = setup_gb();
    gb.write_mem(0xFF40, 0x82); // LCD ON, OBJ ON

    // Place a single sprite at X=8
    gb.ppu.write_oam_by_dma(0xFE00, 16); // Y = 16 (LY 0)
    gb.ppu.write_oam_by_dma(0xFE01, 8); // X = 8

    advance_to_ly(&mut gb, 1);

    // Wait for Mode 3 start
    while (gb.ppu.read_stat() & 0x03) != 3 {
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }

    let mut ticks = 0;
    while (gb.ppu.read_stat() & 0x03) == 3 {
        let phase = &gb.ppu.phase;
        let suspended = gb.ppu.fetcher_suspended;
        let dots = gb.ppu.dots_in_line();
        let state = gb.ppu.fetcher_state;
        let size = gb.ppu.bg_fifo.size();

        // Only print when suspended
        if suspended {
            println!(
                "Tick {:3}: suspended={}, dots={}, phase={:?}, fetcher_state={:?}, fifo={}",
                ticks, suspended, dots, phase, state, size
            );
        }

        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
        ticks += 1;
    }
    println!("------------------------------------------------------");
}
