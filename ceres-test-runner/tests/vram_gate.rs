//! What decides whether a CPU access to VRAM gets through, on ceres, held
//! to the netlist's rule (`ceres-netlist`'s `tests/vram_gate.rs`): a write
//! lands if mode 3 is off as E, F or G ends, a read takes VRAM's byte if it
//! is off as H ends.
//!
//! The same ROMs (Gambatte's `vram_m3` for the DMG, gbmicrotest's `vram`
//! ROMs) run from dmg-sim's quick boot ROM, and ceres's PPU trace gives each
//! CPU access to VRAM, with its half dot `h` on the timing unit's clock and
//! whether ceres's lock was on, and per line where mode 3 ended for the
//! timing unit. The netlist's mode 3 (render's XYMU, which its tally
//! samples) is put on that clock by the mappings:
//!
//! - it rises at h = 154 (`mapping-mode3-start`);
//! - it falls two half dots after ceres's mode 3 ends: render's mode 0
//!   rises a half dot after (`mapping-mode3-end`) and clears XYMU at the
//!   next edge, through VOGA;
//! - a CPU access falls at h = 0 (mod 8), the D edge
//!   (`mapping-access-at-h0`, `mapping-half-dot`), so its M-cycle's phases
//!   A to H run from h - 3 to h + 4;
//! - in line 0 after the LCD is turned on, accesses fall at h = 2 (still
//!   the D edge) until ceres's clock is resynced at the line's end
//!   (`mapping-access-at-h0`), and mode 3 rises at h = 160: ceres's mode 3
//!   loop starts six half dots late there (`mapping-mode3-start`), and the
//!   netlist's tally has mode 3 rise at the B edge of an access at h = 162.
//!   It falls as on the other lines: the reads around the fall have the
//!   same patterns in both tallies, ROM by ROM.
//!
//! With the LCD off, and in VBlank, mode 3 is off throughout.
//!
//! The ROMs' accesses reach a few of an M-cycle's alignments with mode 3's
//! edges. The trace also gives where ceres's locks change on each visible
//! line, from mode 3's rise and end; at those positions ceres's locks are
//! held to the rule for every end of mode 3 (with the parity seen) and
//! every access position on the line.
//!
//! `cargo test -p ceres-test-runner --release --test vram_gate -- --ignored
//! --nocapture`

#![expect(
    clippy::print_stderr,
    reason = "a test, which reports what it compared"
)]

use {
    ceres_core::{GbBuilder, Model, PpuEventKind},
    ceres_test_runner::DummyAudioCallback,
    std::{collections::BTreeMap, fs, path::Path},
};

/// The run of each ROM: 0.29 s in 8 MiHz units, as the netlist's tally.
const UNITS: u64 = 2_437_000;
/// LCDC's bit that turns the LCD on.
const LCDC_ON: u8 = 0x80;
/// The first line of VBlank.
const VBLANK_LINE: u8 = 144;
/// The phases of an M-cycle, and E, G and H among them (A is 0).
const PHASES: i32 = 8;
const PHASE_E: usize = 4;
const PHASE_G: usize = 6;
const PHASE_H: usize = 7;
/// Phase A of an access's M-cycle, from the access's half dot.
const A_FROM_ACCESS: i32 = -3;

/// Where the pixel pipeline's line end puts ceres's clock (`RESYNC_H`):
/// from there on, accesses in line 0 after the LCD was turned on fall at
/// h = 0 (mod 8) too.
const RESYNC_H: i32 = 893;

/// Where the netlist's mode 3 falls, from where ceres's ended.
const FALL_FROM_END: i32 = 2;

/// Where the netlist's mode 3 rises on a line, on ceres's clock, and where
/// accesses fall: a line, and line 0 after the LCD is turned on.
struct Mapping {
    /// Where mode 3 rises.
    rise: i32,
    /// The half dot of an access, mod 8.
    access: i32,
}

const LINE: Mapping = Mapping {
    rise: 154,
    access: 0,
};
const FIRST_LINE: Mapping = Mapping {
    rise: 160,
    access: 2,
};

/// A CPU access to VRAM: its half dot, a write, ceres's lock on.
#[derive(Clone, Copy)]
struct Access {
    h: i32,
    write: bool,
    locked: bool,
}

/// A line's accesses, where ceres's mode 3 ended in it, and where its
/// read and write locks came on and both came off.
#[derive(Default)]
struct Line {
    number: u8,
    /// Line 0 after the LCD was turned on.
    first: bool,
    end: Option<i32>,
    read_on: Option<i32>,
    write_on: Option<i32>,
    off: Option<i32>,
    accesses: Vec<Access>,
}

/// Where ceres's locks came on on a line, from mode 3's rise, and came off,
/// from its end; and whether the end was on an odd half dot.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
struct Locks {
    first: bool,
    read_on: i32,
    write_on: i32,
    off: i32,
    odd_end: bool,
}

/// Per kind of access (a write) and mode 3's pattern, A to H, ceres's
/// outcomes (locked) and their counts.
type Tally = BTreeMap<(bool, String), BTreeMap<bool, u64>>;

/// What the runs told.
#[derive(Default)]
struct Found {
    tally: Tally,
    /// Accesses with the LCD off, in VBlank, and in line 0 after the LCD
    /// was turned on.
    lcd_off: u64,
    vblank: u64,
    first_line: u64,
    /// Accesses not on the D edge the mappings give.
    misaligned: u64,
    /// Accesses on a visible line where ceres's mode 3 did not end (the LCD
    /// was turned off) that mode 3 may have reached.
    unplaced: u64,
    /// The visible lines' locks, and how many lines had them.
    locks: BTreeMap<Locks, u64>,
}

impl Found {
    fn count(&mut self, access: Access, levels: String) {
        *self
            .tally
            .entry((access.write, levels))
            .or_default()
            .entry(access.locked)
            .or_default() += 1;
    }
}

/// Mode 3's pattern, A to H, for an access at `h` where it is on from
/// `rise` up to `fall`.
fn pattern(h: i32, rise: i32, fall: i32) -> String {
    (0..PHASES)
        .map(|phase| {
            let at = h + A_FROM_ACCESS + phase;
            if (rise..fall).contains(&at) { '1' } else { '0' }
        })
        .collect()
}

/// A line, done: its accesses tallied.
fn finish(line: &Line, found: &mut Found) {
    let mapping = if line.first { &FIRST_LINE } else { &LINE };
    if let (Some(end), Some(read_on), Some(write_on), Some(off)) =
        (line.end, line.read_on, line.write_on, line.off)
    {
        *found
            .locks
            .entry(Locks {
                first: line.first,
                read_on: read_on - mapping.rise,
                write_on: write_on - mapping.rise,
                off: off - end,
                odd_end: end & 1 == 1,
            })
            .or_default() += 1;
    }
    for &access in &line.accesses {
        let aligned = if access.h < RESYNC_H {
            mapping.access
        } else {
            LINE.access
        };
        if access.h.rem_euclid(PHASES) != aligned {
            found.misaligned += 1;
            continue;
        }
        if line.first {
            found.first_line += 1;
        }
        let fall = if line.number >= VBLANK_LINE {
            found.vblank += 1;
            mapping.rise
        } else if let Some(end) = line.end {
            end + FALL_FROM_END
        } else if access.h + A_FROM_ACCESS + PHASES <= mapping.rise {
            mapping.rise
        } else {
            found.unplaced += 1;
            continue;
        };
        found.count(access, pattern(access.h, mapping.rise, fall));
    }
}

/// A ROM's run, tallied.
#[expect(clippy::expect_used, reason = "a test, which needs its files")]
fn run(rom: &Path, boot_rom: &[u8], found: &mut Found) {
    let mut gb = GbBuilder::new(48000, DummyAudioCallback)
        .with_model(Model::DmgB)
        .with_run_bootrom(true)
        .with_rom(fs::read(rom).expect("a ROM").into_boxed_slice())
        .expect("the ROM loads")
        .with_bootrom(boot_rom.to_vec().into_boxed_slice())
        .build();
    gb.set_ppu_trace(true);
    let mut lcd_on = false;
    let mut line = Line::default();
    while gb.clock_units() < UNITS {
        gb.step();
        for event in gb.take_ppu_trace() {
            if lcd_on && event.timing_line != line.number {
                finish(&line, found);
                line = Line {
                    number: event.timing_line,
                    ..Line::default()
                };
            }
            let access = match event.kind {
                PpuEventKind::Write {
                    addr: 0xFF40,
                    value,
                } => {
                    let on = value & LCDC_ON != 0;
                    if on != lcd_on {
                        lcd_on = on;
                        finish(&line, found);
                        line = Line {
                            first: on,
                            ..Line::default()
                        };
                    }
                    continue;
                }
                PpuEventKind::VramRead { locked, .. } => Access {
                    h: event.timing_h,
                    write: false,
                    locked,
                },
                PpuEventKind::VramWrite { locked, .. } => Access {
                    h: event.timing_h,
                    write: true,
                    locked,
                },
                PpuEventKind::Mode3End if lcd_on => {
                    line.end.get_or_insert(event.timing_h);
                    continue;
                }
                PpuEventKind::VramLocks { read, write } if lcd_on => {
                    let h = event.timing_h;
                    if read {
                        line.read_on.get_or_insert(h);
                    }
                    if write {
                        line.write_on.get_or_insert(h);
                    }
                    if !read && !write {
                        line.off.get_or_insert(h);
                    }
                    continue;
                }
                _ => continue,
            };
            if !lcd_on {
                found.lcd_off += 1;
                found.count(access, pattern(access.h, 0, 0));
                continue;
            }
            line.accesses.push(access);
        }
    }
    finish(&line, found);
}

/// The netlist's rule: an access with mode 3's `levels` is locked out.
fn rule_locks(write: bool, levels: &str) -> bool {
    let off = |phase: usize| levels.as_bytes().get(phase) == Some(&b'0');
    let through = if write {
        (PHASE_E..=PHASE_G).any(off)
    } else {
        off(PHASE_H)
    };
    !through
}

/// Ceres's locks at `locks`, held to the netlist's rule for every end of
/// mode 3 with that parity and every access on the line's D edges (an
/// access sees the locks as the half dot it falls at left them): the
/// accesses, as (a write, the end, the access's half dot), where they
/// differ.
fn disagreements(locks: Locks) -> Vec<(bool, i32, i32)> {
    let mapping = if locks.first { &FIRST_LINE } else { &LINE };
    let mut differ = Vec::new();
    for end in (mapping.rise..RESYNC_H).filter(|end| (end & 1 == 1) == locks.odd_end) {
        let fall = end + FALL_FROM_END;
        for h in (0..RESYNC_H).filter(|h| h.rem_euclid(PHASES) == mapping.access) {
            for write in [false, true] {
                let on = mapping.rise + if write { locks.write_on } else { locks.read_on };
                let ceres = (on..end + locks.off).contains(&h);
                if ceres != rule_locks(write, &pattern(h, mapping.rise, fall)) {
                    differ.push((write, end, h));
                }
            }
        }
    }
    differ
}

#[test]
#[ignore = "needs the test ROMs and dmg-sim's boot ROM"]
#[expect(clippy::expect_used, reason = "a test, which needs its files")]
fn vram_accesses_follow_the_netlists_rule() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let boot_rom =
        fs::read(root.join("external/reference-implementations/dmg-sim/boot/quickboot.bin"))
            .expect("dmg-sim's quick boot ROM");
    let mut roms = Vec::new();
    for (dir, wanted) in [
        ("external/test-roms/gambatte/vram_m3", "dmg08"),
        ("external/test-roms/gbmicrotest", "vram"),
    ] {
        for entry in fs::read_dir(root.join(dir)).expect("the ROMs' directory") {
            let path = entry.expect("a ROM").path();
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            let is_rom = path
                .extension()
                .is_some_and(|ext| ext == "gb" || ext == "gbc");
            if name.contains(wanted) && is_rom {
                roms.push(path);
            }
        }
    }
    roms.sort();
    assert!(!roms.is_empty(), "no ROMs");

    let mut found = Found::default();
    for rom in &roms {
        run(rom, &boot_rom, &mut found);
    }

    eprintln!(
        "{} ROMs: {} accesses with the LCD off, {} in VBlank, {} in line 0 after the LCD \
         was turned on",
        roms.len(),
        found.lcd_off,
        found.vblank,
        found.first_line
    );
    eprintln!("mode 3 in A to H, then ceres's outcomes:");
    let mut broken = 0_u64;
    for ((write, levels), outcomes) in &found.tally {
        let expected = rule_locks(*write, levels);
        let kind = if *write { "write" } else { "read " };
        let mut parts = Vec::new();
        for (&locked, &count) in outcomes {
            let against = locked != expected;
            if against {
                broken += count;
            }
            parts.push(format!(
                "{} {count}{}",
                if locked { "blocked" } else { "through" },
                if against { " (!)" } else { "" }
            ));
        }
        eprintln!("  {kind} {levels}: {}", parts.join(", "));
    }
    eprintln!("ceres's locks on the visible lines: {:?}", found.locks);
    assert!(!found.locks.is_empty(), "no locks seen");
    for &locks in found.locks.keys() {
        let differ = disagreements(locks);
        assert!(
            differ.is_empty(),
            "ceres's locks {locks:?} are not the netlist's rule for {} accesses, the first \
             (a write, mode 3's end, the access): {:?}",
            differ.len(),
            differ.first()
        );
    }
    assert_eq!(found.misaligned, 0, "accesses off the mappings' D edge");
    assert_eq!(found.unplaced, 0, "accesses where mode 3 is not placed");
    assert_eq!(broken, 0, "accesses where ceres is not the netlist's rule");
}
