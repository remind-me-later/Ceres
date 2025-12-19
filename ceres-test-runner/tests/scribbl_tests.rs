//! Integration tests using the Scribbltests suite
//!
//! Scribbltests are a collection of self-written test ROMs by Hacktix that test
//! various PPU and interrupt behaviors. These tests use screenshot comparison
//! to validate correct emulation behavior.
//!
//! ## Test Categories
//!
//! - `lycscx`: Tests SCX register and LY=LYC STAT interrupts
//! - `lycscy`: Tests SCY register and LY=LYC STAT interrupts
//! - `palettely`: Tests BGP register and STAT/VBlank interrupts
//! - `scxly`: Tests SCX and `HBlank` STAT interrupts
//! - `statcount-auto`: Automated PPU timing test for STAT register
//!
//! ## Excluded Tests
//!
//! The following tests are excluded:
//! - `fairylake`: Demo/WIP, no static expected output
//! - `winpos`: Interactive tool requiring joypad input
//! - `statcount`: Manual version requiring joypad input
//!
//! ## Current Status
//!
//! Total Scribbl tests: 10 (5 tests × 2 models)
//! - **7 tests pass** (70% pass rate)
//! - **3 tests fail** and are marked with `#[ignore]`
//!
//! Passing tests:
//! - lycscx (DMG, CGB)
//! - lycscy (DMG, CGB)
//! - palettely (DMG, CGB)
//! - scxly (DMG only)
//!
//! Failing tests need improvements in:
//! - scxly CGB mode timing/color behavior
//! - statcount-auto requires precise PPU timing

use ceres_core::Model;
use ceres_test_runner::{
    load_test_rom, test_roms_dir,
    test_runner::{ScreenshotCheck, TestConfig, TestResult, TestRunner},
};

/// Timeout for Scribbltests (most complete quickly, but statcount-auto needs more time)
const SCRIBBL_TIMEOUT: u32 = 1200;

/// Extended timeout for statcount-auto which iterates through many NOP counts
const STATCOUNT_AUTO_TIMEOUT: u32 = 3600;

/// Helper function to run a Scribbltest with screenshot comparison
fn run_scribbl_test(
    rom_path: &str,
    screenshot_path: &str,
    model: Model,
    timeout: u32,
) -> TestResult {
    let rom = match load_test_rom(rom_path) {
        Ok(rom) => rom,
        Err(e) => return TestResult::Error(format!("Failed to load test ROM: {e}")),
    };

    let screenshot_path = test_roms_dir().join(screenshot_path);

    let config = TestConfig {
        model,
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

// =============================================================================
// LYCSCX Tests - SCX and LY=LYC STAT interrupts
// =============================================================================

#[test]
fn test_scribbl_lycscx_dmg() {
    let result = run_scribbl_test(
        "scribbltests/lycscx/lycscx.gb",
        "scribbltests/lycscx/lycscx-cgb-dmg.png",
        Model::DmgB,
        SCRIBBL_TIMEOUT,
    );
    assert!(result.is_passed(), "lycscx test failed (DMG)");
}

#[test]
fn test_scribbl_lycscx_cgb() {
    let result = run_scribbl_test(
        "scribbltests/lycscx/lycscx.gb",
        "scribbltests/lycscx/lycscx-cgb-dmg.png",
        Model::CgbE,
        SCRIBBL_TIMEOUT,
    );
    assert!(result.is_passed(), "lycscx test failed (CGB)");
}

// =============================================================================
// LYCSCY Tests - SCY and LY=LYC STAT interrupts
// =============================================================================

#[test]
fn test_scribbl_lycscy_dmg() {
    let result = run_scribbl_test(
        "scribbltests/lycscy/lycscy.gb",
        "scribbltests/lycscy/lycscy-cgb-dmg.png",
        Model::DmgB,
        SCRIBBL_TIMEOUT,
    );
    assert!(result.is_passed(), "lycscy test failed (DMG)");
}

#[test]
fn test_scribbl_lycscy_cgb() {
    let result = run_scribbl_test(
        "scribbltests/lycscy/lycscy.gb",
        "scribbltests/lycscy/lycscy-cgb-dmg.png",
        Model::CgbE,
        SCRIBBL_TIMEOUT,
    );
    assert!(result.is_passed(), "lycscy test failed (CGB)");
}

// =============================================================================
// PaletteLY Tests - BGP register and STAT/VBlank interrupts
// =============================================================================

#[test]
fn test_scribbl_palettely_dmg() {
    let result = run_scribbl_test(
        "scribbltests/palettely/palettely.gb",
        "scribbltests/palettely/palettely-dmg.png",
        Model::DmgB,
        SCRIBBL_TIMEOUT,
    );
    assert!(result.is_passed(), "palettely test failed (DMG)");
}

#[test]
fn test_scribbl_palettely_cgb() {
    let result = run_scribbl_test(
        "scribbltests/palettely/palettely.gb",
        "scribbltests/palettely/palettely-cgb.png",
        Model::CgbE,
        SCRIBBL_TIMEOUT,
    );
    assert!(result.is_passed(), "palettely test failed (CGB)");
}

// =============================================================================
// SCXLY Tests - SCX and HBlank STAT interrupts
// =============================================================================

#[test]
fn test_scribbl_scxly_dmg() {
    let result = run_scribbl_test(
        "scribbltests/scxly/scxly.gb",
        "scribbltests/scxly/scxly-dmg.png",
        Model::DmgB,
        SCRIBBL_TIMEOUT,
    );
    assert!(result.is_passed(), "scxly test failed (DMG)");
}

#[test]
#[ignore] // TODO: Enable when passing - CGB mode has different timing/color behavior
fn test_scribbl_scxly_cgb() {
    let result = run_scribbl_test(
        "scribbltests/scxly/scxly.gb",
        "scribbltests/scxly/scxly-cgb.png",
        Model::CgbE,
        SCRIBBL_TIMEOUT,
    );
    assert!(result.is_passed(), "scxly test failed (CGB)");
}

// =============================================================================
// STATcount-auto Tests - Automated PPU timing validation
// =============================================================================

#[test]
#[ignore] // TODO: Enable when passing - requires precise PPU timing
fn test_scribbl_statcount_auto_dmg() {
    let result = run_scribbl_test(
        "scribbltests/statcount/statcount-auto.gb",
        "scribbltests/statcount/statcount_auto-cgb-dmg.png",
        Model::DmgB,
        STATCOUNT_AUTO_TIMEOUT,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "statcount-auto test failed (DMG)"
    );
}

#[test]
#[ignore] // TODO: Enable when passing - requires precise PPU timing
fn test_scribbl_statcount_auto_cgb() {
    let result = run_scribbl_test(
        "scribbltests/statcount/statcount-auto.gb",
        "scribbltests/statcount/statcount_auto-cgb-dmg.png",
        Model::CgbE,
        STATCOUNT_AUTO_TIMEOUT,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "statcount-auto test failed (CGB)"
    );
}
