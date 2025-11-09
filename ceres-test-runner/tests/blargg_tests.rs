//! Integration tests using Blargg's test ROM suite
//!
//! These tests validate CPU instructions, timing behavior, and hardware bugs
//! using Blargg's comprehensive test suite.
//!
//! We only run the combined test suites (e.g., `cpu_instrs.gb`, `mem_timing.gb`)
//! which have reference screenshots for pixel-perfect comparison. Individual
//! tests rely on serial output which is not as reliable.

use ceres_test_runner::{
    expected_screenshot_path, load_test_rom,
    test_runner::{TestConfig, TestResult, TestRunner, timeouts},
};

/// Helper to run a Blargg test ROM with a specific timeout and screenshot comparison
fn run_blargg_test(path: &str, timeout: u32) -> TestResult {
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

#[test]
fn test_blargg_cpu_instrs() {
    let result = run_blargg_test("blargg/cpu_instrs/cpu_instrs.gb", timeouts::CPU_INSTRS);
    assert_eq!(
        result,
        TestResult::Passed,
        "CPU instructions test suite failed"
    );
}

#[test]
fn test_blargg_instr_timing() {
    let result = run_blargg_test(
        "blargg/instr_timing/instr_timing.gb",
        timeouts::INSTR_TIMING,
    );
    assert_eq!(result, TestResult::Passed, "Instruction timing test failed");
}

#[test]
fn test_blargg_mem_timing() {
    let result = run_blargg_test("blargg/mem_timing/mem_timing.gb", timeouts::MEM_TIMING);
    assert_eq!(
        result,
        TestResult::Passed,
        "Memory timing test suite failed"
    );
}

#[test]
fn test_blargg_mem_timing_2() {
    let result = run_blargg_test("blargg/mem_timing-2/mem_timing.gb", timeouts::MEM_TIMING_2);
    assert_eq!(
        result,
        TestResult::Passed,
        "Memory timing 2 test suite failed"
    );
}

#[test]
fn test_blargg_interrupt_time() {
    let result = run_blargg_test(
        "blargg/interrupt_time/interrupt_time.gb",
        timeouts::INTERRUPT_TIME,
    );
    assert_eq!(result, TestResult::Passed, "Interrupt timing test failed");
}

#[test]
fn test_blargg_halt_bug() {
    let result = run_blargg_test("blargg/halt_bug.gb", timeouts::HALT_BUG);
    assert_eq!(result, TestResult::Passed, "Halt bug test failed");
}
