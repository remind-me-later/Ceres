//! Integration test for MBC3 bank switching functionality
//!
//! This test validates MBC3 ROM bank switching using the mbc3-tester ROM.

use ceres_test_runner::{
    load_test_rom, test_roms_dir,
    test_runner::{ScreenshotCheck, TestConfig, TestResult, TestRunner},
};

/// Run mbc3-tester test
fn run_mbc3_tester(model: ceres_core::Model, screenshot_name: &str) -> TestResult {
    let rom = match load_test_rom("mbc3-tester/mbc3-tester.gb") {
        Ok(rom) => rom,
        Err(e) => return TestResult::Error(format!("Failed to load test ROM: {e}")),
    };

    let screenshot_path = test_roms_dir().join(format!("mbc3-tester/{screenshot_name}"));
    let config = TestConfig {
        model,
        timeout_frames: 300, // Give it 5 seconds to complete
        ..TestConfig::default()
    };

    let check = Box::new(ScreenshotCheck::new(screenshot_path));

    let mut runner = match TestRunner::new(rom, config, check) {
        Ok(runner) => runner,
        Err(e) => return TestResult::Error(format!("Failed to create test runner: {e}")),
    };

    runner.run()
}

#[test]
#[ignore]
fn test_mbc3_tester_cgb() {
    let result = run_mbc3_tester(ceres_core::Model::CgbE, "mbc3-tester-cgb.png");

    match &result {
        TestResult::Passed => println!("✓ MBC3 tester passed (CGB mode)"),
        TestResult::Failed(msg) => println!("✗ MBC3 tester failure (CGB mode): {msg}"),
        TestResult::Error(msg) => println!("✗ MBC3 tester error (CGB mode): {msg}"),
    }

    assert!(
        result.is_passed(),
        "MBC3 tester failed (CGB mode). This test validates MBC3 ROM bank switching."
    );
}

#[test]
#[ignore]
fn test_mbc3_tester_dmg() {
    let result = run_mbc3_tester(ceres_core::Model::DmgB, "mbc3-tester-dmg.png");

    match &result {
        TestResult::Passed => println!("✓ MBC3 tester passed (DMG mode)"),
        TestResult::Failed(msg) => println!("✗ MBC3 tester failure (DMG mode): {msg}"),
        TestResult::Error(msg) => println!("✗ MBC3 tester error (DMG mode): {msg}"),
    }

    assert!(
        result.is_passed(),
        "MBC3 tester failed (DMG mode). This test validates MBC3 ROM bank switching."
    );
}
