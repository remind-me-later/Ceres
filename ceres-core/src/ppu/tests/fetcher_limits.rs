use super::*;

#[test]
fn test_ppu_fetcher_first_tile_priming() {
    let mut gb = setup_gb();
    gb.write_mem(0xFF40, 0x80); // LCD ON
    gb.write_mem(0xFF43, 0x03); // SCX = 3 (discard first 3 pixels)

    // Advance to a steady-state scanline (line 2) to avoid Line 0 startup quirks.
    advance_to_ly(&mut gb, 2);
    // Advance to exactly the start of Mode 3
    advance_to_mode(&mut gb, 3);

    // At the very first tick of Mode 3, the fetcher should be in its first cycle
    // and lcd_x should still be 0.
    assert_eq!(gb.ppu.lcd_x(), 0);

    // Distill: The fetcher must take exactly 6 dots (12 ticks) to push the first 8 pixels.
    // Before those 12 ticks are up, the sequencer should be stalled (not popping).
    for _ in 0..11 {
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
        assert_eq!(
            gb.ppu.lcd_x(),
            0,
            "Sequencer started before first tile was fetched"
        );
    }
}

#[test]
fn test_ppu_scx_discard_logic() {
    let mut gb = setup_gb();
    gb.write_mem(0xFF43, 0x05); // SCX = 5
    gb.write_mem(0xFF40, 0x80); // LCD ON

    // Wait for a normal line (line 2) to avoid startup quirks
    advance_to_ly(&mut gb, 2);
    advance_to_mode(&mut gb, 3);

    // Advance until the first tile is pushed to FIFO (usually ~12 ticks)
    while gb.ppu.bg_fifo_size() == 0 {
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }

    // Distill: When the first 8 pixels are pushed, pixel_discard_count MUST be 5.
    // If it's 0, the first 8 pixels of the line will be shifted or flicker.
    assert_eq!(gb.ppu.pixel_discard_count(), 5);
}

#[test]
fn test_ppu_window_activation_at_start_of_line() {
    let mut gb = setup_gb();
    gb.write_mem(0xFF4A, 0); // WY = 0
    gb.write_mem(0xFF4B, 7); // WX = 7 (matches absolute left edge)
    gb.write_mem(0xFF40, 0xA1); // LCD ON, Window ON, BG ON

    // Wait for a normal line (line 2) to avoid startup quirks
    advance_to_ly(&mut gb, 2);
    advance_to_mode(&mut gb, 3);

    // Distill: In this state, the PPU should immediately start a Window fetch
    // instead of a BG fetch if WX matches the start of the line.

    // Tick until we are at an even dot where the window can trigger.
    while !gb.ppu.dots_in_line().is_multiple_of(2) {
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }

    // It should trigger in the very first opportunity (within 2 ticks).
    for _ in 0..2 {
        if gb.ppu.window_is_being_fetched() {
            break;
        }
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }

    assert!(
        gb.ppu.window_is_being_fetched(),
        "Window should trigger at start of line for WX=7"
    );
}

#[test]
fn test_ppu_window_activation_with_scx_discard() {
    let mut gb = setup_gb();
    gb.write_mem(0xFF43, 0x03); // SCX = 3
    gb.write_mem(0xFF4A, 0); // WY = 0
    gb.write_mem(0xFF4B, 7); // WX = 7
    gb.write_mem(0xFF40, 0xA1); // LCD ON, Window ON, BG ON

    advance_to_ly(&mut gb, 2);
    advance_to_mode(&mut gb, 3);

    // Distill: Window at WX=7 should trigger at the start of the line
    // regardless of SCX discards.

    while !gb.ppu.dots_in_line().is_multiple_of(2) {
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }

    for _ in 0..2 {
        if gb.ppu.window_is_being_fetched() {
            break;
        }
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }

    assert!(
        gb.ppu.window_is_being_fetched(),
        "Window should trigger immediately regardless of SCX discards"
    );
}

#[test]
fn test_ppu_window_discard_logic_wx_less_than_7() {
    let mut gb = setup_gb();
    gb.write_mem(0xFF43, 0x00); // SCX = 0
    gb.write_mem(0xFF4A, 0); // WY = 0
    gb.write_mem(0xFF4B, 0); // WX = 0
    gb.write_mem(0xFF40, 0xA1); // LCD ON, Window ON, BG ON

    advance_to_ly(&mut gb, 2);
    advance_to_mode(&mut gb, 3);

    // WX=0 should discard 7 pixels from the first window tile.

    // Wait until window activates
    while !gb.ppu.window_is_being_fetched() {
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }

    // Advance until first tile is pushed
    while gb.ppu.bg_fifo_size() == 0 {
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }

    // Distill: At WX=0, we must discard 7 pixels.
    assert_eq!(
        gb.ppu.pixel_discard_count(),
        7,
        "Window at WX=0 should discard 7 pixels"
    );
}

#[test]
fn test_ppu_scx_mid_line_change_alignment() {
    let mut gb = setup_gb();
    gb.write_mem(0xFF43, 0x07); // SCX = 7
    gb.write_mem(0xFF40, 0x80); // LCD ON

    advance_to_ly(&mut gb, 2);
    advance_to_mode(&mut gb, 3);

    // Now we are at the start of Mode 3. pixel_discard_count is latched to 7.
    assert_eq!(gb.ppu.pixel_discard_count(), 7);

    // Immediately change SCX to 0.
    gb.write_mem(0xFF43, 0x00);

    // Advance until first tile is pushed.
    while gb.ppu.bg_fifo_size() == 0 {
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }

    // The fetcher should have used the value of SCX that was active when it started
    // the fetch, OR it should be consistent with the discard count.
    // On hardware, SCX is latched for the duration of the fetch or at least at the start of the line.

    // If the first 8 pixels are flickering, it's likely because the fetcher used SCX=0
    // but the sequencer is still discarding 7 pixels.

    let _tile_addr = gb.ppu.fetcher_tile_index_addr();
    // SCX=7 -> Tile 0 (0..7). Discard 7. Show pixel 7.
    // SCX=0 -> Tile 0 (0..7). Discard 0. Show pixels 0..7.

    // In our test, SCX was 7 when Mode 3 started, so pixel_discard_count=7.
    // Then we changed SCX to 0.
    // If the fetcher uses SCX=0, it will fetch Tile 0.
    // But since discard=7, it will only show pixel 7 of Tile 0.
    // This is "correct" in the sense that Tile 0 contains pixel 7.

    // Wait, what if we change SCX to 8?
    // SCX=7 -> Tile 0.
    // SCX=8 -> Tile 1.
    // If fetcher uses SCX=8 but discard=7, it will show pixel 7 of Tile 1 as the first pixel!
    // That's definitely wrong and would cause a massive shift/flicker.
}

#[test]
fn test_ppu_scx_latching_consistency() {
    let mut gb = setup_gb();
    gb.write_mem(0xFF43, 0x07); // SCX = 7
    gb.write_mem(0xFF40, 0x80); // LCD ON

    advance_to_ly(&mut gb, 2);
    advance_to_mode(&mut gb, 3);

    // Latched discard count should be 7
    assert_eq!(gb.ppu.pixel_discard_count(), 7);

    // Immediately change SCX to 16 (Tile 2)
    gb.write_mem(0xFF43, 16);

    // Advance until first tile is fetched (T1 state finished)
    while !matches!(gb.ppu.fetcher_state(), crate::ppu::FetcherState::GetTileT2) {
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }

    // On hardware, the first tile fetch MUST be consistent with the discard count
    // latched at the start of the line. So it should still fetch Tile 0.
    let tile_map_base = if gb.read_mem(0xFF40) & 0x08 != 0 {
        0x9C00
    } else {
        0x9800
    };
    assert_eq!(
        gb.ppu.fetcher_tile_index_addr(),
        tile_map_base,
        "Fetcher used new SCX for first tile, causing misalignment with latched discard count!"
    );
}
