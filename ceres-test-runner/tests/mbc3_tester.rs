//! Integration test for MBC3 bank switching functionality
//!
//! This test validates MBC3 ROM bank switching using the mbc3-tester ROM.

use ceres_test_runner::{
    load_test_rom, test_roms_dir,
    test_runner::{TestConfig, TestResult, TestRunner},
};

/// Run mbc3-tester test
fn run_mbc3_tester(
    model: ceres_core::Model,
    screenshot_name: &str,
) -> TestResult {
    let rom = match load_test_rom("mbc3-tester/mbc3-tester.gb") {
        Ok(rom) => rom,
        Err(e) => return TestResult::Failed(format!("Failed to load test ROM: {e}")),
    };

    let config = TestConfig {
        model,
        timeout_frames: 300, // Give it 5 seconds to complete
        expected_screenshot: Some(test_roms_dir().join(format!("mbc3-tester/{screenshot_name}"))),
        ..TestConfig::default()
    };

    let mut runner = match TestRunner::new(rom, config) {
        Ok(runner) => runner,
        Err(e) => return TestResult::Failed(format!("Failed to create test runner: {e}")),
    };

    runner.run()
}

#[test]
#[ignore]
fn test_mbc3_tester_cgb() {
    let result = run_mbc3_tester(
        ceres_core::Model::Cgb,
        "mbc3-tester-cgb.png",
    );

    match &result {
        TestResult::Passed => println!("✓ MBC3 tester passed (CGB mode)"),
        TestResult::Failed(msg) => {
            println!("✗ MBC3 tester failed (CGB mode): {msg}");
        }
        TestResult::Timeout => {
            println!("✗ MBC3 tester timed out (CGB mode)");
        }
        TestResult::Unknown => {
            println!("✗ MBC3 tester result unknown (CGB mode)");
        }
    }

    assert_eq!(
        result,
        TestResult::Passed,
        "MBC3 tester failed (CGB mode). This test validates MBC3 ROM bank switching."
    );
}

#[test]
#[ignore]
fn test_mbc3_tester_dmg() {
    let result = run_mbc3_tester(
        ceres_core::Model::Dmg,
        "mbc3-tester-dmg.png",
    );

    match &result {
        TestResult::Passed => println!("✓ MBC3 tester passed (DMG mode)"),
        TestResult::Failed(msg) => {
            println!("✗ MBC3 tester failed (DMG mode): {msg}");
        }
        TestResult::Timeout => {
            println!("✗ MBC3 tester timed out (DMG mode)");
        }
        TestResult::Unknown => {
            println!("✗ MBC3 tester result unknown (DMG mode)");
        }
    }

    assert_eq!(
        result,
        TestResult::Passed,
        "MBC3 tester failed (DMG mode). This test validates MBC3 ROM bank switching."
    );
}
