//! Integration tests using the SameSuite ROMs.
//!
//! Source: <https://github.com/LIJI32/SameSuite>. A ROM reports through the
//! registers when it reaches its `ld b, b` breakpoint: the Fibonacci numbers
//! 3, 5, 8, 13, 21, 34 in B, C, D, E, H, L on success.
//!
//! Each ROM runs on the models it is written for, and must pass on all of
//! them (see [`targets`]). On the other revisions real hardware fails many of
//! them, so they are not run there.

use ceres_core::Model;
use ceres_test_runner::{Run, checks::RegisterCheck, collect_roms, test_roms_dir, timeouts};

const MODELS: &[(&str, Model)] = &[
    ("dmg", Model::DmgB),
    ("cgb0", Model::Cgb0),
    ("cgba", Model::CgbA),
    ("cgbb", Model::CgbB),
    ("cgbc", Model::CgbC),
    ("cgbd", Model::CgbD),
    ("cgbe", Model::CgbE),
    ("agb", Model::Agb),
    ("sgb", Model::Sgb),
    ("sgb2", Model::Sgb2),
];

/// The CGB revision a suffix letter names.
fn revision(letter: char) -> Model {
    match letter {
        '0' => Model::Cgb0,
        'A' => Model::CgbA,
        'B' => Model::CgbB,
        'C' => Model::CgbC,
        'D' => Model::CgbD,
        'E' => Model::CgbE,
        _ => panic!("unknown CGB revision {letter}"),
    }
}

/// The models the ROM at `rel` (under `same-suite/`) is written for.
///
/// - A suffix names the CGB revisions: `-cgb0B` is for the CGB-0 and B, `-A`
///   for the CGB-A.
/// - The other APU ROMs are for the CGB-D and E, as the APU README says: the
///   CGB-C and older glitch when the PCM registers are read, which only
///   spares the channel 3 ROMs and the ones about no channel in particular.
///   The DMG passes `div_write_trigger` and `div_write_trigger_10` (the
///   others need the PCM registers), and the CGB-D fails
///   `channel_1_sweep_restart_2`. None was verified on the AGB.
/// - The DMA, interrupt and PPU ROMs document no revision: every CGB model.
/// - The SGB ROMs are for both SGBs.
fn targets(rel: &str) -> Vec<Model> {
    let name = rel.rsplit('/').next().expect("a file name");
    let stem = name.split('.').next().expect("a stem");

    if rel.starts_with("sgb/") {
        return vec![Model::Sgb, Model::Sgb2];
    }
    if !rel.starts_with("apu/") {
        return vec![
            Model::Cgb0,
            Model::CgbA,
            Model::CgbB,
            Model::CgbC,
            Model::CgbD,
            Model::CgbE,
            Model::Agb,
        ];
    }
    if let Some((_, suffix)) = stem.split_once('-') {
        return suffix
            .trim_start_matches("cgb")
            .chars()
            .map(revision)
            .collect();
    }
    if stem == "channel_1_sweep_restart_2" {
        return vec![Model::CgbE];
    }
    let mut models = Vec::new();
    if matches!(stem, "div_write_trigger" | "div_write_trigger_10") {
        models.push(Model::DmgB);
    }
    if rel.starts_with("apu/channel_3/") || !rel.starts_with("apu/channel_") {
        models.push(Model::CgbC);
    }
    models.extend([Model::CgbD, Model::CgbE]);
    models
}

fn model_name(model: Model) -> &'static str {
    MODELS
        .iter()
        .find(|(_, m)| *m == model)
        .map_or("?", |(name, _)| name)
}

#[test]
fn same_suite() {
    let root = test_roms_dir().join("same-suite");
    let mut failures = Vec::new();
    let mut ran = 0;
    for path in collect_roms(&root) {
        let rel = path.strip_prefix(&root).expect("under root");
        let rel = rel.to_string_lossy().replace('\\', "/");
        for model in targets(&rel) {
            ran += 1;
            let result = Run::new(&path, model)
                .skip_boot_rom()
                .timeout(timeouts::SAME_SUITE)
                .check(RegisterCheck);
            if !result.is_passed() {
                failures.push(format!("{} {rel}: {result:?}", model_name(model)));
            }
        }
    }
    eprintln!("{ran} runs");
    assert!(ran > 150, "only {ran} SameSuite runs");
    assert!(failures.is_empty(), "failures: {failures:#?}");
}
