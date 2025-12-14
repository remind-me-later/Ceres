//! Integration tests using the SameSuite
//!
//! The SameSuite is a comprehensive collection of hardware-validated Game Boy test ROMs
//! compiled with RGBDS. These tests validate various hardware behaviors with high accuracy,
//! particularly focusing on edge cases and timing-critical operations.
//!
//! ## Test Protocol
//!
//! SameSuite tests use the same protocol as Mooneye tests to signal pass/fail:
//! - **Pass**: CPU registers contain Fibonacci numbers (B=3, C=5, D=8, E=13, H=21, L=34)
//! - **Fail**: Registers don't match the expected pattern
//! - **Exit**: Tests execute the `ld b, b` instruction (opcode 0x40) when finished
//!
//! ## Test Organization
//!
//! Tests are organized by category:
//! - `dma/`: DMA transfer tests including GDMA and HDMA
//! - `interrupt/`: Interrupt timing and behavior tests
//!
//! ## Hardware Compatibility
//!
//! Some SameSuite APU tests only work on CPU CGB E revision. Compatibility information
//! for non-APU tests is not fully documented, so tests run on CGB by default.
//!
//! ## Current Status
//!
//! Total SameSuite tests: 5
//! - **3 tests pass** (60% pass rate)
//! - **2 tests fail** and are marked with `#[ignore]`
//!
//! Passing tests:
//! - DMA: gbc_dma_cont, gdma_addr_mask
//! - Interrupt: ei_delay_halt
//!
//! Failing tests need improvements in:
//! - HDMA mode 0 (General Purpose DMA) behavior
//! - HDMA behavior when LCD is off

use ceres_core::Model;
use ceres_test_runner::{
    load_test_rom,
    test_runner::{TestConfig, TestResult, TestRunner, timeouts},
};

/// Helper function to run a SameSuite test
fn run_samesuite_test(path: &str, model: Model) -> TestResult {
    let rom = match load_test_rom(path) {
        Ok(rom) => rom,
        Err(e) => return TestResult::Failed(format!("Failed to load test ROM: {e}")),
    };

    let config = TestConfig {
        model,
        timeout_frames: timeouts::MOONEYE_ACCEPTANCE, // Use same timeout as Mooneye
        use_mooneye_validation: true,                 // SameSuite uses same Fibonacci validation
        capture_serial: false,
        ..TestConfig::default()
    };

    let mut runner = match TestRunner::new(rom, config) {
        Ok(runner) => runner,
        Err(e) => return TestResult::Failed(format!("Failed to create test runner: {e}")),
    };

    runner.run()
}

// =============================================================================
// DMA Tests
// =============================================================================

#[test]
fn test_samesuite_gbc_dma_cont() {
    let result = run_samesuite_test("same-suite/dma/gbc_dma_cont.gb", Model::CgbE);
    assert_eq!(result, TestResult::Passed, "dma/gbc_dma_cont test failed");
}

#[test]
fn test_samesuite_gdma_addr_mask() {
    let result = run_samesuite_test("same-suite/dma/gdma_addr_mask.gb", Model::CgbE);
    assert_eq!(result, TestResult::Passed, "dma/gdma_addr_mask test failed");
}

#[test]
#[ignore] // TODO: Enable when passing - HDMA with LCD off behavior needs fixing
fn test_samesuite_hdma_lcd_off() {
    let result = run_samesuite_test("same-suite/dma/hdma_lcd_off.gb", Model::CgbE);
    assert_eq!(result, TestResult::Passed, "dma/hdma_lcd_off test failed");
}

#[test]
#[ignore] // TODO: Enable when passing - HDMA mode 0 (General Purpose DMA) needs fixing
fn test_samesuite_hdma_mode0() {
    let result = run_samesuite_test("same-suite/dma/hdma_mode0.gb", Model::CgbE);
    assert_eq!(result, TestResult::Passed, "dma/hdma_mode0 test failed");
}

// =============================================================================
// Interrupt Tests
// =============================================================================

#[test]
fn test_samesuite_ei_delay_halt() {
    let result = run_samesuite_test("same-suite/interrupt/ei_delay_halt.gb", Model::CgbE);
    assert_eq!(
        result,
        TestResult::Passed,
        "interrupt/ei_delay_halt test failed"
    );
}
