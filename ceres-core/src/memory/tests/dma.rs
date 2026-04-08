use super::*;

fn setup_cgb() -> Gb {
    let mut gb = GbBuilder::new(44100, DummyAudio)
        .with_model(Model::CgbE)
        .build();
    // Disable bootrom: writing any odd value to 0xFF50 locks it out.
    gb.write_mem(0xFF50, 0x01);
    // LCD off — VRAM accessible at all times (no Mode 3 lock).
    gb.write_mem(0xFF40, 0x00);
    gb
}

fn run_gdma(gb: &mut Gb, src: u16, dst_vram_offset: u16, len_blocks: u8) {
    // HDMA1: src high byte
    gb.write_mem(0xFF51, (src >> 8) as u8);
    // HDMA2: src low byte (lower nibble ignored — must be 0-aligned)
    gb.write_mem(0xFF52, src as u8);
    // HDMA3: dst high byte (only bits [12:8] used, bit 4 forced 0)
    gb.write_mem(0xFF53, (dst_vram_offset >> 8) as u8);
    // HDMA4: dst low byte (lower nibble ignored)
    gb.write_mem(0xFF54, dst_vram_offset as u8);
    // HDMA5: trigger General DMA (bit 7 = 0), length = (len_blocks - 1)
    gb.write_mem(0xFF55, len_blocks - 1);
    gb.run_hdma();
}

#[test]
fn gdma_dst_encoding_1f00() {
    let mut gb = setup_cgb();

    // Source: single byte 0x01 in WRAM
    gb.write_mem(0xC000, 0x01);
    // Dest: VRAM index 0x1F00 (address 0x9F00)
    run_gdma(&mut gb, 0xC000, 0x1F00, 1);

    // Destination received the transferred byte
    assert_eq!(
        gb.read_mem(0x9F00),
        0x01,
        "VRAM[0x9F00] should hold transferred value 0x01"
    );
    // VRAM[0x8000] must be unaffected
    assert_eq!(
        gb.read_mem(0x8000),
        0x00,
        "VRAM[0x8000] must not be touched (dst was 0x9F00)"
    );
}

#[test]
fn gdma_dst_encoding_1ff0() {
    let mut gb = setup_cgb();

    gb.write_mem(0xC000, 0x01);
    run_gdma(&mut gb, 0xC000, 0x1FF0, 1);

    assert_eq!(
        gb.read_mem(0x9FF0),
        0x01,
        "VRAM[0x9FF0] should hold transferred value 0x01"
    );
    assert_eq!(
        gb.read_mem(0x8000),
        0x00,
        "VRAM[0x8000] must not be touched (dst was 0x9FF0)"
    );
}

#[test]
fn gdma_src_hram_reads_ff() {
    let mut gb = setup_cgb();

    // Fill HRAM with distinct non-0xFF values
    for i in 0u8..8 {
        gb.write_mem(0xFF80 + u16::from(i), i + 1);
    }
    // Verify the writes took effect before the transfer
    for i in 0u8..8 {
        assert_eq!(gb.read_mem(0xFF80 + u16::from(i)), i + 1);
    }

    // GDMA: src=0xFF80 → VRAM 0x8000, 1 block (16 bytes)
    run_gdma(&mut gb, 0xFF80, 0x0000, 1);

    // On CGB, HRAM is not a valid GDMA source: reads return 0xFF
    for i in 0u16..8 {
        assert_eq!(
            gb.read_mem(0x8000 + i),
            0xFF,
            "VRAM[0x{:04X}] should be 0xFF (HRAM unreadable during GDMA)",
            0x8000 + i
        );
    }
}

#[test]
fn gdma_src_hram_result_is_ff() {
    let mut gb = setup_cgb();

    for i in 0u8..8 {
        gb.write_mem(0xFF80 + u16::from(i), i + 1);
    }

    run_gdma(&mut gb, 0xFF80, 0x0000, 1);

    let result = gb.read_mem(0x8007);
    assert_eq!(
        result, 0xFF,
        "VRAM[0x8007] should be 0xFF when GDMA source is HRAM"
    );
    // Equivalent to the ROM's `sub a, 0xFE` check
    assert_eq!(
        result.wrapping_sub(0xFE),
        0x01,
        "0xFF - 0xFE should equal 0x01"
    );
}

#[test]
fn gdma_src_oam_reads_ff() {
    let mut gb = setup_cgb();

    // Fill OAM with distinct non-0xFF values via direct write_mem.
    // OAM writes are only blocked when OAM DMA (`dma.blocks_oam()`) is active;
    // `dma` is inactive here so this succeeds.
    for i in 0u8..8 {
        gb.write_mem(0xFE00 + u16::from(i), i + 1);
    }

    run_gdma(&mut gb, 0xFE00, 0x0000, 1);

    for i in 0u16..8 {
        assert_eq!(
            gb.read_mem(0x8000 + i),
            0xFF,
            "VRAM[0x{:04X}] should be 0xFF (OAM unreadable during GDMA)",
            0x8000 + i
        );
    }
}

#[test]
fn gdma_src_vram_reads_ff() {
    let mut gb = setup_cgb();

    // Write to VRAM at 0x9000 (LCD is off so VRAM is accessible)
    for i in 0u8..8 {
        gb.write_mem(0x9000 + u16::from(i), i + 1);
    }
    // Sanity-check that the write landed
    assert_eq!(gb.read_mem(0x9000), 0x01);

    run_gdma(&mut gb, 0x9000, 0x0000, 1);

    for i in 0u16..8 {
        assert_eq!(
            gb.read_mem(0x8000 + i),
            0xFF,
            "VRAM[0x{:04X}] should be 0xFF (VRAM unreadable as GDMA source)",
            0x8000 + i
        );
    }
}

#[test]
fn gdma_src_address_fixup_above_e000() {
    let mut gb = setup_cgb();

    // write_hdma1(0xFF): src = 0xFF00 | 0x00 = 0xFF00, which is ≥ 0xE000
    // → src |= 0xF000 → 0xFF00 (no-op, already has F000)
    gb.write_mem(0xFF51, 0xFF);
    // write_hdma2(0xF0): src_lo = 0xF0, so src = 0xFF00 | 0xF0 = 0xFFF0
    gb.write_mem(0xFF52, 0xF0);
    // dst = 0x9FF0
    gb.write_mem(0xFF53, 0xFF);
    gb.write_mem(0xFF54, 0xF0);
    // Trigger 1-block GDMA
    gb.write_mem(0xFF55, 0x00);
    gb.run_hdma();

    // HRAM / high IO space returns 0xFF during GDMA — verify transfer filled
    // VRAM 0x9FF0..0x9FFF with 0xFF
    for i in 0u16..16 {
        assert_eq!(
            gb.read_mem(0x9FF0 + i),
            0xFF,
            "VRAM[0x{:04X}] should be 0xFF (HRAM/IO unreadable during GDMA src)",
            0x9FF0 + i
        );
    }
}

#[test]
fn hdma1_reads_ff() {
    let mut gb = setup_cgb();

    // Read before any write — should be 0xFF
    assert_eq!(
        gb.read_mem(0xFF51),
        0xFF,
        "FF51 should read 0xFF before any write"
    );

    // Write a value and trigger a GDMA
    gb.write_mem(0xC000, 0xAB); // something in WRAM
    run_gdma(&mut gb, 0xC000, 0x0000, 1);

    // After GDMA, FF51 must still read 0xFF (write-only register)
    assert_eq!(
        gb.read_mem(0xFF51),
        0xFF,
        "FF51 should read 0xFF after GDMA (write-only)"
    );
}

#[test]
fn hdma4_reads_ff() {
    let mut gb = setup_cgb();

    assert_eq!(
        gb.read_mem(0xFF54),
        0xFF,
        "FF54 should read 0xFF before any write"
    );

    gb.write_mem(0xC000, 0xCD);
    run_gdma(&mut gb, 0xC000, 0x0080, 1);

    assert_eq!(
        gb.read_mem(0xFF54),
        0xFF,
        "FF54 should read 0xFF after GDMA (write-only)"
    );
}

#[test]
fn hdma2_reads_ff() {
    let mut gb = setup_cgb();

    // Before any write
    assert_eq!(
        gb.read_mem(0xFF52),
        0xFF,
        "FF52 should read 0xFF before write"
    );

    // Write a value and trigger a GDMA
    gb.write_mem(0xC000, 0xAB);
    run_gdma(&mut gb, 0xC000, 0x0000, 1);

    // After GDMA, FF52 must still read 0xFF (write-only)
    assert_eq!(
        gb.read_mem(0xFF52),
        0xFF,
        "FF52 should read 0xFF after GDMA (write-only)"
    );
}

#[test]
fn hdma3_reads_ff() {
    let mut gb = setup_cgb();

    // Before any write
    assert_eq!(
        gb.read_mem(0xFF53),
        0xFF,
        "FF53 should read 0xFF before write"
    );

    // Write a value and trigger a GDMA
    gb.write_mem(0xC000, 0xCD);
    run_gdma(&mut gb, 0xC000, 0x0080, 1);

    // After GDMA, FF53 must still read 0xFF (write-only)
    assert_eq!(
        gb.read_mem(0xFF53),
        0xFF,
        "FF53 should read 0xFF after GDMA (write-only)"
    );
}

#[test]
fn hdma5_bit7_set_when_idle() {
    let mut gb = setup_cgb();

    gb.write_mem(0xC000, 0x42);
    run_gdma(&mut gb, 0xC000, 0x0000, 1);

    let hdma5 = gb.read_mem(0xFF55);
    assert_eq!(
        hdma5 & 0x80,
        0x80,
        "FF55 bit 7 should be 1 (no transfer active) after GDMA completes, got 0x{hdma5:02X}"
    );
}

#[test]
fn hdma5_bit7_clear_when_hblank_dma_active() {
    let mut gb = setup_cgb();

    gb.write_mem(0xC000, 0x11);
    // Program source/destination but do NOT call run_gdma — set up HBlank DMA instead
    gb.write_mem(0xFF51, 0xC0);
    gb.write_mem(0xFF52, 0x00);
    gb.write_mem(0xFF53, 0x80);
    gb.write_mem(0xFF54, 0x00);
    // Start 128-block HBlank DMA (bit 7 = 1 → HBlank mode, len_blocks - 1 = 0x7F)
    gb.write_mem(0xFF55, 0xFF);

    let hdma5 = gb.read_mem(0xFF55);
    assert_eq!(
        hdma5 & 0x80,
        0x00,
        "FF55 bit 7 should be 0 (transfer active) immediately after starting HBlank DMA, got 0x{hdma5:02X}"
    );
}

#[test]
fn gdma_len_encoding_two_blocks() {
    let mut gb = setup_cgb();

    // Fill 32 bytes of WRAM with distinct values
    for i in 0u8..32 {
        gb.write_mem(0xC000 + u16::from(i), i.wrapping_add(1));
    }

    // Trigger 2-block GDMA (N=1 → 32 bytes)
    run_gdma(&mut gb, 0xC000, 0x0000, 2);

    for i in 0u8..32 {
        assert_eq!(
            gb.read_mem(0x8000 + u16::from(i)),
            i.wrapping_add(1),
            "VRAM[0x{:04X}] should be 0x{:02X} (2-block GDMA)",
            0x8000 + u16::from(i),
            i.wrapping_add(1)
        );
    }
    // Byte 32 (just outside the 2-block window) must be untouched
    assert_eq!(
        gb.read_mem(0x8020),
        0x00,
        "VRAM[0x8020] must not be written (outside 2-block transfer)"
    );
}

#[test]
fn gdma_uses_wram_bank_active_at_transfer_time() {
    let mut gb = setup_cgb();

    // Write 0xAA to 0xD000 in WRAM bank 1 (SVBK=1)
    gb.write_mem(0xFF70, 0x01);
    gb.write_mem(0xD000, 0xAA);

    // Write 0xBB to 0xD000 in WRAM bank 2 (SVBK=2)
    gb.write_mem(0xFF70, 0x02);
    gb.write_mem(0xD000, 0xBB);

    // Bank 2 is active — GDMA from 0xD000 should copy bank-2 value (0xBB)
    run_gdma(&mut gb, 0xD000, 0x0000, 1);

    assert_eq!(
        gb.read_mem(0x8000),
        0xBB,
        "VRAM[0x8000] should contain 0xBB (WRAM bank 2 active during GDMA)"
    );
}

#[test]
fn oam_dma_copies_wram_to_oam() {
    let mut gb = setup_cgb();

    // Fill WRAM with recognisable pattern
    for i in 0u8..160 {
        gb.write_mem(0xC000 + u16::from(i), i);
    }

    // Start OAM DMA from 0xC000 (FF46 = 0xC0)
    gb.write_mem(0xFF46, 0xC0);

    // Advance enough dots for the full 160-byte transfer to complete.
    // Each byte takes 1 M-cycle (4 dots); startup delay is 2 M-cycles (8 dots).
    // Total: (160 + 2) * 4 = 648 dots → drive 650 to be safe.
    for _ in 0..650 {
        gb.advance_dots(1);
        gb.run_dma();
    }

    // Verify OAM received the WRAM data
    for i in 0u8..160 {
        assert_eq!(
            gb.read_mem(0xFE00 + u16::from(i)),
            i,
            "OAM[0x{:04X}] should be 0x{i:02X} after DMA from WRAM",
            0xFE00 + u16::from(i)
        );
    }
}

#[test]
fn gdma_from_wram_copies_correctly() {
    let mut gb = setup_cgb();

    for i in 0u8..16 {
        gb.write_mem(0xC000 + u16::from(i), i + 1);
    }

    run_gdma(&mut gb, 0xC000, 0x0000, 1);

    for i in 0u8..16 {
        assert_eq!(
            gb.read_mem(0x8000 + u16::from(i)),
            i + 1,
            "VRAM[0x{:04X}] should be 0x{:02X}",
            0x8000 + u16::from(i),
            i + 1
        );
    }
}

#[test]
fn gdma_from_rom_copies_ff() {
    let mut gb = setup_cgb();

    // Confirm VRAM[0x8000] starts as 0x00 (cleared at reset)
    assert_eq!(
        gb.read_mem(0x8000),
        0x00,
        "VRAM[0x8000] should be 0x00 before transfer"
    );

    // GDMA from ROM 0x0000 → VRAM 0x8000, 1 block (16 bytes)
    run_gdma(&mut gb, 0x0000, 0x0000, 1);

    // Default cartridge ROM is all 0xFF; GDMA must copy those values to VRAM
    for i in 0u16..16 {
        assert_eq!(
            gb.read_mem(0x8000 + i),
            0xFF,
            "VRAM[0x{:04X}] should be 0xFF after GDMA from ROM (default cart)",
            0x8000 + i
        );
    }
}

#[test]
fn oam_dma_from_vram_bank0() {
    let mut gb = setup_cgb();

    // Ensure VRAM bank 0 is selected
    gb.write_mem(0xFF4F, 0x00);

    // Write pattern to VRAM[0x8000..0x80A0]
    for i in 0u8..160 {
        gb.write_mem(0x8000 + u16::from(i), i.wrapping_add(1));
    }

    // Start OAM DMA from VRAM bank 0 (FF46 = 0x80)
    gb.write_mem(0xFF46, 0x80);

    // Run enough dots: 2 M-cycle startup + 160 bytes × 4 dots = 648 dots
    for _ in 0..650 {
        gb.advance_dots(1);
        gb.run_dma();
    }

    // Verify OAM received the VRAM data
    for i in 0u8..160 {
        assert_eq!(
            gb.read_mem(0xFE00 + u16::from(i)),
            i.wrapping_add(1),
            "OAM[0x{:04X}] should be 0x{:02X} after OAM DMA from VRAM",
            0xFE00 + u16::from(i),
            i.wrapping_add(1)
        );
    }
}

#[test]
fn oam_dma_vram_bank_active_at_transfer_time() {
    // --- Bank 0 ---
    {
        let mut gb = setup_cgb();

        // Write 0xAA to VRAM bank 0 at 0x8000
        gb.write_mem(0xFF4F, 0x00); // VBK = bank 0
        gb.write_mem(0x8000, 0xAA);

        // Write 0xBB to VRAM bank 1 at 0x8000
        gb.write_mem(0xFF4F, 0x01); // VBK = bank 1
        gb.write_mem(0x8000, 0xBB);

        // Select bank 0 and start OAM DMA from 0x8000
        gb.write_mem(0xFF4F, 0x00);
        gb.write_mem(0xFF46, 0x80);

        for _ in 0..650 {
            gb.advance_dots(1);
            gb.run_dma();
        }

        assert_eq!(
            gb.read_mem(0xFE00),
            0xAA,
            "OAM[0] should be 0xAA (VRAM bank 0 active during OAM DMA)"
        );
    }

    // --- Bank 1 ---
    {
        let mut gb = setup_cgb();

        gb.write_mem(0xFF4F, 0x00);
        gb.write_mem(0x8000, 0xAA);

        gb.write_mem(0xFF4F, 0x01);
        gb.write_mem(0x8000, 0xBB);

        // Select bank 1 and start OAM DMA from 0x8000
        gb.write_mem(0xFF4F, 0x01);
        gb.write_mem(0xFF46, 0x80);

        for _ in 0..650 {
            gb.advance_dots(1);
            gb.run_dma();
        }

        assert_eq!(
            gb.read_mem(0xFE00),
            0xBB,
            "OAM[0] should be 0xBB (VRAM bank 1 active during OAM DMA)"
        );
    }
}

#[test]
fn oam_dma_wram_bank_active_at_transfer_time() {
    // --- SVBK = 1 ---
    {
        let mut gb = setup_cgb();

        gb.write_mem(0xFF70, 0x01); // SVBK = bank 1
        gb.write_mem(0xD000, 0xAA);

        gb.write_mem(0xFF70, 0x02); // SVBK = bank 2
        gb.write_mem(0xD000, 0xBB);

        // Switch back to bank 1 and start OAM DMA from 0xD000
        gb.write_mem(0xFF70, 0x01);
        gb.write_mem(0xFF46, 0xD0);

        for _ in 0..650 {
            gb.advance_dots(1);
            gb.run_dma();
        }

        assert_eq!(
            gb.read_mem(0xFE00),
            0xAA,
            "OAM[0] should be 0xAA (WRAM bank 1 active during OAM DMA)"
        );
    }

    // --- SVBK = 2 ---
    {
        let mut gb = setup_cgb();

        gb.write_mem(0xFF70, 0x01);
        gb.write_mem(0xD000, 0xAA);

        gb.write_mem(0xFF70, 0x02);
        gb.write_mem(0xD000, 0xBB);

        // Stay on bank 2 and start OAM DMA from 0xD000
        gb.write_mem(0xFF70, 0x02);
        gb.write_mem(0xFF46, 0xD0);

        for _ in 0..650 {
            gb.advance_dots(1);
            gb.run_dma();
        }

        assert_eq!(
            gb.read_mem(0xFE00),
            0xBB,
            "OAM[0] should be 0xBB (WRAM bank 2 active during OAM DMA)"
        );
    }
}

#[test]
fn oam_dma_src_high_cgb_reads_ff() {
    let mut gb = setup_cgb();

    // Pre-fill OAM with a known non-FF value so we can detect a real copy
    for i in 0u8..160 {
        gb.write_mem(0xFE00 + u16::from(i), 0x42);
    }

    // Start OAM DMA from 0xFE00 (source >= 0xE000)
    gb.write_mem(0xFF46, 0xFE);

    for _ in 0..650 {
        gb.advance_dots(1);
        gb.run_dma();
    }

    // CGB: source ≥ 0xE000 → all bytes read as 0xFF
    for i in 0u8..160 {
        assert_eq!(
            gb.read_mem(0xFE00 + u16::from(i)),
            0xFF,
            "OAM[0x{:04X}] should be 0xFF (CGB OAM DMA from src>=0xE000)",
            0xFE00 + u16::from(i)
        );
    }
}

#[test]
fn samesuite_gbc_dma_cont() {
    let mut gb = setup_cgb();

    // Init source buffer in WRAM
    for i in 0u16..32 {
        gb.write_mem(0xC000 + i, i as u8);
    }

    // GDMA 1: copy 1 block (16 bytes) to VRAM 0x8000
    gb.write_mem(0xFF51, 0xC0);
    gb.write_mem(0xFF52, 0x00);
    gb.write_mem(0xFF53, 0x00);
    gb.write_mem(0xFF54, 0x00);
    gb.write_mem(0xFF55, 0x00); // 1 block GDMA
    gb.run_hdma();

    // Verify first block
    for i in 0u16..16 {
        assert_eq!(gb.read_mem(0x8000 + i), i as u8);
    }

    // Trigger another 1-block GDMA immediately by writing to HDMA5 again
    // It should continue from where the previous one left off (src=0xC010, dst=0x8010)
    gb.write_mem(0xFF55, 0x00);
    gb.run_hdma();

    // Verify second block
    for i in 0u16..16 {
        assert_eq!(gb.read_mem(0x8010 + i), (i + 16) as u8);
    }
}

#[test]
fn samesuite_hdma_mode0() {
    let mut gb = setup_cgb();
    gb.write_mem(0xFF40, 0x80); // LCD ON

    // Init source buffer in WRAM
    for i in 0u16..32 {
        gb.write_mem(0xC000 + i, i as u8);
    }

    // Program HDMA
    gb.write_mem(0xFF51, 0xC0);
    gb.write_mem(0xFF52, 0x00);
    gb.write_mem(0xFF53, 0x08); // dst = 0x8800
    gb.write_mem(0xFF54, 0x00);

    // Start 2-block HBlank DMA (bit 7 = 1, len = 1)
    gb.write_mem(0xFF55, 0x81);

    // Should be active, but nothing copied yet (not in HBlank)
    assert_eq!(gb.read_mem(0xFF55) & 0x80, 0);
    assert_eq!(gb.read_mem(0x8800), 0);

    // Advance to HBlank
    loop {
        gb.advance_dots(1);
        gb.run_hdma();
        if (gb.read_mem(0xFF41) & 0x03) == 0 {
            break;
        }
    }

    // After one HBlank, first block should be copied
    for i in 0u16..16 {
        assert_eq!(gb.read_mem(0x8800 + i), i as u8);
    }
    // Second block not yet
    assert_eq!(gb.read_mem(0x8810), 0);

    // HDMA5 should reflect 1 block remaining (bits 6:0 = 0)
    assert_eq!(gb.read_mem(0xFF55) & 0x7F, 0);

    // Advance to next HBlank
    loop {
        gb.advance_dots(1);
        gb.run_hdma();
        if (gb.read_mem(0xFF41) & 0x03) != 0 {
            break;
        } // Exit current HBlank
    }
    loop {
        gb.advance_dots(1);
        gb.run_hdma();
        if (gb.read_mem(0xFF41) & 0x03) == 0 {
            break;
        } // Enter next HBlank
    }

    // Now second block should be copied
    for i in 0u16..16 {
        assert_eq!(gb.read_mem(0x8810 + i), (i + 16) as u8);
    }

    // HDMA5 bit 7 should be set (finished)
    assert_eq!(gb.read_mem(0xFF55), 0xFF);
}

#[test]
fn test_repro_hdma_mode0_two_blocks_across_hblanks() {
    let mut gb = setup_cgb();
    gb.write_mem(0xFF40, 0x80); // LCD ON

    // Init source buffer in WRAM
    for i in 0u16..32 {
        gb.write_mem(0xC000 + i, i as u8);
    }

    // Program HDMA (dst = 0x8800)
    gb.write_mem(0xFF51, 0xC0);
    gb.write_mem(0xFF52, 0x00);
    gb.write_mem(0xFF53, 0x08);
    gb.write_mem(0xFF54, 0x00);

    // Start 2-block HBlank DMA (bit 7 = 1, len = 1 => 2 blocks)
    gb.write_mem(0xFF55, 0x81);

    // 1. Advance to FIRST HBlank
    let mut hblank_reached = false;
    for _ in 0..1000 {
        gb.advance_dots(1);
        gb.run_hdma();
        if (gb.read_mem(0xFF41) & 0x03) == 0 {
            hblank_reached = true;
            break;
        }
    }
    assert!(hblank_reached, "First HBlank never reached");

    // After one HBlank, ONLY the first block should be copied.
    for i in 0u16..16 {
        assert_eq!(
            gb.read_mem(0x8800 + i),
            i as u8,
            "HDMA failed to copy block 1 in first HBlank"
        );
    }
    assert_eq!(
        gb.read_mem(0x8810),
        0,
        "Second block should NOT be copied in the first HBlank"
    );

    // 2. Advance to SECOND HBlank
    // First, exit current HBlank
    for _ in 0..1000 {
        gb.advance_dots(1);
        gb.run_hdma();
        if (gb.read_mem(0xFF41) & 0x03) != 0 {
            break;
        }
    }
    // Then, enter next HBlank
    hblank_reached = false;
    for _ in 0..1000 {
        gb.advance_dots(1);
        gb.run_hdma();
        if (gb.read_mem(0xFF41) & 0x03) == 0 {
            hblank_reached = true;
            break;
        }
    }
    assert!(hblank_reached, "Second HBlank never reached");

    // After second HBlank, second block should be copied
    for i in 0u16..16 {
        assert_eq!(
            gb.read_mem(0x8810 + i),
            (i + 16) as u8,
            "HDMA failed to copy block 2 in second HBlank"
        );
    }
}

#[test]
fn samesuite_hdma_lcd_off() {
    let mut gb = setup_cgb();
    gb.write_mem(0xFF40, 0x00); // LCD OFF

    // Init source buffer in WRAM
    for i in 0u16..32 {
        gb.write_mem(0xC000 + i, i as u8);
    }

    // Program HDMA
    gb.write_mem(0xFF51, 0xC0);
    gb.write_mem(0xFF52, 0x00);
    gb.write_mem(0xFF53, 0x08); // dst = 0x8800
    gb.write_mem(0xFF54, 0x00);

    // Start HBlank DMA (bit 7 = 1, len = 1 -> 2 blocks)
    // While LCD is off, it should immediately copy ONE block and pause.
    gb.write_mem(0xFF55, 0x81);
    gb.run_hdma();

    // Verify first block is copied
    for i in 0u16..16 {
        assert_eq!(gb.read_mem(0x8800 + i), i as u8);
    }
    // Second block should NOT be copied (LCD is off, no HBlanks)
    assert_eq!(gb.read_mem(0x8810), 0);

    // HDMA5 should reflect 1 block remaining (bits 6:0 = 0) and bit 7=0 (active)
    let hdma5 = gb.read_mem(0xFF55);
    assert_eq!(
        hdma5 & 0x80,
        0x00,
        "HDMA should still be active (bit 7 = 0)"
    );
    assert_eq!(
        hdma5 & 0x7F,
        0x00,
        "HDMA should have 1 block remaining (bits 6:0 = 0)"
    );
}

#[test]
fn test_dma_oam_blocking_boundary() {
    let mut gb = setup_cgb();

    // 1. Initial state: OAM accessible
    gb.write_mem(0xFE00, 0x55);
    assert_eq!(gb.read_mem(0xFE00), 0x55);

    // 2. Start DMA
    gb.write_mem(0xFF46, 0xC0);

    // In Ceres, Dma::write() sets state to Starting(8).
    // Starting(8) means blocks_oam() is FALSE for 8 ticks (4 T-cycles).

    // Tick 1-4: OAM should still be accessible
    for t in 1..=4 {
        assert_eq!(
            gb.read_mem(0xFE00),
            0x55,
            "OAM should be accessible at tick {} after DMA trigger",
            t
        );
        gb.advance_dots(1);
        gb.run_dma();
    }

    // At tick 4, Starting(8) becomes Starting(4) after step().
    // Starting(4) STILL returns blocks_oam() == false.

    // Tick 5-8: OAM still accessible in Starting(4) stage
    for t in 5..=8 {
        assert_eq!(
            gb.read_mem(0xFE00),
            0x55,
            "OAM should still be accessible at tick {} (Starting(4) stage)",
            t
        );
        gb.advance_dots(1);
        gb.run_dma();
    }

    // Tick 9: OAM should definitely be blocked now (Transferring state)
    assert_eq!(
        gb.read_mem(0xFE00),
        0xFF,
        "OAM should be blocked at tick 9 after DMA trigger"
    );
}
