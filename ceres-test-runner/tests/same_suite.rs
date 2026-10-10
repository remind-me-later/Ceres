//! Integration tests using the SameSuite ROMs.
//!
//! Source: <https://github.com/LIJI32/SameSuite>. A ROM reports through the
//! registers when it reaches its `ld b, b` breakpoint: the Fibonacci numbers
//! 3, 5, 8, 13, 21, 34 in B, C, D, E, H, L on success.
//!
//! Most ROMs are written for one family of CGB revisions (the suffix of the
//! name says which), so each ROM runs on every model and the ones that fail
//! are listed in `same_suite_known_failures.txt` (`<model> <rom>`): SameBoy,
//! the reference this emulator follows, shows the very same screen on each
//! of them. The test fails on any other failure, and on a known failure that
//! now passes. Run with `BLESS=1` to rewrite the list.

use ceres_core::{AudioCallback, GbBuilder, Model, Sample};
use ceres_test_runner::test_roms_dir;
use std::path::{Path, PathBuf};

/// Plenty for every ROM (the slowest needs ~100 frames).
const MAX_FRAMES: u32 = 600;

const CGB_MODELS: &[(&str, Model)] = &[
    ("cgb0", Model::Cgb0),
    ("cgba", Model::CgbA),
    ("cgbb", Model::CgbB),
    ("cgbc", Model::CgbC),
    ("cgbd", Model::CgbD),
    ("cgbe", Model::CgbE),
    ("agb", Model::Agb),
];

const SGB_MODELS: &[(&str, Model)] = &[("sgb", Model::Sgb), ("sgb2", Model::Sgb2)];

struct NoAudio;

impl AudioCallback for NoAudio {
    fn audio_sample(&self, _l: Sample, _r: Sample) {}
}

fn run(path: &Path, model: Model) -> bool {
    let rom = std::fs::read(path).expect("read ROM");
    let mut gb = GbBuilder::new(48000, NoAudio)
        .with_model(model)
        .with_run_bootrom(false)
        .with_rom(rom.into_boxed_slice())
        .expect("valid ROM")
        .build();

    for _ in 0..MAX_FRAMES {
        gb.run_frame();
        if gb.take_ld_b_b_breakpoint() || gb.take_illegal_opcode() {
            return (
                gb.cpu_b(),
                gb.cpu_c(),
                gb.cpu_d(),
                gb.cpu_e(),
                gb.cpu_h(),
                gb.cpu_l(),
            ) == (3, 5, 8, 13, 21, 34);
        }
    }
    false
}

fn collect_roms(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .expect("same-suite directory")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            collect_roms(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("gb") {
            out.push(path);
        }
    }
}

#[test]
fn same_suite() {
    let root = test_roms_dir().join("same-suite");
    let mut roms = Vec::new();
    collect_roms(&root, &mut roms);

    let mut failures = Vec::new();
    let mut ran = 0;
    for path in &roms {
        let rel = path.strip_prefix(&root).expect("under root");
        let rel = rel.to_string_lossy().replace('\\', "/");
        let models = if rel.starts_with("sgb/") {
            SGB_MODELS
        } else {
            CGB_MODELS
        };
        for (name, model) in models {
            ran += 1;
            if !run(path, *model) {
                failures.push(format!("{name} {rel}"));
            }
        }
    }
    failures.sort();

    if std::env::var("BLESS").is_ok() {
        let mut text = failures.join("\n");
        text.push('\n');
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("same_suite_known_failures.txt");
        std::fs::write(path, text).expect("write known failures");
        return;
    }

    let known: Vec<&str> = include_str!("same_suite_known_failures.txt")
        .lines()
        .filter(|l| !l.is_empty())
        .collect();
    let new_failures: Vec<&String> = failures
        .iter()
        .filter(|f| !known.contains(&f.as_str()))
        .collect();
    let fixed: Vec<&&str> = known
        .iter()
        .filter(|k| !failures.iter().any(|f| f == **k))
        .collect();
    eprintln!("{ran} runs, {} known failures", failures.len());
    assert!(ran > 400, "only {ran} SameSuite runs");
    assert!(new_failures.is_empty(), "new failures: {new_failures:#?}");
    assert!(
        fixed.is_empty(),
        "these known failures now pass (run with BLESS=1): {fixed:#?}"
    );
}
