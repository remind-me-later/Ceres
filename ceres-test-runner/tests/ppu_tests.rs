//! PPU rendering accuracy tests
//!
//! These tests validate the Pixel Processing Unit implementation using
//! visual accuracy test ROMs like cgb-acid2 and dmg-acid2.

use ceres_test_runner::{
    expected_screenshot_path, load_test_rom, test_roms_dir,
    test_runner::{TestConfig, TestResult, TestRunner, timeouts},
};

#[test]
fn test_cgb_acid2() {
    let rom = match load_test_rom("cgb-acid2/cgb-acid2.gbc") {
        Ok(rom) => rom,
        Err(e) => panic!("Failed to load test ROM: {e}"),
    };

    let config = TestConfig {
        timeout_frames: timeouts::CGB_ACID2,
        expected_screenshot: Some(test_roms_dir().join("cgb-acid2/cgb-acid2.png")),
        ..TestConfig::default()
    };

    let mut runner = match TestRunner::new(rom, config) {
        Ok(runner) => runner,
        Err(e) => panic!("Failed to create test runner: {e}"),
    };

    let result = runner.run();
    assert_eq!(result, TestResult::Passed, "CGB Acid2 PPU test failed");
}

#[test]
#[ignore = "DMG PPU rendering doesn't match reference - known issue"]
fn test_dmg_acid2_dmg() {
    let rom = match load_test_rom("dmg-acid2/dmg-acid2.gb") {
        Ok(rom) => rom,
        Err(e) => panic!("Failed to load test ROM: {e}"),
    };

    let config = TestConfig {
        model: ceres_core::Model::Dmg,
        timeout_frames: timeouts::DMG_ACID2,
        expected_screenshot: expected_screenshot_path(
            "dmg-acid2/dmg-acid2.gb",
            ceres_core::Model::Dmg,
        ),
        ..TestConfig::default()
    };

    let mut runner = match TestRunner::new(rom, config) {
        Ok(runner) => runner,
        Err(e) => panic!("Failed to create test runner: {e}"),
    };

    let result = runner.run();
    assert_eq!(
        result,
        TestResult::Passed,
        "DMG Acid2 PPU test failed (DMG mode)"
    );
}

#[test]
fn test_dmg_acid2_cgb() {
    let rom = match load_test_rom("dmg-acid2/dmg-acid2.gb") {
        Ok(rom) => rom,
        Err(e) => panic!("Failed to load test ROM: {e}"),
    };

    let config = TestConfig {
        model: ceres_core::Model::Cgb,
        timeout_frames: timeouts::DMG_ACID2,
        expected_screenshot: expected_screenshot_path(
            "dmg-acid2/dmg-acid2.gb",
            ceres_core::Model::Cgb,
        ),
        ..TestConfig::default()
    };

    let mut runner = match TestRunner::new(rom, config) {
        Ok(runner) => runner,
        Err(e) => panic!("Failed to create test runner: {e}"),
    };

    let result = runner.run();
    assert_eq!(
        result,
        TestResult::Passed,
        "DMG Acid2 PPU test failed (CGB mode)"
    );
}
