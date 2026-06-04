//! PPU rendering accuracy tests
//!
//! These tests validate the Pixel Processing Unit implementation using
//! visual accuracy test ROMs like cgb-acid2 and dmg-acid2.

use ceres_core::Model;
use ceres_test_runner::{
    expected_screenshot_path, load_test_rom, test_roms_dir,
    test_runner::{ScreenshotCheck, TestConfig, TestRunner, timeouts},
};

#[test]
fn test_cgb_acid2() {
    let rom = match load_test_rom("cgb-acid2/cgb-acid2.gbc") {
        Ok(rom) => rom,
        Err(e) => panic!("Failed to load test ROM: {e}"),
    };

    let screenshot_path = test_roms_dir().join("cgb-acid2/cgb-acid2.png");
    let config = TestConfig {
        model: Model::CgbE,
        timeout_frames: timeouts::CGB_ACID2,
        ..TestConfig::default()
    };

    let check = Box::new(ScreenshotCheck::new(screenshot_path));

    let mut runner = match TestRunner::new(rom, config, check) {
        Ok(runner) => runner,
        Err(e) => panic!("Failed to create test runner: {e}"),
    };

    let result = runner.run();
    assert!(result.is_passed(), "CGB Acid2 PPU test failed");
}

#[test]
fn test_dmg_acid2_dmg() {
    let rom = match load_test_rom("dmg-acid2/dmg-acid2.gb") {
        Ok(rom) => rom,
        Err(e) => panic!("Failed to load test ROM: {e}"),
    };

    let screenshot_path =
        expected_screenshot_path("dmg-acid2/dmg-acid2.gb", ceres_core::Model::DmgB)
            .expect("Expected screenshot not found");

    let config = TestConfig {
        model: ceres_core::Model::DmgB,
        timeout_frames: timeouts::DMG_ACID2,
        ..TestConfig::default()
    };

    let check = Box::new(ScreenshotCheck::new(screenshot_path));

    let mut runner = match TestRunner::new(rom, config, check) {
        Ok(runner) => runner,
        Err(e) => panic!("Failed to create test runner: {e}"),
    };

    let result = runner.run();
    assert!(result.is_passed(), "DMG Acid2 PPU test failed (DMG mode)");
}

#[test]
fn test_dmg_acid2_cgb() {
    let rom = match load_test_rom("dmg-acid2/dmg-acid2.gb") {
        Ok(rom) => rom,
        Err(e) => panic!("Failed to load test ROM: {e}"),
    };

    let screenshot_path =
        expected_screenshot_path("dmg-acid2/dmg-acid2.gb", ceres_core::Model::CgbE)
            .expect("Expected screenshot not found");

    let config = TestConfig {
        model: ceres_core::Model::CgbE,
        timeout_frames: timeouts::DMG_ACID2,
        ..TestConfig::default()
    };

    let check = Box::new(ScreenshotCheck::new(screenshot_path));

    let mut runner = match TestRunner::new(rom, config, check) {
        Ok(runner) => runner,
        Err(e) => panic!("Failed to create test runner: {e}"),
    };

    let result = runner.run();

    assert!(result.is_passed(), "DMG Acid2 PPU test failed (CGB mode)");
}
