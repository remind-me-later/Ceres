//! Where ceres's DMG sets the CPU's VRAM and OAM locks on a visible line,
//! proved from its own code (`cargo kani -p ceres-core --harness
//! cpu_locks`).
//!
//! The gate tests (`ceres-test-runner/tests/vram_gate.rs` and
//! `oam_gate.rs`) hold ceres's locks to the netlist's rules at the positions
//! ceres's trace measured. On a visible line but line 0 after the LCD is
//! turned on, in half dots of the timing unit's clock:
//!
//! | h                  | VRAM       | OAM                   |
//! | ------------------ | ---------- | --------------------- |
//! | 903 (line before)  |            | read lock on          |
//! | 905 (line before)  |            | write lock on too     |
//! | 145                | read lock on | write lock off       |
//! | 153                | write lock on | write lock on        |
//! | mode 3's end - 2   | both off   | both off              |
//!
//! and nothing else changes; after line 143, VBlank has no scan. Counted in
//! units (half dots) as the trace counts them (the unit that changed a lock,
//! the unit that runs mode 3's loop's first dot, the unit that ends mode 3
//! for the timing unit):
//!
//! - `cpu_locks_at_scan_end` (Kani): from the scan's step for sprite 36,
//!   with any number of objects found, the locks change twice: 4 units on
//!   (the step for sprite 37: VRAM's read lock on, OAM's write lock off)
//!   and 12 on (the scan's end: all on); mode 3's loop starts 22 on, so 18
//!   and 10 units after the changes.
//! - `cpu_locks_at_mode3_end` (Kani): as mode 3's loop ends (`mode3_done`),
//!   all four fall, and `HBlankStart` ends mode 3 for the timing unit
//!   (`timing_mode3_end`) 2 units later, with nothing locked in between.
//! - `tests::lines_lock_as_measured` (not Kani: a line of the whole display
//!   with symbolic registers does not fit in 9 GB): from a visible line's
//!   start (`NextLine`, or `FrameEnd` for line 0) to the next line's, mode 3
//!   included, over every line and LCDC with the LCD on and OAMs of each
//!   kind, the locks change at units 6 and 8 (OAM's), 160 and 168 (the two
//!   above), as mode 3 ends, and nowhere else; mode 3's loop starts at 178.
//!
//! With the loop's first dot at h = 163 (`mapping-mode3-start`), unit 0 is
//! h = 897 of the line before, so units 6, 8, 160 and 168 are h = 903, 905,
//! 145 and 153; mode 3's end is where the timing unit has it
//! (`mapping-mode3-end`). The line is not cut off (the 456-dot abort,
//! `Mode3Abort`, which skips `mode3_done` and the next line's scan, so that
//! the locks stay on): `dmg-lines-not-cut`.

#![expect(
    clippy::needless_bitwise_bool,
    reason = "relations for Kani: a lazy && or || is a branch for the solver"
)]

use {
    super::state::State,
    crate::{Model, interrupts::Interrupts, ppu::Ppu},
};

/// The CPU's locks: VRAM's read and write, OAM's read and write.
#[expect(
    clippy::struct_excessive_bools,
    reason = "one lock per memory and direction"
)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Locks {
    vram_read: bool,
    vram_write: bool,
    oam_read: bool,
    oam_write: bool,
}

impl Locks {
    const fn of(ppu: &Ppu) -> Self {
        let cpu = &ppu.core.d.cpu;
        Self {
            vram_read: cpu.vram_read_blocked,
            vram_write: cpu.vram_write_blocked,
            oam_read: cpu.oam_read_blocked,
            oam_write: cpu.oam_write_blocked,
        }
    }

    #[cfg(kani)]
    const fn same(self, other: Self) -> bool {
        (self.vram_read == other.vram_read)
            & (self.vram_write == other.vram_write)
            & (self.oam_read == other.oam_read)
            & (self.oam_write == other.oam_write)
    }
}

/// The scan's locks before sprite 37's step: OAM's, not VRAM's.
const SCANNING: Locks = Locks {
    vram_read: false,
    vram_write: false,
    oam_read: true,
    oam_write: true,
};
/// From sprite 37's step: VRAM's read lock, OAM's read lock alone.
const SCAN_TAIL: Locks = Locks {
    vram_read: true,
    vram_write: false,
    oam_read: true,
    oam_write: false,
};
const ALL: Locks = Locks {
    vram_read: true,
    vram_write: true,
    oam_read: true,
    oam_write: true,
};
const NONE: Locks = Locks {
    vram_read: false,
    vram_write: false,
    oam_read: false,
    oam_write: false,
};

/// The most dots a DMG line counts by the end of mode 3: 89 to the loop,
/// 160 pixels, SCX's 7, the window's 6 and ten objects' 11 each.
#[cfg(kani)]
const LONGEST_LINE: i32 = 89 + 160 + 7 + 6 + 110;

/// A DMG with the LCD on and `line` visible, its locks open.
fn dmg_on_line(line: u8, lcdc: u8) -> Ppu {
    let mut ppu = Ppu::new(Model::DmgB);
    ppu.lcdc = lcdc | 0x80;
    ppu.d.current_line = line;
    ppu.ly = line;
    ppu.d.line_clock_valid = true;
    ppu
}

#[cfg(kani)]
const fn set(ppu: &mut Ppu, locks: Locks) {
    let cpu = &mut ppu.core.d.cpu;
    cpu.vram_read_blocked = locks.vram_read;
    cpu.vram_write_blocked = locks.vram_write;
    cpu.oam_read_blocked = locks.oam_read;
    cpu.oam_write_blocked = locks.oam_write;
}

/// Mode 3's loop runs its first dot in the coming unit.
const fn loop_starts(ppu: &Ppu) -> bool {
    matches!(ppu.pace.state(), State::Mode3Start) & (ppu.pace.wait == 1) & !ppu.core.d.half_dot
}

/// From the unit in which the scan's step for sprite 36 ran: the locks
/// change 4 units on and 12 on, to the scan's tail and to all, and the
/// loop starts 22 on.
// @assumes dmg-lines-not-cut
// @assumes scan-match-only-in-the-store
#[cfg(kani)]
#[kani::proof]
#[kani::unwind(25)]
fn cpu_locks_at_scan_end() {
    // What the scan matches reaches only the object store (`ObjectSearch::
    // consider` takes the store alone: scan-match-only-in-the-store), and a
    // symbolic match costs Kani 6 GB in the store's insertion, so the
    // sprites left (37 to 39, zeroed: tall objects, on) match nothing here,
    // on line 3; the test below runs every line and LCDC. How many objects
    // the scan found before them is any, the store full included.
    let mut ppu = dmg_on_line(3, 0x06);
    let found: usize = kani::any();
    kani::assume(found <= 10);
    ppu.pace.objs.count = found;
    set(&mut ppu, SCANNING);
    // Sprite 36's step ran in the unit before, the first of its dot, and
    // went on to sprite 37's: OamScanNext sleeps two dots.
    ppu.pace.objs.index = 37;
    ppu.pace.sleep(State::OamScanObject, 2);
    ppu.d.half_dot = true;
    let mut ints = Interrupts::default();

    // Unit 0 here is the step's unit + 1.
    let mut locks = SCANNING;
    let mut changes = 0;
    let mut loop_start = None;
    for unit in 0..24_u32 {
        if loop_starts(&ppu) {
            loop_start = Some(unit);
            break;
        }
        ppu.run_display(&mut ints);
        let now = Locks::of(&ppu);
        if !now.same(locks) {
            changes += 1;
            match unit {
                3 => assert!(now.same(SCAN_TAIL), "sprite 37's step"),
                11 => assert!(now.same(ALL), "the scan's end"),
                _ => panic!("a lock changes elsewhere"),
            }
            locks = now;
        }
    }
    assert!(changes == 2, "the locks do not change twice");
    assert!(
        loop_start == Some(21),
        "the loop does not start 22 units on"
    );
}

/// As mode 3's loop ends: all the locks fall, and the timing unit's mode 3
/// ends 2 units later.
// @assumes dmg-lines-not-cut
// @assumes locks-with-mode-0-pulse
#[cfg(kani)]
#[kani::proof]
#[kani::unwind(6)]
fn cpu_locks_at_mode3_end() {
    let line: u8 = kani::any();
    kani::assume(line < super::LINES);
    let mut ppu = dmg_on_line(line, kani::any::<u8>() & 0x06);
    set(&mut ppu, ALL);
    // The loop drew the line. The line's length so far (`cycles_for_line`)
    // is read in these units only by the abort's comparison: here the
    // longest a DMG line gets (dmg-lines-not-cut), which a shorter one
    // passes too. Left symbolic, it costs Kani 6 GB.
    ppu.d.lcd_x = crate::PX_WIDTH;
    ppu.pace.cfl = LONGEST_LINE;
    // The unit in which the loop ended, the first of its dot.
    ppu.d.half_dot = true;
    ppu.mode3_done();
    assert!(Locks::of(&ppu).same(NONE), "the locks do not fall");

    let mut ints = Interrupts::default();
    let mut end = None;
    for unit in 1..5_u32 {
        ppu.run_display(&mut ints);
        assert!(Locks::of(&ppu).same(NONE), "a lock comes on");
        if ppu.timing_mode3_ending() {
            end = Some(unit);
            break;
        }
    }
    assert!(end == Some(2), "mode 3 does not end 2 units after");
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        alloc::{format, vec::Vec},
    };

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

    /// The locks that come in at each unit of a visible line's run.
    const OAM_READ: Locks = Locks {
        vram_read: false,
        vram_write: false,
        oam_read: true,
        oam_write: false,
    };

    #[test]
    #[expect(clippy::print_stderr, reason = "the test reports what it found")]
    fn lines_lock_as_measured() {
        let (mut lines, mut pulsed) = (0_u32, 0_u32);
        for line in 0..super::super::LINES {
            for lcdc in 0x80..=0xFF_u8 {
                for seed in 0..4_u32 {
                    let mut ppu = dmg_on_line(line, lcdc);
                    match seed {
                        // Every sprite on the line, in a row from X 8.
                        0 => {
                            for (i, byte) in ppu.oam_mut().bytes_mut().iter_mut().enumerate() {
                                *byte = match i % 4 {
                                    0 => line.wrapping_add(16),
                                    #[expect(
                                        clippy::cast_possible_truncation,
                                        reason = "40 sprites"
                                    )]
                                    1 => 8 + (i / 4) as u8 * 4,
                                    _ => 0,
                                };
                            }
                        }
                        1 => {}
                        _ => bytes(seed * 7919 + u32::from(line), ppu.oam_mut().bytes_mut()),
                    }
                    ppu.scx = lcdc;
                    ppu.wy = line;
                    ppu.wx = lcdc.wrapping_mul(3);
                    if line == 0 {
                        ppu.d.current_line = 0;
                        ppu.pace.sleep(State::FrameEnd, 1);
                    } else {
                        ppu.d.current_line = line - 1;
                        ppu.pace.sleep(State::NextLine, 1);
                    }
                    let mut ints = Interrupts::default();

                    let mut changes = Vec::new();
                    let (mut loop_start, mut end, mut first_end) = (None, None, None);
                    let mut locks = Locks::of(&ppu);
                    let mut next_line = None;
                    for unit in 0..1000_u32 {
                        if loop_starts(&ppu) && loop_start.is_none() {
                            loop_start = Some(unit);
                        }
                        let state = ppu.pace.state();
                        let starting = matches!(state, State::NextLine) && ppu.pace.wait == 1;
                        if unit > 0 && starting && !ppu.d.half_dot {
                            next_line = Some(unit);
                            break;
                        }
                        if unit > 0 && matches!(state, State::VBlankLine) {
                            next_line = Some(unit);
                            break;
                        }
                        ppu.run_display(&mut ints);
                        // Mode 3's end: the unit in which HBlankStart runs
                        // (`timing_mode3_end`); the timing unit's first sight
                        // of an end, which a sprite at X 167 or the WX 166
                        // glitch brings earlier (a mode 0 pulse).
                        if matches!(state, State::HBlankStart)
                            && !matches!(ppu.pace.state(), State::HBlankStart)
                        {
                            end = Some(unit);
                        }
                        if ppu.timing_mode3_ending() && first_end.is_none() {
                            first_end = Some(unit);
                        }
                        let now = Locks::of(&ppu);
                        if now != locks {
                            changes.push((unit, now));
                            locks = now;
                        }
                    }
                    let what = format!("line {line}, LCDC {lcdc:02X}, OAM {seed}");
                    assert!(
                        end.is_some() && next_line.is_some(),
                        "{what}: the line does not end"
                    );
                    let end = end.unwrap_or_default();
                    assert_eq!(loop_start, Some(178), "{what}");
                    assert_eq!(
                        changes,
                        [
                            (6, OAM_READ),
                            (8, SCANNING),
                            (160, SCAN_TAIL),
                            (168, ALL),
                            (end - 2, NONE),
                        ],
                        "{what}"
                    );
                    if first_end != Some(end) {
                        pulsed += 1;
                    }
                    lines += 1;
                }
            }
        }
        assert_eq!(lines, 144 * 128 * 4);
        // The lines whose timing unit saw a mode 0 pulse before the end:
        // their locks are as on the others, from the end
        // (`locks-with-mode-0-pulse`).
        std::eprintln!("{lines} lines, {pulsed} with an early mode 0 pulse");
        assert!(pulsed > 0, "no line with a mode 0 pulse");
    }
}
