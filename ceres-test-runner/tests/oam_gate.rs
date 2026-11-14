//! What decides whether a CPU access to OAM gets through, on ceres, held
//! to the netlist's rule (`ceres-netlist`'s `tests/oam_gate.rs`): a write
//! lands if OAM is free as E, F or G ends, or in the nanosecond between the
//! scan and mode 3; a read takes FF unless OAM is free as G and H end, and
//! then OAM's first latches, which hold the byte render addressed if mode 3
//! held OAM as F ended.
//!
//! The same ROMs (Gambatte's `oam_access` for the DMG, gbmicrotest's `oam`
//! ROMs but the DMA ones, Mooneye's and AGE's DMG OAM ROMs) run from
//! dmg-sim's quick boot ROM, and ceres's PPU trace gives each CPU access to
//! FE00-FE9F, with its half dot `h` and whether ceres's locks (or an OAM
//! DMA) held it off, and per line the locks' changes and where mode 3
//! ended. What holds OAM in the netlist is put on that clock by the
//! mappings, as `vram_gate.rs` does for mode 3:
//!
//! - the scan, from h = 906 of the line before (mode 0 falls at the scan's
//!   line start there, `mapping-mode3-end`) up to h = 154; not in line 0
//!   after the LCD is turned on, nor for the lines of VBlank;
//! - mode 3, from h = 154 (160 in line 0 after the LCD is turned on) up to
//!   two half dots after ceres's mode 3 ends;
//! - an access at h, the D edge, has its phases A to H from h - 3 to h + 4
//!   (h = 2 (mod 8) in line 0 after the LCD is turned on, until the resync).
//!
//! A read where mode 3 still held OAM as F ended, and not as G and H did,
//! takes in the netlist the byte render's address left in OAM's first
//! latches; ceres takes OAM's byte (`oam-read-as-mode-3-ends`). Those reads
//! are counted apart.
//!
//! The trace also gives where ceres's locks change on each visible line; at
//! those positions ceres's locks are held to the rule for every end of mode
//! 3 and every access position.
//!
//! `cargo test -p ceres-test-runner --release --test oam_gate -- --ignored
//! --nocapture`

#![expect(
    clippy::print_stderr,
    reason = "a test, which reports what it compared"
)]

use {
    ceres_core::{GbBuilder, Model, PpuEventKind},
    ceres_test_runner::DummyAudioCallback,
    std::{
        collections::BTreeMap,
        fs,
        path::{Path, PathBuf},
    },
};

/// The run of each ROM: 0.29 s in 8 MiHz units, as the netlist's tally.
const UNITS: u64 = 2_437_000;
/// LCDC's bit that turns the LCD on.
const LCDC_ON: u8 = 0x80;
/// The first line of VBlank, and the last line.
const VBLANK_LINE: u8 = 144;
const LAST_LINE: u8 = 153;
/// The phases of an M-cycle, and E to H among them (A is 0).
const PHASES: i32 = 8;
const PHASE_E: usize = 4;
const PHASE_F: usize = 5;
const PHASE_G: usize = 6;
const PHASE_H: usize = 7;
/// Phase A of an access's M-cycle, from the access's half dot.
const A_FROM_ACCESS: i32 = -3;
/// Where the pixel pipeline's line end puts ceres's clock (`RESYNC_H`),
/// and the half dots of a line.
const RESYNC_H: i32 = 893;
const LINE_H: i32 = 912;
/// Where the next line's scan takes OAM.
const SCAN_START: i32 = 906;
/// Where mode 3 rises: on a line, and in line 0 after the LCD is turned on.
const MODE3_RISE: i32 = 154;
const FIRST_LINE_MODE3_RISE: i32 = 160;
/// Where mode 3 falls, from where ceres's ended.
const FALL_FROM_END: i32 = 2;
/// The half dots between which a lock change counts from mode 3's end.
const END_RELATIVE: std::ops::Range<i32> = 200..880;

/// A CPU access to OAM: its half dot, a write, held off by ceres.
#[derive(Clone, Copy)]
struct Access {
    h: i32,
    write: bool,
    blocked: bool,
}

/// Where a lock changed: at a half dot of the line, or from mode 3's end.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum At {
    Line(i32),
    End(i32),
}

/// A visible line's lock changes: where, the read lock as a read meets it
/// (ceres's DMG read meets the write lock too), the write lock.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
struct Layout {
    /// Line 0 after the LCD was turned on.
    first: bool,
    /// The next line has a scan.
    next_scans: bool,
    changes: Vec<(At, bool, bool)>,
}

/// A line's accesses, lock changes and the end of ceres's mode 3.
#[derive(Default)]
struct Line {
    number: u8,
    first: bool,
    /// The LCD was turned off, or the run ended, before the line did.
    cut: bool,
    end: Option<i32>,
    locks: Vec<(i32, bool, bool)>,
    accesses: Vec<Access>,
}

/// Per kind of access (a write) and what held OAM, A to H, ceres's outcomes
/// (blocked) and their counts.
type Tally = BTreeMap<(bool, String), BTreeMap<bool, u64>>;

/// What the runs told.
#[derive(Default)]
struct Found {
    tally: Tally,
    /// Accesses with the LCD off, and that an OAM DMA held off.
    lcd_off: u64,
    dma: u64,
    /// Accesses not on the D edge the mappings give, and on a visible line
    /// where ceres's mode 3 did not end.
    misaligned: u64,
    unplaced: u64,
    /// The visible lines' lock layouts, and how many lines had them.
    layouts: BTreeMap<Layout, u64>,
}

/// Whether the line after `number` has a scan.
const fn next_scans(number: u8) -> bool {
    number < VBLANK_LINE - 1 || number == LAST_LINE
}

/// What held OAM as each phase of an access at `h` ended, on line `number`
/// (line 0 after the LCD was turned on if `first`) where ceres's mode 3
/// ended at `end`: the scan (`2`), mode 3 (`3`) or nothing (`.`).
fn holders(number: u8, first: bool, h: i32, end: Option<i32>) -> String {
    let visible = number < VBLANK_LINE;
    let rise = if first {
        FIRST_LINE_MODE3_RISE
    } else {
        MODE3_RISE
    };
    let fall = end.map_or(rise, |end| end + FALL_FROM_END);
    (0..PHASES)
        .map(|phase| {
            let at = h + A_FROM_ACCESS + phase;
            if visible && !first && at < MODE3_RISE {
                '2'
            } else if visible && (rise..fall).contains(&at) {
                '3'
            } else if next_scans(number) && at >= SCAN_START {
                '2'
            } else {
                '.'
            }
        })
        .collect()
}

/// The netlist's rule: whether an access with OAM's `holders` is held off,
/// and whether a read takes the byte render latched.
fn rule(write: bool, holders: &str) -> (bool, bool) {
    let free = |phase: usize| holders.as_bytes().get(phase) == Some(&b'.');
    if write {
        let gap = holders.contains("23");
        (!gap && !(PHASE_E..=PHASE_G).any(free), false)
    } else if !(free(PHASE_G) && free(PHASE_H)) {
        (true, false)
    } else {
        (false, !free(PHASE_F))
    }
}

/// A line, done: its lock layout and its accesses tallied.
fn finish(line: &Line, found: &mut Found) {
    if line.number < VBLANK_LINE
        && !line.cut
        && let Some(end) = line.end
    {
        let changes = line
            .locks
            .iter()
            .map(|&(h, read, write)| {
                let at = if END_RELATIVE.contains(&h) {
                    At::End(h - end)
                } else {
                    At::Line(h)
                };
                (at, read, write)
            })
            .collect();
        *found
            .layouts
            .entry(Layout {
                first: line.first,
                next_scans: next_scans(line.number),
                changes,
            })
            .or_default() += 1;
    }
    for &access in &line.accesses {
        let aligned = if line.first && access.h < RESYNC_H {
            2
        } else {
            0
        };
        if access.h.rem_euclid(PHASES) != aligned {
            found.misaligned += 1;
            continue;
        }
        if line.number < VBLANK_LINE
            && line.end.is_none()
            && access.h - A_FROM_ACCESS + PHASES > MODE3_RISE
        {
            found.unplaced += 1;
            continue;
        }
        let levels = holders(line.number, line.first, access.h, line.end);
        found.count(access, levels);
    }
}

impl Found {
    fn count(&mut self, access: Access, levels: String) {
        *self
            .tally
            .entry((access.write, levels))
            .or_default()
            .entry(access.blocked)
            .or_default() += 1;
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
            let h = event.timing_h;
            let (write, locked, dma) = match event.kind {
                PpuEventKind::Write {
                    addr: 0xFF40,
                    value,
                } => {
                    let on = value & LCDC_ON != 0;
                    if on != lcd_on {
                        lcd_on = on;
                        line.cut = !on;
                        finish(&line, found);
                        line = Line {
                            first: on,
                            ..Line::default()
                        };
                    }
                    continue;
                }
                PpuEventKind::OamRead { locked, dma, .. } => (false, locked, dma),
                PpuEventKind::OamWrite { locked, dma, .. } => (true, locked, dma),
                PpuEventKind::Mode3End if lcd_on => {
                    line.end.get_or_insert(h);
                    continue;
                }
                PpuEventKind::OamLocks { read, write } if lcd_on => {
                    line.locks.push((h, read || write, write));
                    continue;
                }
                _ => continue,
            };
            found.dma += u64::from(dma);
            let access = Access {
                h,
                write,
                blocked: locked || dma,
            };
            if lcd_on {
                line.accesses.push(access);
            } else {
                found.lcd_off += 1;
                found.count(access, ".".repeat(8));
            }
        }
    }
    // The run ends in the line.
    line.cut = true;
    finish(&line, found);
}

/// Ceres's locks at `layout`, held to the netlist's rule for every odd end
/// of mode 3 and every access on the line's D edges (an access sees the
/// locks as the half dot it falls at left them), but the latched reads:
/// the accesses, as (a write, the end, the access's half dot), where they
/// differ.
fn disagreements(layout: &Layout) -> Vec<(bool, i32, i32)> {
    let number = if layout.first {
        0
    } else if layout.next_scans {
        1
    } else {
        VBLANK_LINE - 1
    };
    let mut differ = Vec::new();
    for end in (END_RELATIVE.start + 1..END_RELATIVE.end).step_by(2) {
        let locks_at = |h: i32| {
            // The line before's scan locks both, but before line 0 after
            // the LCD was turned on.
            let mut locks = (!layout.first, !layout.first);
            for &(at, read, write) in &layout.changes {
                let at = match at {
                    At::Line(h) => h,
                    At::End(from_end) => end + from_end,
                };
                if at <= h {
                    locks = (read, write);
                }
            }
            locks
        };
        let aligned = |h: i32| {
            let edge = if layout.first && h < RESYNC_H { 2 } else { 0 };
            h.rem_euclid(PHASES) == edge
        };
        for h in (0..LINE_H).filter(|&h| aligned(h)) {
            let held = holders(number, layout.first, h, Some(end));
            let (read_locked, write_locked) = locks_at(h);
            for write in [false, true] {
                let (blocked, latched) = rule(write, &held);
                let ceres = if write { write_locked } else { read_locked };
                if !latched && ceres != blocked {
                    differ.push((write, end, h));
                }
            }
        }
    }
    differ
}

/// The ROMs: in each directory, those whose names hold the first word and
/// none of the others.
#[expect(clippy::expect_used, reason = "a test, which needs its files")]
fn roms(root: &Path) -> Vec<PathBuf> {
    let mut roms = Vec::new();
    for (dir, wanted, unwanted) in [
        ("external/test-roms/gambatte/oam_access", "dmg08", &[][..]),
        ("external/test-roms/gbmicrotest", "oam", &["dma"][..]),
        ("external/test-roms/age-test-roms/oam", "dmgC", &[][..]),
        (
            "external/test-roms/mooneye-test-suite/acceptance/bits",
            "mem_oam",
            &[][..],
        ),
        (
            "external/test-roms/mooneye-test-suite/acceptance/ppu",
            "intr_2_oam_ok_timing",
            &[][..],
        ),
    ] {
        for entry in fs::read_dir(root.join(dir)).expect("the ROMs' directory") {
            let path = entry.expect("a ROM").path();
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            let is_rom = path
                .extension()
                .is_some_and(|ext| ext == "gb" || ext == "gbc");
            if is_rom && name.contains(wanted) && !unwanted.iter().any(|u| name.contains(u)) {
                roms.push(path);
            }
        }
    }
    roms.sort();
    roms
}

#[test]
#[ignore = "needs the test ROMs and dmg-sim's boot ROM"]
#[expect(clippy::expect_used, reason = "a test, which needs its files")]
fn oam_accesses_follow_the_netlists_rule() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let boot_rom =
        fs::read(root.join("external/reference-implementations/dmg-sim/boot/quickboot.bin"))
            .expect("dmg-sim's quick boot ROM");
    let roms = roms(&root);
    assert!(!roms.is_empty(), "no ROMs");
    let mut found = Found::default();
    for rom in &roms {
        run(rom, &boot_rom, &mut found);
    }

    eprintln!(
        "{} ROMs: {} accesses with the LCD off, {} that an OAM DMA held off",
        roms.len(),
        found.lcd_off,
        found.dma
    );
    eprintln!("what held OAM in A to H (2 the scan, 3 mode 3), then ceres's outcomes:");
    let (mut broken, mut latched_reads) = (0_u64, 0_u64);
    for ((write, levels), outcomes) in &found.tally {
        let (expected, latched) = rule(*write, levels);
        let kind = if *write { "write" } else { "read " };
        let mut parts = Vec::new();
        for (&blocked, &count) in outcomes {
            let against = !latched && blocked != expected;
            if against {
                broken += count;
            }
            if latched {
                latched_reads += count;
            }
            parts.push(format!(
                "{} {count}{}",
                if blocked { "blocked" } else { "through" },
                if against {
                    " (!)"
                } else if latched {
                    " (the netlist's: render's byte)"
                } else {
                    ""
                }
            ));
        }
        eprintln!("  {kind} {levels}: {}", parts.join(", "));
    }
    eprintln!("ceres's lock layouts on the visible lines:");
    for (layout, count) in &found.layouts {
        eprintln!("  {count} lines: {layout:?}");
        let differ = disagreements(layout);
        assert!(
            differ.is_empty(),
            "ceres's locks are not the netlist's rule for {} accesses, the first (a write, mode \
             3's end, the access): {:?}",
            differ.len(),
            differ.first()
        );
    }
    eprintln!("{latched_reads} reads take OAM's byte where the netlist's take render's");
    assert_eq!(found.misaligned, 0, "accesses off the mappings' D edge");
    assert_eq!(found.unplaced, 0, "accesses where mode 3 is not placed");
    assert_eq!(
        found.dma, 0,
        "accesses an OAM DMA held off: not in these ROMs"
    );
    assert_eq!(broken, 0, "accesses where ceres is not the netlist's rule");
}
