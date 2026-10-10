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

use ceres_core::Model;
use ceres_test_runner::{
    Run, check_known_failures, checks::RegisterCheck, collect_roms, test_roms_dir, timeouts,
};

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

#[test]
fn same_suite() {
    let root = test_roms_dir().join("same-suite");
    let mut failures = Vec::new();
    let mut ran = 0;
    for path in collect_roms(&root) {
        let rel = path.strip_prefix(&root).expect("under root");
        let rel = rel.to_string_lossy().replace('\\', "/");
        let models = if rel.starts_with("sgb/") {
            SGB_MODELS
        } else {
            CGB_MODELS
        };
        for (name, model) in models {
            ran += 1;
            let result = Run::new(&path, *model)
                .skip_boot_rom()
                .timeout(timeouts::SAME_SUITE)
                .check(RegisterCheck);
            if !result.is_passed() {
                failures.push(format!("{name} {rel}"));
            }
        }
    }
    eprintln!("{ran} runs");
    assert!(ran > 400, "only {ran} SameSuite runs");
    check_known_failures("same_suite_known_failures.txt", failures);
}
