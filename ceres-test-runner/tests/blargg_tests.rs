//! Integration tests using Blargg's test ROM suite
//!
//! These tests validate CPU instructions, timing behavior, and hardware bugs
//! using Blargg's comprehensive test suite.
//!
//! We only run the combined test suites (e.g., `cpu_instrs.gb`, `mem_timing.gb`)
//! which have reference screenshots for pixel-perfect comparison.

use ceres_test_runner::{
    expected_screenshot_path, load_test_rom,
    test_runner::{ScreenshotCheck, TestConfig, TestResult, TestRunner, timeouts},
};

/// Helper to run a Blargg test ROM with a specific timeout and screenshot comparison
fn run_blargg_test(path: &str, timeout: u32) -> TestResult {
    let rom = match load_test_rom(path) {
        Ok(rom) => rom,
        Err(e) => return TestResult::Error(format!("Failed to load test ROM: {e}")),
    };

    let screenshot_path = match expected_screenshot_path(path, ceres_core::Model::CgbE) {
        Some(path) => path,
        None => {
            return TestResult::Error(format!(
                "No expected screenshot found for {path} with model CgbE"
            ));
        }
    };

    let config = TestConfig {
        timeout_frames: timeout,
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
fn test_blargg_cpu_instrs() {
    let result = run_blargg_test("blargg/cpu_instrs/cpu_instrs.gb", timeouts::CPU_INSTRS);
    assert!(result.is_passed(), "CPU instructions test suite failed");
}

#[test]
fn test_blargg_instr_timing() {
    let result = run_blargg_test(
        "blargg/instr_timing/instr_timing.gb",
        timeouts::INSTR_TIMING,
    );
    assert!(result.is_passed(), "Instruction timing test failed");
}

#[test]
fn test_blargg_mem_timing() {
    let result = run_blargg_test("blargg/mem_timing/mem_timing.gb", timeouts::MEM_TIMING);
    assert!(result.is_passed(), "Memory timing test suite failed");
}

#[test]
fn test_blargg_mem_timing_2() {
    let result = run_blargg_test("blargg/mem_timing-2/mem_timing.gb", timeouts::MEM_TIMING_2);
    assert!(result.is_passed(), "Memory timing 2 test suite failed");
}

#[test]
fn test_blargg_interrupt_time() {
    let result = run_blargg_test(
        "blargg/interrupt_time/interrupt_time.gb",
        timeouts::INTERRUPT_TIME,
    );
    assert!(result.is_passed(), "Interrupt timing test failed");
}

#[test]
fn test_blargg_halt_bug() {
    let result = run_blargg_test("blargg/halt_bug.gb", timeouts::HALT_BUG);
    assert!(result.is_passed(), "Halt bug test failed");
}
