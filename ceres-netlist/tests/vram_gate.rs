//! What decides whether a CPU access to VRAM gets through, on `NetChip`.
//!
//! The ROMs that read and write VRAM around mode 3 (Gambatte's `vram_m3`
//! for the DMG, gbmicrotest's `vram` ROMs) run from the power-on, and each
//! CPU access to 8000-9FFF is tallied by mode 3's level in the eight phases
//! of its M-cycle (render's latch as each phase ends, A to H) against what
//! became of it: a read took VRAM's byte or FF, a write landed or not.
//!
//! The rule the tally is held to: a write lands if mode 3 is off as one of
//! E, F and G ends (the phases of the write strobe, `cpu_wr_sync`), and a
//! read takes VRAM's byte if mode 3 is off as H ends (the core's registers
//! take the data bus through G and H, and keep what H leaves).
//!
//! `cargo test -p ceres-netlist --release --test vram_gate -- --ignored
//! --nocapture`

#![expect(
    clippy::print_stderr,
    reason = "a test, which reports what it compared"
)]

use {
    ceres_netlist::run::Run,
    std::{collections::BTreeMap, fs, path::Path},
};

/// The edges each ROM runs: 0.29 s of the 4 MiHz crystal's half periods.
const EDGES: u64 = 2_437_000;
/// The phases' indices in an M-cycle (A is 0).
const PHASES: usize = 8;
const PHASE_B: usize = 1;
const PHASE_E: usize = 4;
const PHASE_G: usize = 6;
const PHASE_H: usize = 7;

/// What became of an access.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Outcome {
    /// A read took VRAM's byte; a write landed.
    Through,
    /// A read took FF; a write left the byte.
    Blocked,
    /// Neither tells: the byte read is FF in VRAM too, or the byte written
    /// was there already.
    Unknown,
    /// Something else: another byte read, or written.
    Other,
}

impl Outcome {
    const fn name(self) -> &'static str {
        match self {
            Self::Through => "through",
            Self::Blocked => "blocked",
            Self::Unknown => "unknown",
            Self::Other => "other",
        }
    }
}

/// Mode 3's level as each phase of an M-cycle ends, A first, as a string
/// of 0 and 1.
fn pattern(levels: [bool; PHASES]) -> String {
    levels
        .iter()
        .map(|&on| if on { '1' } else { '0' })
        .collect()
}

/// VRAM's address of the CPU's `addr`, if it is in 8000-9FFF.
fn vram_index(addr: u16) -> Option<usize> {
    (0x8000..0xA000)
        .contains(&addr)
        .then(|| usize::from(addr & 0x1FFF))
}

/// The tally: per kind of access (write or read) and mode 3's pattern, the
/// outcomes' counts.
type Tally = BTreeMap<(bool, String), BTreeMap<Outcome, u64>>;

/// A ROM's run, tallied.
fn run(rom: Vec<u8>, boot_rom: Vec<u8>, tally: &mut Tally) {
    let mut run = Run::new(rom, boot_rom);
    let mut levels = [false; PHASES];
    // VRAM's byte at the M-cycle's address as phase B found it, and whether
    // an OAM DMA ran in the M-cycle (it takes VRAM's bus too).
    let mut before = None;
    let mut dma = false;
    for _ in 0..EDGES {
        let access = run.step();
        let phase = run.chip.clocks.phase() as usize;
        levels[phase] = run.chip.render.mode3;
        if phase == 0 {
            dma = false;
        }
        dma |= run.chip.dma.run;
        if phase == PHASE_B {
            before = vram_index(run.chip.core_out.addr).map(|at| run.memories.vram[at]);
        }
        let (Some(access), Some(was)) = (access, before) else {
            continue;
        };
        let Some(at) = vram_index(access.addr) else {
            continue;
        };
        if dma || !run.booted {
            continue;
        }
        let outcome = if access.write {
            let now = run.memories.vram[at];
            if was == access.value {
                Outcome::Unknown
            } else if now == access.value {
                Outcome::Through
            } else if now == was {
                Outcome::Blocked
            } else {
                Outcome::Other
            }
        } else if was == 0xFF {
            Outcome::Unknown
        } else if access.value == was {
            Outcome::Through
        } else if access.value == 0xFF {
            Outcome::Blocked
        } else {
            Outcome::Other
        };
        *tally
            .entry((access.write, pattern(levels)))
            .or_default()
            .entry(outcome)
            .or_default() += 1;
    }
}

/// The rule: what an access with mode 3's `levels` must come to.
fn rule(write: bool, levels: &str) -> Outcome {
    let off = |phase: usize| levels.as_bytes().get(phase) == Some(&b'0');
    let through = if write {
        (PHASE_E..=PHASE_G).any(off)
    } else {
        off(PHASE_H)
    };
    if through {
        Outcome::Through
    } else {
        Outcome::Blocked
    }
}

#[test]
#[ignore = "needs the test ROMs and dmg-sim's boot ROM"]
#[expect(clippy::expect_used, reason = "a test, which needs its files")]
fn vram_accesses_follow_mode3_in_their_phases() {
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

    let mut tally = Tally::new();
    for rom in &roms {
        run(fs::read(rom).expect("a ROM"), boot_rom.clone(), &mut tally);
    }

    eprintln!("{} ROMs; mode 3 in A to H, then the outcomes:", roms.len());
    let mut broken = 0_u64;
    for ((write, levels), outcomes) in &tally {
        let expected = rule(*write, levels);
        let kind = if *write { "write" } else { "read " };
        let mut parts = Vec::new();
        for (outcome, count) in outcomes {
            let against = *outcome != expected && *outcome != Outcome::Unknown;
            if against {
                broken += count;
            }
            parts.push(format!(
                "{} {count}{}",
                outcome.name(),
                if against { " (!)" } else { "" }
            ));
        }
        eprintln!("  {kind} {levels}: {}", parts.join(", "));
    }
    assert_eq!(broken, 0, "accesses that do not follow the rule");
}
