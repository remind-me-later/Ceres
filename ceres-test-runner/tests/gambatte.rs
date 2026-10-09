//! Integration test using the Gambatte hardware test ROMs.
//!
//! Source: <https://github.com/pokemon-speedrunning/gambatte-core> (`test/hwtests`).
//! Most ROMs show a hexadecimal result on screen whose expected value, for a
//! DMG-CPU-08 and/or a CPU-CGB-C, is encoded in the file name
//! (`..._dmg08_out<hex>`, `..._cgb04c_out<hex>`, `..._dmg08_cgb04c_out<hex>`).
//! The screen has to match Gambatte's monochrome digit glyphs.
//!
//! This is a slow test (minutes): run it with `cargo test --test gambatte --
//! --ignored`. Failures that are known (the APU and serial ports, hardware
//! behaviour the reference emulators do not model either) are listed in
//! `gambatte_known_failures_{dmg,cgb}.txt`; the test fails on any other
//! failure, and on a known failure that now passes. Run with `BLESS=1` to
//! rewrite the lists.

use ceres_test_runner::gambatte::{collect_roms, expected, roms_dir, run_rom};
use std::path::Path;

fn check(cgb: bool, known: &str, known_path: &str) {
    let root = roms_dir();
    let mut roms = Vec::new();
    collect_roms(&root, &mut roms);

    let mut failures = Vec::new();
    let mut ran = 0;
    for path in &roms {
        let stem = path.file_stem().and_then(|s| s.to_str()).expect("name");
        let Some(expected) = expected(stem, cgb) else {
            continue;
        };
        ran += 1;
        if run_rom(path, cgb, expected.len()).as_deref() != Some(expected.as_str()) {
            let rel = path.strip_prefix(&root).expect("under root");
            failures.push(rel.to_string_lossy().replace('\\', "/"));
        }
    }
    failures.sort();

    if std::env::var("BLESS").is_ok() {
        let mut text = failures.join("\n");
        text.push('\n');
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join(known_path);
        std::fs::write(path, text).expect("write known failures");
        return;
    }

    let known: Vec<&str> = known.lines().filter(|l| !l.is_empty()).collect();
    let new_failures: Vec<&String> = failures
        .iter()
        .filter(|f| !known.contains(&f.as_str()))
        .collect();
    let fixed: Vec<&&str> = known
        .iter()
        .filter(|k| !failures.iter().any(|f| f == **k))
        .collect();
    eprintln!("{ran} ROMs run, {} known failures", failures.len());
    assert!(new_failures.is_empty(), "new failures: {new_failures:#?}");
    assert!(
        fixed.is_empty(),
        "these known failures now pass (run with BLESS=1): {fixed:#?}"
    );
}

#[test]
#[ignore = "slow (minutes); run with --ignored"]
fn gambatte_dmg() {
    check(
        false,
        include_str!("gambatte_known_failures_dmg.txt"),
        "gambatte_known_failures_dmg.txt",
    );
}

#[test]
#[ignore = "slow (minutes); run with --ignored"]
fn gambatte_cgb() {
    check(
        true,
        include_str!("gambatte_known_failures_cgb.txt"),
        "gambatte_known_failures_cgb.txt",
    );
}
