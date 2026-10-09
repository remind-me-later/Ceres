//! Integration test using the Gambatte hardware test ROMs.
//!
//! Source: <https://github.com/pokemon-speedrunning/gambatte-core> (`test/hwtests`).
//! Most ROMs show a hexadecimal result on screen whose expected value, for a
//! DMG-CPU-08 and/or a CPU-CGB-C, is encoded in the file name
//! (`..._dmg08_out<hex>`, `..._cgb04c_out<hex>`, `..._dmg08_cgb04c_out<hex>`).
//! The screen has to match Gambatte's monochrome digit glyphs.
//!
//! This is a slow test (minutes): run it with `cargo test --test gambatte --
//! --ignored`. Every ROM has to pass.

use ceres_test_runner::gambatte::{collect_roms, expected, roms_dir, run_rom};

fn check(cgb: bool) {
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

    eprintln!("{ran} ROMs run, {} failures", failures.len());
    assert!(failures.is_empty(), "failures: {failures:#?}");
}

#[test]
#[ignore = "slow (minutes); run with --ignored"]
fn gambatte_dmg() {
    check(false);
}

#[test]
#[ignore = "slow (minutes); run with --ignored"]
fn gambatte_cgb() {
    check(true);
}
