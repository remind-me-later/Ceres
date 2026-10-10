//! PPU rendering accuracy tests: cgb-acid2, cgb-acid-hell and dmg-acid2.
//!
//! The screen must be the reference screenshot, pixel for pixel, when the ROM
//! reaches its `ld b, b` breakpoint.

use ceres_core::Model;
use ceres_test_runner::{
    Run,
    checks::{ExactScreenshotCheck, TestResult},
    rom_test, run_exact_screenshot, test_roms_dir, timeouts,
};

fn cgb_acid(rom: &str, screenshot: &str) -> TestResult {
    Run::new(rom, Model::CgbE)
        .timeout(timeouts::CGB_ACID2)
        .check(ExactScreenshotCheck::new(test_roms_dir().join(screenshot)))
}

rom_test!(test_cgb_acid2: cgb_acid("cgb-acid2/cgb-acid2.gbc", "cgb-acid2/cgb-acid2.png"));
rom_test!(test_cgb_acid_hell: cgb_acid(
    "cgb-acid-hell/cgb-acid-hell.gbc",
    "cgb-acid-hell/cgb-acid-hell.png"
));
rom_test!(test_dmg_acid2_dmg: run_exact_screenshot(
    "dmg-acid2/dmg-acid2.gb",
    Model::DmgB,
    timeouts::DMG_ACID2
));
rom_test!(test_dmg_acid2_cgb: run_exact_screenshot(
    "dmg-acid2/dmg-acid2.gb",
    Model::CgbE,
    timeouts::DMG_ACID2
));
