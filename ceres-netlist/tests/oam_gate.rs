//! What decides whether a CPU access to OAM gets through, on `NetChip`.
//!
//! The ROMs that read and write OAM around modes 2 and 3 (Gambatte's
//! `oam_access` for the DMG, gbmicrotest's `oam` ROMs but the DMA ones,
//! Mooneye's and AGE's DMG OAM ROMs) run from the power-on, and each CPU
//! access to FE00-FE9F is tallied by what held OAM in the eight phases of
//! its M-cycle, as each phase ends, A to H (an OAM DMA, the scan, render's
//! mode 3, or nothing), against what became of it: a read took OAM's byte
//! or FF, a write landed or not.
//!
//! The rule the tally is held to:
//!
//! - a write lands if OAM is free as one of E, F and G ends (the write
//!   strobe's phases, as for VRAM), or in the nanosecond between the scan
//!   and mode 3 (`oam-write-as-mode-3-starts`);
//! - a read takes FF unless OAM is free as G and H end, where OAM drives
//!   the CPU's bus; then it takes OAM's first latches, which the CPU's
//!   read strobe opens up to F: OAM's byte if OAM was free as F ended, the
//!   byte at the address render gave OAM if mode 3 held it then.
//!
//! `cargo test -p ceres-netlist --release --test oam_gate -- --ignored
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
const PHASE_F: usize = 5;
const PHASE_G: usize = 6;
const PHASE_H: usize = 7;

/// What became of an access.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Outcome {
    /// A read took OAM's byte; a write landed.
    Through,
    /// A read took FF; a write left the byte.
    Blocked,
    /// A read took the first latches' byte, which render's address put
    /// there (and is not OAM's byte at the CPU's address).
    Latched,
    /// Neither tells: the byte read is FF in OAM too, or the byte written
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
            Self::Latched => "latched",
            Self::Unknown => "unknown",
            Self::Other => "other",
        }
    }
}

/// What held OAM as a phase ended: an OAM DMA (`D`), the scan (`2`),
/// render's mode 3 (`3`), or nothing (`.`).
const fn holder(run: &Run) -> char {
    let i = run.chip.render.oam_inputs();
    if i.dma_run {
        'D'
    } else if i.mode2 {
        '2'
    } else if i.mode3 {
        '3'
    } else {
        '.'
    }
}

/// OAM's byte at the CPU's `addr`, if it is in FE00-FE9F.
fn oam_byte(run: &Run, addr: u16) -> Option<u8> {
    if !(0xFE00..0xFEA0).contains(&addr) {
        return None;
    }
    let word = usize::from((addr & 0xFF) >> 1);
    let oam = &run.chip.render.oam;
    if addr & 1 == 0 {
        oam.ram_even.get(word).copied()
    } else {
        oam.ram_odd.get(word).copied()
    }
}

/// The tally: per kind of access (write or read) and what held OAM, A to
/// H, the outcomes' counts.
type Tally = BTreeMap<(bool, String), BTreeMap<Outcome, u64>>;

/// A ROM's run, tallied.
fn run(rom: Vec<u8>, boot_rom: Vec<u8>, tally: &mut Tally) {
    let mut run = Run::new(rom, boot_rom);
    let mut holders = ['.'; PHASES];
    // OAM's first latches, even and odd, as F ended.
    let mut latched = (0, 0);
    // OAM's byte at the M-cycle's address as phase B found it.
    let mut before = None;
    for _ in 0..EDGES {
        let access = run.step();
        let phase = run.chip.clocks.phase() as usize;
        if let Some(slot) = holders.get_mut(phase) {
            *slot = holder(&run);
        }
        if phase == PHASE_F {
            let oam = &run.chip.render.oam;
            latched = (oam.first_even, oam.first_odd);
        }
        if phase == PHASE_B {
            before = oam_byte(&run, run.chip.core_out.addr);
        }
        let (Some(access), Some(was)) = (access, before) else {
            continue;
        };
        if !run.booted || !(0xFE00..0xFEA0).contains(&access.addr) {
            continue;
        }
        let outcome = if access.write {
            let now = oam_byte(&run, access.addr).unwrap_or(was);
            if was == access.value {
                Outcome::Unknown
            } else if now == access.value {
                Outcome::Through
            } else if now == was {
                Outcome::Blocked
            } else {
                Outcome::Other
            }
        } else {
            let latch = if access.addr & 1 == 0 {
                latched.0
            } else {
                latched.1
            };
            if was == 0xFF && access.value == 0xFF {
                Outcome::Unknown
            } else if access.value == was {
                Outcome::Through
            } else if access.value == 0xFF {
                Outcome::Blocked
            } else if access.value == latch {
                Outcome::Latched
            } else {
                Outcome::Other
            }
        };
        *tally
            .entry((access.write, holders.iter().collect()))
            .or_default()
            .entry(outcome)
            .or_default() += 1;
    }
}

/// The rule: what an access with OAM's `holders` must come to, but in the
/// nanosecond between the scan and mode 3.
fn rule(write: bool, holders: &str) -> Outcome {
    let free = |phase: usize| holders.as_bytes().get(phase) == Some(&b'.');
    if write {
        if (PHASE_E..=PHASE_G).any(free) {
            Outcome::Through
        } else {
            Outcome::Blocked
        }
    } else if !(free(PHASE_G) && free(PHASE_H)) {
        Outcome::Blocked
    } else if free(PHASE_F) {
        Outcome::Through
    } else {
        Outcome::Latched
    }
}

/// The scan's end and mode 3's start, both in the M-cycle: the nanosecond
/// between them, which an edge's settle does not show.
fn scan_then_mode3(holders: &str) -> bool {
    holders.contains("23")
}

/// The ROMs: in each directory, those whose names hold the first word and
/// none of the others.
#[expect(clippy::expect_used, reason = "a test, which needs its files")]
fn roms(root: &Path) -> Vec<std::path::PathBuf> {
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
fn oam_accesses_by_what_holds_oam() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let boot_rom =
        fs::read(root.join("external/reference-implementations/dmg-sim/boot/quickboot.bin"))
            .expect("dmg-sim's quick boot ROM");
    let roms = roms(&root);
    assert!(!roms.is_empty(), "no ROMs");

    let mut tally = Tally::new();
    for rom in &roms {
        run(fs::read(rom).expect("a ROM"), boot_rom.clone(), &mut tally);
    }

    eprintln!(
        "{} ROMs; what held OAM in A to H (D DMA, 2 the scan, 3 mode 3), then the outcomes:",
        roms.len()
    );
    let mut broken = 0_u64;
    for ((write, holders), outcomes) in &tally {
        let kind = if *write { "write" } else { "read " };
        let expected = rule(*write, holders);
        // A write in the gap lands; a latched read may show OAM's byte or FF
        // by chance (the latches' byte may be either), as an unknown one does.
        let allowed = |outcome: Outcome| {
            outcome == expected
                || outcome == Outcome::Unknown
                || (*write && scan_then_mode3(holders) && outcome == Outcome::Through)
                || (expected == Outcome::Latched && outcome != Outcome::Other)
        };
        let mut parts = Vec::new();
        for (&outcome, &count) in outcomes {
            let against = !allowed(outcome);
            if against {
                broken += count;
            }
            parts.push(format!(
                "{} {count}{}",
                outcome.name(),
                if against { " (!)" } else { "" }
            ));
        }
        eprintln!("  {kind} {holders}: {}", parts.join(", "));
    }
    assert_eq!(broken, 0, "accesses that do not follow the rule");
}
