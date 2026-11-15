//! Where ceres's DMG sets the CPU's VRAM locks on a visible line, proved
//! from its own code (`cargo kani -p ceres-core --harness vram_locks`).
//!
//! The VRAM gate test (`ceres-test-runner/tests/vram_gate.rs`) holds ceres's
//! locks to the netlist's rule at the positions its trace measured: the
//! read lock 9 half dots before the netlist's mode 3 rises, the write lock
//! 1 before, both off 2 before ceres's mode 3 ends. Counted in units (half
//! dots), as the trace counts them (the unit that changed a lock, the unit
//! that runs the loop's first dot, the unit that ends mode 3):
//!
//! - `vram_locks_at_scan_end`: from the scan's step for sprite 36, with
//!   any number of objects found (what the scan matches reaches only the
//!   object store: `scan-match-only-in-the-store`), `run_display`
//!   puts the read lock on 4 units later (the step for 37), the write lock
//!   on 12 later (the scan's end) and starts mode 3's loop 22 later, so 18
//!   and 10 units ahead of it; neither lock falls in between.
//! - `vram_locks_at_mode3_end`: as mode 3's loop ends (`mode3_done`), both
//!   locks fall, and `HBlankStart` ends mode 3 for the timing unit
//!   (`timing_mode3_end`) 2 units later, with nothing locked in between.
//! - `tests::scan_start_leaves_vram_open` (not Kani: 180 units of the whole
//!   display do not fit in 9 GB): from a visible line's start (`NextLine`,
//!   or `FrameEnd` for line 0), over every line and LCDC with the LCD on
//!   and OAMs of each kind, VRAM stays open until the step for sprite 37,
//!   which comes 160 units after the line's start; the loop starts at 178.
//!   The scan's states branch on nothing but the model and the index for
//!   their sleeps and locks.
//!
//! With the loop's first dot at h = 163 and the netlist's rise at 154
//! (`mapping-mode3-start`), the read lock comes on at 145 and the write
//! lock at 153: 9 and 1 before the rise. The line is not cut off (the
//! 456-dot abort, `Mode3Abort`, which skips `mode3_done` and the next
//! line's scan, so that its locks stay on): `dmg-lines-not-cut`.

use {
    super::state::State,
    crate::{Model, interrupts::Interrupts, ppu::Ppu},
};

/// The most dots a DMG line counts by the end of mode 3: 89 to the loop,
/// 160 pixels, SCX's 7, the window's 6 and ten objects' 11 each.
#[cfg(kani)]
const LONGEST_LINE: i32 = 89 + 160 + 7 + 6 + 110;

/// A DMG with the LCD on and `line` visible, its locks as the scan has
/// them (OAM locked, VRAM open).
fn dmg_in_scan(line: u8, lcdc: u8) -> Ppu {
    let mut ppu = Ppu::new(Model::DmgB);
    ppu.lcdc = lcdc | 0x80;
    ppu.d.current_line = line;
    ppu.d.line_clock_valid = true;
    ppu
}

/// Runs units until mode 3's loop would run its first dot (left unrun:
/// what it does is the mode 3 proofs'), recording the units at which the
/// VRAM locks came on. Returns (read on, write on, loop start).
#[cfg(kani)]
fn run_to_mode3(
    ppu: &mut Ppu,
    ints: &mut Interrupts,
    units: u32,
) -> (Option<u32>, Option<u32>, Option<u32>) {
    let (mut read_on, mut write_on) = (None, None);
    for unit in 0..units {
        if ppu.pace.state() == State::Mode3Start && ppu.pace.wait == 1 && !ppu.d.half_dot {
            return (read_on, write_on, Some(unit));
        }
        ppu.run_display(ints);
        let (read, write) = (ppu.d.cpu.vram_read_blocked, ppu.d.cpu.vram_write_blocked);
        if read_on.is_none() & read {
            read_on = Some(unit);
        }
        if write_on.is_none() & write {
            write_on = Some(unit);
        }
        assert!(
            read | read_on.is_none(),
            "the read lock falls before mode 3"
        );
        assert!(
            write | write_on.is_none(),
            "the write lock falls before mode 3"
        );
    }
    (read_on, write_on, None)
}

/// From the unit in which the scan's step for sprite 36 ran: the read lock
/// 4 units on, the write lock 12, the loop 22.
// @assumes scan-match-only-in-the-store
// @assumes dmg-lines-not-cut
#[cfg(kani)]
#[kani::proof]
#[kani::unwind(25)]
fn vram_locks_at_scan_end() {
    // What the scan matches reaches only the object store (`ObjectSearch::
    // consider` takes the store alone: scan-match-only-in-the-store), and a
    // symbolic match costs Kani 6 GB in the store's insertion, so the
    // sprites left (37 to 39, zeroed: tall objects, on) match nothing here,
    // on line 3;
    // the test below runs every line and LCDC. How many objects the scan
    // found before them is any, the store full included.
    let mut ppu = dmg_in_scan(3, 0x06);
    ppu.ly = 3;
    let found: usize = kani::any();
    kani::assume(found <= 10);
    ppu.pace.objs.count = found;
    ppu.d.cpu.oam_read_blocked = true;
    ppu.d.cpu.oam_write_blocked = true;
    // Sprite 36's step ran in this unit, the first of its dot, and went on
    // to sprite 37's: OamScanNext sleeps two dots.
    ppu.pace.objs.index = 37;
    ppu.pace.sleep(State::OamScanObject, 2);
    ppu.d.half_dot = true;
    let mut ints = Interrupts::default();

    let (read_on, write_on, loop_start) = run_to_mode3(&mut ppu, &mut ints, 24);
    // The run starts a unit after the step's: unit 0 here is unit 1 there.
    assert!(read_on == Some(3), "the read lock is not 4 units after");
    assert!(write_on == Some(11), "the write lock is not 12 units after");
    assert!(
        loop_start == Some(21),
        "the loop does not start 22 units after"
    );
}

/// As mode 3's loop ends: both locks fall, and the timing unit's mode 3
/// ends 2 units later.
// @assumes dmg-lines-not-cut
#[cfg(kani)]
#[kani::proof]
#[kani::unwind(6)]
fn vram_locks_at_mode3_end() {
    let line: u8 = kani::any();
    kani::assume(line < super::LINES);
    let mut ppu = dmg_in_scan(line, kani::any::<u8>() & 0x06);
    ppu.ly = line;
    ppu.d.cpu.vram_read_blocked = true;
    ppu.d.cpu.vram_write_blocked = true;
    ppu.d.cpu.oam_read_blocked = true;
    ppu.d.cpu.oam_write_blocked = true;
    // The loop drew the line. The line's length so far (`cycles_for_line`)
    // is read in these units only by the abort's comparison: here the
    // longest a DMG line gets (dmg-lines-not-cut), which a shorter one
    // passes too. Left symbolic, it costs Kani 6 GB.
    ppu.d.lcd_x = crate::PX_WIDTH;
    ppu.pace.cfl = LONGEST_LINE;
    // The unit in which the loop ended, the first of its dot.
    ppu.d.half_dot = true;
    ppu.mode3_done();
    assert!(!ppu.d.cpu.vram_read_blocked & !ppu.d.cpu.vram_write_blocked);

    let mut ints = Interrupts::default();
    let mut end = None;
    for unit in 1..5_u32 {
        ppu.run_display(&mut ints);
        assert!(!ppu.d.cpu.vram_read_blocked & !ppu.d.cpu.vram_write_blocked);
        if ppu.timing_mode3_ending() {
            end = Some(unit);
            break;
        }
    }
    assert!(
        end == Some(2),
        "mode 3 does not end 2 units after the unlock"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small xorshift, for OAMs of each kind.
    #[expect(clippy::cast_possible_truncation, reason = "the low byte")]
    fn bytes(seed: u32, out: &mut [u8]) {
        let mut x = seed | 1;
        for byte in out {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            *byte = x as u8;
        }
    }

    #[test]
    fn scan_start_leaves_vram_open() {
        for line in 0..super::super::LINES {
            for lcdc in 0x80..=0xFF_u8 {
                for seed in [0, 1, 2, 3] {
                    let mut ppu = dmg_in_scan(line, lcdc);
                    match seed {
                        // Every sprite on the line.
                        0 => {
                            for (i, byte) in ppu.oam_mut().bytes_mut().iter_mut().enumerate() {
                                *byte = if i % 4 == 0 { line.wrapping_add(16) } else { 8 };
                            }
                        }
                        1 => {}
                        _ => bytes(seed * 7919 + u32::from(line), ppu.oam_mut().bytes_mut()),
                    }
                    ppu.scx = lcdc;
                    ppu.wy = line;
                    ppu.wx = lcdc.wrapping_mul(3);
                    if line == 0 {
                        ppu.pace.sleep(State::FrameEnd, 1);
                    } else {
                        ppu.d.current_line = line - 1;
                        ppu.pace.sleep(State::NextLine, 1);
                    }
                    let mut ints = Interrupts::default();
                    let (mut at_37, mut read_on, mut loop_start) = (None, None, None);
                    for unit in 0..200 {
                        if ppu.pace.state() == State::Mode3Start
                            && ppu.pace.wait == 1
                            && !ppu.d.half_dot
                        {
                            loop_start = Some(unit);
                            break;
                        }
                        let before = ppu.pace.objs.index;
                        ppu.run_display(&mut ints);
                        if before == 37 && ppu.pace.objs.index == 38 {
                            at_37 = Some(unit);
                        }
                        if read_on.is_none() && ppu.d.cpu.vram_read_blocked {
                            read_on = Some(unit);
                        }
                        if at_37.is_none() {
                            assert!(
                                !ppu.d.cpu.vram_read_blocked && !ppu.d.cpu.vram_write_blocked,
                                "line {line}, LCDC {lcdc:02X}: VRAM locked at unit {unit}"
                            );
                        }
                    }
                    assert_eq!(at_37, Some(160), "line {line}, LCDC {lcdc:02X}");
                    assert_eq!(read_on, Some(160), "line {line}, LCDC {lcdc:02X}");
                    assert_eq!(loop_start, Some(178), "line {line}, LCDC {lcdc:02X}");
                }
            }
        }
    }
}
