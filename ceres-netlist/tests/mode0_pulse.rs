//! What render's mode 3 latch (XYMU) does on `NetChip` where mode 0 pulses
//! before mode 3 ends: a sprite at X 167 stops mode 3 for about 4 ns as the
//! pixel counter reaches it (`sprite-at-167-mode-0-pulse`), and its fetch
//! holds mode 3 again.
//!
//! The VRAM and OAM gates (`vram_gate.rs`, `oam_gate.rs`) are rules on
//! XYMU's level as each phase ends. Ceres keeps the CPU's locks on through
//! such a pulse, to mode 3's real end (`ceres-core`'s
//! `ppu::display::lock_proofs`), so it agrees with the netlist there if
//! XYMU stays up through the pulse: `locks-with-mode-0-pulse`. Gambatte's
//! DMG ROMs with a sprite at X 167 (A7) run from the quick boot ROM, and at
//! each edge with a pulse the test records XYMU after it, and how many edges
//! later XYMU falls.
//!
//! `cargo test -p ceres-netlist --release --test mode0_pulse -- --ignored
//! --nocapture`

#![expect(clippy::print_stderr, reason = "a test, which reports what it found")]

use {
    ceres_netlist::run::Run,
    std::{collections::BTreeMap, fs, path::Path},
};

/// The edges each ROM runs: 0.29 s of the 4 MiHz crystal's half periods.
const EDGES: u64 = 2_437_000;

#[test]
#[ignore = "runs 10 ROMs on NetChip (80 s in release)"]
#[expect(clippy::expect_used, reason = "a test, which needs its files")]
fn mode3_latch_holds_through_mode0_pulses() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let boot_rom =
        fs::read(root.join("external/reference-implementations/dmg-sim/boot/quickboot.bin"))
            .expect("dmg-sim's quick boot ROM");
    let mut roms = Vec::new();
    for dir in ["sprites", "window", "enable_display", "m2int_m0irq"] {
        let Ok(entries) = fs::read_dir(root.join("external/test-roms/gambatte").join(dir)) else {
            continue;
        };
        for entry in entries {
            let path = entry.expect("a ROM").path();
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if name.contains("a7") && name.contains("dmg08") {
                roms.push(path);
            }
        }
    }
    roms.sort();
    assert!(!roms.is_empty(), "no ROMs");

    let (mut pulses, mut held) = (0_u64, 0_u64);
    // Edges from a pulse to XYMU's fall, and how many pulses had each.
    let mut falls: BTreeMap<u64, u64> = BTreeMap::new();
    for rom in &roms {
        let mut run = Run::new(fs::read(rom).expect("a ROM"), boot_rom.clone());
        let mut since_pulse = None;
        let mut mode0_before = false;
        for _ in 0..EDGES {
            run.step();
            let render = &run.chip.render;
            if let Some(edges) = since_pulse.as_mut() {
                *edges += 1;
                if !render.mode3 {
                    *falls.entry(*edges).or_default() += 1;
                    since_pulse = None;
                }
            }
            // A pulse: mode 0 rose and fell within the edge (the flag also
            // marks mode 0's fall at a line's start, which it held before).
            let pulse = render.mode0_pulse && !mode0_before;
            mode0_before = render.mode0();
            if pulse && run.booted {
                pulses += 1;
                if render.mode3 {
                    held += 1;
                    since_pulse = Some(0);
                }
            }
        }
    }

    let falls: Vec<String> = falls
        .iter()
        .map(|(edges, count)| format!("{edges} edges: {count}"))
        .collect();
    eprintln!(
        "{} ROMs: {pulses} mode 0 pulses, XYMU up after {held}; its fall after: {}",
        roms.len(),
        falls.join(", ")
    );
    assert!(pulses > 0, "no mode 0 pulse");
    assert_eq!(held, pulses, "XYMU fell in a pulse");
}
