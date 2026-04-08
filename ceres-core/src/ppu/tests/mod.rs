mod blocking;
mod gambatte;
mod sprites;
mod stat_lyc;
mod timing;

use super::SpriteFetcherState;
use crate::ppu::color_palette;
use crate::test_util::setup_gb;
use crate::{CgbMode, Model};
type Gb = crate::Gb<crate::test_util::DummyAudio>;

enum BlarggOamBugAccessKind {
    Write,
    ReadWrite,
}

fn advance_to_ly(gb: &mut Gb, target_ly: u8) {
    for _ in 0..10_000_000 {
        if gb.ppu.read_ly() == target_ly {
            return;
        }
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }
    panic!("LY={target_ly} never reached");
}

fn advance_to_mode(gb: &mut Gb, target_mode: u8) {
    for _ in 0..10_000_000 {
        if gb.ppu.read_stat() & 0x03 == target_mode {
            return;
        }
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }
    panic!("Mode={target_mode} never reached");
}

fn advance_to_oam_scan_tick(
    gb: &mut Gb,
    target_ly: u8,
    target_tick: u16,
    cgb_mode: crate::CgbMode,
    double_speed: bool,
) {
    // First advance to the correct LY
    for _ in 0..10_000_000 {
        if gb.ppu.read_ly() == target_ly
            && matches!(
                gb.ppu.phase,
                crate::ppu::PpuPhase::OamScan(crate::ppu::OamScanStage::Running { tick })
                    if tick == target_tick
            )
        {
            return;
        }
        gb.ppu.tick(&mut gb.ints, cgb_mode, double_speed);
    }
    panic!(
        "OamScan Running {{ tick: {} }} on LY={} never reached",
        target_tick, target_ly
    );
}

fn mode3_duration_ticks(
    gb: &mut Gb,
    target_ly: u8,
    cgb_mode: crate::CgbMode,
    double_speed: bool,
) -> u32 {
    // Wait for mode-3 to start on target_ly
    for _ in 0..10_000_000 {
        if gb.ppu.read_ly() == target_ly && gb.ppu.read_stat() & 0x03 == 3 {
            break;
        }
        gb.ppu.tick(&mut gb.ints, cgb_mode, double_speed);
    }
    assert_eq!(
        gb.ppu.read_stat() & 0x03,
        3,
        "Mode-3 never started on LY={}",
        target_ly
    );
    let mut count: u32 = 0;
    for _ in 0..2000 {
        if gb.ppu.read_stat() & 0x03 != 3 {
            break;
        }
        gb.ppu.tick(&mut gb.ints, cgb_mode, double_speed);
        count += 1;
    }
    count
}

fn advance_to_ly_dot(gb: &mut Gb, target_ly: u8, target_dot: u16) {
    for _ in 0..10_000_000 {
        if gb.ppu.read_ly() == target_ly && gb.ppu.dots_in_line() == target_dot {
            return;
        }
        gb.advance_dots(1);
    }

    panic!(
        "LY={} dot={} never reached (current LY={}, dot={})",
        target_ly,
        target_dot,
        gb.ppu.read_ly(),
        gb.ppu.dots_in_line()
    );
}

fn fill_blargg_oam_bug_pattern(gb: &mut Gb) {
    for i in 0u16..0x00A0 {
        let val = (i as u8).wrapping_mul(3).wrapping_add(1);
        gb.ppu.write_oam_by_dma(0xFE00 + i, val);
    }
}

fn snapshot_oam(gb: &Gb) -> [u8; 0xA0] {
    let mut bytes = [0; 0xA0];
    bytes.copy_from_slice(gb.ppu.oam().bytes());
    bytes
}

fn row_base(row: usize) -> usize {
    row * 8
}

fn read_oam_word(bytes: &[u8; 0xA0], row: usize, word: usize) -> u16 {
    let base = row_base(row) + word * 2;
    u16::from_le_bytes([bytes[base], bytes[base + 1]])
}

fn write_oam_word(bytes: &mut [u8; 0xA0], row: usize, word: usize, val: u16) {
    let base = row_base(row) + word * 2;
    let [lo, hi] = val.to_le_bytes();
    bytes[base] = lo;
    bytes[base + 1] = hi;
}

fn apply_blargg_oam_bug_write_corruption(bytes: &mut [u8; 0xA0], row: usize) {
    if row == 0 {
        return;
    }

    let a = read_oam_word(bytes, row, 0);
    let b = read_oam_word(bytes, row - 1, 0);
    let c = read_oam_word(bytes, row - 1, 2);
    write_oam_word(bytes, row, 0, ((a ^ c) & (b ^ c)) ^ c);

    for word in 1..4 {
        let prev = read_oam_word(bytes, row - 1, word);
        write_oam_word(bytes, row, word, prev);
    }
}

fn apply_blargg_oam_bug_read_corruption(bytes: &mut [u8; 0xA0], row: usize) {
    if row == 0 {
        return;
    }

    let a = read_oam_word(bytes, row, 0);
    let b = read_oam_word(bytes, row - 1, 0);
    let c = read_oam_word(bytes, row - 1, 2);
    write_oam_word(bytes, row, 0, b | (a & c));

    for word in 1..4 {
        let prev = read_oam_word(bytes, row - 1, word);
        write_oam_word(bytes, row, word, prev);
    }
}

fn apply_blargg_oam_bug_read_write_corruption(bytes: &mut [u8; 0xA0], row: usize) {
    if (4..19).contains(&row) {
        let a = read_oam_word(bytes, row - 2, 0);
        let b = read_oam_word(bytes, row - 1, 0);
        let c = read_oam_word(bytes, row, 0);
        let d = read_oam_word(bytes, row - 1, 2);
        let corrupt_prev = (b & (a | c | d)) | (a & c & d);
        write_oam_word(bytes, row - 1, 0, corrupt_prev);

        let prev_row_words = [
            read_oam_word(bytes, row - 1, 0),
            read_oam_word(bytes, row - 1, 1),
            read_oam_word(bytes, row - 1, 2),
            read_oam_word(bytes, row - 1, 3),
        ];

        for (word, val) in prev_row_words.into_iter().enumerate() {
            write_oam_word(bytes, row, word, val);
            write_oam_word(bytes, row - 2, word, val);
        }
    }

    apply_blargg_oam_bug_read_corruption(bytes, row);
}

fn expected_oam_after_blargg_oam_bug_access(
    initial: [u8; 0xA0],
    row: usize,
    access: BlarggOamBugAccessKind,
) -> [u8; 0xA0] {
    let mut expected = initial;
    match access {
        BlarggOamBugAccessKind::Write => {
            apply_blargg_oam_bug_write_corruption(&mut expected, row);
        }
        BlarggOamBugAccessKind::ReadWrite => {
            apply_blargg_oam_bug_read_write_corruption(&mut expected, row);
        }
    }
    expected
}

fn assert_oam_changed(before: &[u8; 0xA0], after: &[u8; 0xA0], context: &str) {
    assert_ne!(before, after, "Expected OAM corruption: {context}");
}

fn assert_oam_unchanged(before: &[u8; 0xA0], after: &[u8; 0xA0], context: &str) {
    assert_eq!(before, after, "Expected OAM to remain unchanged: {context}");
}

fn run_opcode(gb: &mut Gb, bytes: &[u8]) {
    let pc = 0xC000;
    gb.set_cpu_pc(pc);
    for (i, &byte) in bytes.iter().enumerate() {
        gb.write_mem(pc + i as u16, byte);
    }
    gb.run_cpu();
}

fn setup_blargg_oam_bug_mid_window() -> Gb {
    let mut gb = setup_gb();
    fill_blargg_oam_bug_pattern(&mut gb);
    gb.write_mem(0xFF40, 0x80);
    advance_to_ly_dot(&mut gb, 2, 16);
    gb
}

fn oam_access_setup() -> Gb {
    let mut gb = setup_gb();
    gb.write_mem(0xFF40, 0x80); // LCD ON

    // Wait for LY=90
    while gb.ppu.read_ly() != 90 {
        gb.advance_dots(1);
    }
    // Wait for Mode 3
    advance_to_mode(&mut gb, 3);

    // Enable Mode 2 STAT interrupt
    gb.write_mem(0xFF41, 0x20);
    gb.write_mem(0xFFFF, 0x02); // IE: STAT
    gb.ints.write_if(0);

    // Pre-set OAM[0] = 0
    gb.write_mem(0xFE00, 0x00);

    // Wait for Mode 2 IRQ to fire (next line)
    while (gb.ints.read_if() & 0x02) == 0 {
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }

    gb
}

fn setup_sprites_n(n: usize) -> Gb {
    let mut gb = setup_gb();
    gb.write_mem(0xFF40, 0x80); // LCD ON, sprites ON

    // Clear OAM
    for i in 0..40 {
        let base = 0xFE00 + (i as u16) * 4;
        gb.write_mem(base, 0); // Y = 0 (off-screen)
        gb.write_mem(base + 1, 0);
        gb.write_mem(base + 2, 0);
        gb.write_mem(base + 3, 0);
    }

    // Place n sprites on line 10
    for i in 0..n.min(10) {
        let base = 0xFE00 + (i as u16) * 4;
        gb.write_mem(base, 16); // Y = 16 → visible on LY=10 (16-16=0)
        gb.write_mem(base + 1, (8 + i * 8) as u8); // X positions
        gb.write_mem(base + 2, 0x10); // Tile index
        gb.write_mem(base + 3, 0); // Flags
    }

    advance_to_ly(&mut gb, 10);
    advance_to_mode(&mut gb, 3);
    advance_to_mode(&mut gb, 0); // Finish line 10
    advance_to_mode(&mut gb, 2); // Start line 11 OAM scan
    gb
}
