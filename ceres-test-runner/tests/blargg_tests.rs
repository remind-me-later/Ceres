//! Integration tests using Blargg test ROMs
//!
//! These tests validate the CPU instruction implementation against
//! Blargg's comprehensive test suite.
//!
//! We only run the combined test suites (e.g., `cpu_instrs.gb`, `mem_timing.gb`)
//! which have reference screenshots for pixel-perfect comparison. Individual
//! tests rely on serial output which is not as reliable.

use ceres_test_runner::{
    expected_screenshot_path, load_test_rom,
    test_runner::{TestConfig, TestResult, TestRunner, timeouts},
};

/// Helper to run a test ROM with a specific timeout and optional screenshot comparison
fn run_test_rom(path: &str, timeout: u32) -> TestResult {
    let rom = match load_test_rom(path) {
        Ok(rom) => rom,
        Err(e) => return TestResult::Failed(format!("Failed to load test ROM: {e}")),
    };

    let config = TestConfig {
        timeout_frames: timeout,
        expected_screenshot: expected_screenshot_path(path, ceres_core::Model::Cgb),
        ..TestConfig::default()
    };

    let mut runner = match TestRunner::new(rom, config) {
        Ok(runner) => runner,
        Err(e) => return TestResult::Failed(format!("Failed to create test runner: {e}")),
    };

    runner.run()
}

// ============================================================================
// CPU Instructions Tests
// ============================================================================

/// Run the complete CPU instructions test suite.
/// This runs all 11 CPU tests in one ROM and validates against a screenshot.
#[test]
fn test_blargg_cpu_instrs() {
    let result = run_test_rom("blargg/cpu_instrs/cpu_instrs.gb", timeouts::CPU_INSTRS);
    assert_eq!(
        result,
        TestResult::Passed,
        "CPU instructions test suite failed"
    );
}

// ============================================================================
// Instruction Timing Tests
// ============================================================================

/// Run the instruction timing test.
/// This validates that instructions take the correct number of cycles.
#[test]
fn test_blargg_instr_timing() {
    let result = run_test_rom(
        "blargg/instr_timing/instr_timing.gb",
        timeouts::INSTR_TIMING,
    );
    assert_eq!(result, TestResult::Passed, "Instruction timing test failed");
}

// ============================================================================
// Memory Timing Tests
// ============================================================================

/// Run the complete memory timing test suite.
#[test]
fn test_blargg_mem_timing() {
    let result = run_test_rom("blargg/mem_timing/mem_timing.gb", timeouts::MEM_TIMING);
    assert_eq!(
        result,
        TestResult::Passed,
        "Memory timing test suite failed"
    );
}

// ============================================================================
// Memory Timing 2 Tests
// ============================================================================
// Note: These tests currently timeout - they expose emulation bugs
// that need to be fixed. Run with --ignored to test them.

/// Run the complete memory timing 2 test suite.
#[test]
fn test_blargg_mem_timing_2() {
    let result = run_test_rom("blargg/mem_timing-2/mem_timing.gb", timeouts::MEM_TIMING_2);
    assert_eq!(
        result,
        TestResult::Passed,
        "Memory timing 2 test suite failed"
    );
}

// ============================================================================
// Interrupt Timing Tests
// ============================================================================
// Note: This test currently times out - it exposes emulation bugs
// that need to be fixed. Run with --ignored to test it.

/// Run the interrupt timing test.
/// This validates that interrupts occur at the correct time.
#[test]
fn test_blargg_interrupt_time() {
    let result = run_test_rom(
        "blargg/interrupt_time/interrupt_time.gb",
        timeouts::INTERRUPT_TIME,
    );
    assert_eq!(result, TestResult::Passed, "Interrupt timing test failed");
}
