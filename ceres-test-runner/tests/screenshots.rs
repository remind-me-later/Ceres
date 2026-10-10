//! Integration tests that compare the final screen of a ROM with a
//! reference screenshot: the scribbltests, turtle-tests, mbc3-tester and
//! strikethrough ROMs (blargg, AGE, bully and little-things-gb have their own
//! files, with a test per ROM).
//!
//! The screenshot's name says which hardware it was taken on (`-dmg`, `-cgb`,
//! `-dmg-cgb`, AGE's `-dmgC`, `-cgbBCE`, `-ncmBC`, ...). Colours are compared
//! by rank (brightest first), so the palette does not matter. The ROMs run
//! from the real boot ROMs. The test is slow (minutes): run it with
//! `cargo test --test screenshots -- --ignored`.
//!
//! The failures that are known are listed in `screenshots_known_failures.txt`
//! (`<model> <rom>`): SameBoy, the reference this emulator follows, shows the
//! same screen on each of them. The test fails on any other failure, and on
//! a known failure that now passes. Run with `BLESS=1` to rewrite the list.

use ceres_core::Model;
use ceres_test_runner::{
    Run, check_known_failures,
    checks::{load_screenshot, rank_image},
    test_roms_dir,
};
use std::path::{Path, PathBuf};

/// Models a screenshot suffix stands for.
fn models(suffix: &str) -> Option<&'static [(&'static str, Model)]> {
    const DMG: &[(&str, Model)] = &[("dmg", Model::DmgB)];
    const CGB: &[(&str, Model)] = &[("cgbc", Model::CgbC), ("cgbe", Model::CgbE)];
    const BC: &[(&str, Model)] = &[("cgbc", Model::CgbC)];
    const E: &[(&str, Model)] = &[("cgbe", Model::CgbE)];
    const BOTH: &[(&str, Model)] = &[
        ("dmg", Model::DmgB),
        ("cgbc", Model::CgbC),
        ("cgbe", Model::CgbE),
    ];
    const NONE: &[(&str, Model)] = &[("dmg", Model::DmgB), ("cgbe", Model::CgbE)];
    Some(match suffix {
        "-dmg" | "-dmgC" => DMG,
        "-cgb" | "-cgbBCE" | "-ncmBCE" => CGB,
        "-ncmBC" => BC,
        "-ncmE" => E,
        "-dmg-cgb" | "-cgb-dmg" => BOTH,
        "" => NONE,
        _ => return None,
    })
}

fn frames(rom: &Path) -> u32 {
    let name = rom.file_name().and_then(|n| n.to_str()).unwrap_or_default();
    match name {
        "cpu_instrs.gb" | "dmg_sound.gb" | "cgb_sound.gb" => 5400,
        "mbc3-tester.gb" => 2300,
        _ if rom.to_string_lossy().contains("/blargg/") => 3300,
        _ => 700,
    }
}

fn run(rom: &Path, model: Model, reference: &Path) -> bool {
    let reference = load_screenshot(reference).expect("reference screenshot");
    Run::new(rom, model)
        .timeout(frames(rom))
        .machine()
        .is_ok_and(|gb| rank_image(gb.pixel_data_rgba()) == rank_image(&reference))
}

/// `(reference, ROM, suffix)` of every screenshot of `dir`.
fn collect(dir: &Path, out: &mut Vec<(PathBuf, PathBuf, String)>) {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .expect("directory")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            collect(&path, out);
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("png") {
            continue;
        }
        let base = path.file_stem().and_then(|s| s.to_str()).expect("name");
        let (stem, suffix) = [
            "-dmg-cgb", "-cgb-dmg", "-dmgC", "-cgbBCE", "-ncmBCE", "-ncmBC", "-ncmE", "-dmg",
            "-cgb",
        ]
        .iter()
        .find_map(|s| base.strip_suffix(s).map(|stem| (stem, (*s).to_string())))
        .unwrap_or((base, String::new()));
        // `statcount_auto-cgb-dmg.png` belongs to `statcount-auto.gb`.
        let rom = [stem.to_string(), stem.replace('_', "-")]
            .iter()
            .flat_map(|stem| ["gb", "gbc"].map(|e| path.with_file_name(format!("{stem}.{e}"))))
            .find(|p| p.exists());
        if let Some(rom) = rom {
            out.push((path, rom, suffix));
        }
    }
}

#[test]
#[ignore = "slow (minutes); run with --ignored"]
fn screenshots() {
    let root = test_roms_dir();
    let mut jobs = Vec::new();
    for dir in [
        "mbc3-tester",
        "scribbltests",
        "strikethrough",
        "turtle-tests",
    ] {
        collect(&root.join(dir), &mut jobs);
    }

    let mut failures = Vec::new();
    let mut ran = 0;
    for (reference, rom, suffix) in &jobs {
        let Some(models) = models(suffix) else {
            continue;
        };
        for (name, model) in models {
            ran += 1;
            if !run(rom, *model, reference) {
                let rel = rom.strip_prefix(&root).expect("under root");
                failures.push(format!(
                    "{name} {}",
                    rel.to_string_lossy().replace('\\', "/")
                ));
            }
        }
    }
    eprintln!("{ran} runs");
    assert!(ran > 20, "only {ran} screenshot runs");
    check_known_failures("screenshots_known_failures.txt", failures);
}
