//! Integration test for MBC3 bank switching functionality
//!
//! This test validates MBC3 ROM bank switching using the mbc3-tester ROM.
//! Tracing is enabled to demonstrate the trace collection and export capabilities
//! for debugging test failures.

use ceres_test_runner::{
    load_test_rom, test_roms_dir,
    test_runner::{TestConfig, TestResult, TestRunner},
};

/// Run mbc3-tester with trace collection enabled to validate the proposal implementation
fn run_mbc3_tester_with_trace(
    model: ceres_core::Model,
    screenshot_name: &str,
    test_name: &str,
) -> TestResult {
    let rom = match load_test_rom("mbc3-tester/mbc3-tester.gb") {
        Ok(rom) => rom,
        Err(e) => return TestResult::Failed(format!("Failed to load test ROM: {e}")),
    };

    // Enable trace collection with JSONL format for machine-friendly analysis
    let config = TestConfig {
        model,
        timeout_frames: 300, // Give it 5 seconds to complete
        expected_screenshot: Some(test_roms_dir().join(format!("mbc3-tester/{screenshot_name}"))),
        enable_trace: true,            // Enable trace collection
        export_trace_on_failure: true, // Export traces on failure
        trace_buffer_size: 10_000,     // Keep last 10k trace entries
        trace_format: ceres_test_runner::test_runner::TraceFormat::JsonLines, // Use JSONL format
        test_name: Some(test_name.to_string()), // Set test name for better file naming
        ..TestConfig::default()
    };

    let mut runner = match TestRunner::new(rom, config) {
        Ok(runner) => runner,
        Err(e) => return TestResult::Failed(format!("Failed to create test runner: {e}")),
    };

    runner.run()
}

#[test]
fn test_mbc3_tester_cgb() {
    let result = run_mbc3_tester_with_trace(
        ceres_core::Model::Cgb,
        "mbc3-tester-cgb.png",
        "test_mbc3_tester_cgb",
    );

    // If the test fails, traces will be exported to target/traces/
    match &result {
        TestResult::Passed => println!("✓ MBC3 tester passed (CGB mode)"),
        TestResult::Failed(msg) => {
            println!("✗ MBC3 tester failed (CGB mode): {msg}");
            println!("Check target/traces/ for exported trace data");
        }
        TestResult::Timeout => {
            println!("✗ MBC3 tester timed out (CGB mode)");
            println!("Check target/traces/ for exported trace data");
        }
        TestResult::Unknown => {
            println!("✗ MBC3 tester result unknown (CGB mode)");
        }
    }

    assert_eq!(
        result,
        TestResult::Passed,
        "MBC3 tester failed (CGB mode). This test validates MBC3 ROM bank switching. \
         Check target/traces/ for detailed execution traces."
    );
}

#[test]
fn test_mbc3_tester_dmg() {
    let result = run_mbc3_tester_with_trace(
        ceres_core::Model::Dmg,
        "mbc3-tester-dmg.png",
        "test_mbc3_tester_dmg",
    );

    // If the test fails, traces will be exported to target/traces/
    match &result {
        TestResult::Passed => println!("✓ MBC3 tester passed (DMG mode)"),
        TestResult::Failed(msg) => {
            println!("✗ MBC3 tester failed (DMG mode): {msg}");
            println!("Check target/traces/ for exported trace data");
        }
        TestResult::Timeout => {
            println!("✗ MBC3 tester timed out (DMG mode)");
            println!("Check target/traces/ for exported trace data");
        }
        TestResult::Unknown => {
            println!("✗ MBC3 tester result unknown (DMG mode)");
        }
    }

    assert_eq!(
        result,
        TestResult::Passed,
        "MBC3 tester failed (DMG mode). This test validates MBC3 ROM bank switching. \
         Check target/traces/ for detailed execution traces."
    );
}
