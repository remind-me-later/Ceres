//! Integration tests using the TurtleTests suite
//!
//! [TurtleTests](https://github.com/Powerlated/TurtleTests) is a suite of test
//! ROMs for the Game Boy and Game Boy Color.
//!
//! ## Test Categories
//!
//! - `window_y_trigger`: Tests window Y triggering behavior
//! - `window_y_trigger_wx_offscreen`: Tests window Y triggering when WX is offscreen
//!
//! ## Status
//!
//! These tests validate window triggering logic, particularly edge cases involving
//! WX positioning relative to the screen bounds.

use ceres_core::Model;
use ceres_test_runner::{
    load_test_rom, test_roms_dir,
    test_runner::{ScreenshotCheck, TestConfig, TestResult, TestRunner},
};

const TURTLE_TIMEOUT: u32 = 1200;

fn run_turtle_test(
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
// Window Y Trigger Tests
// =============================================================================

#[test]
fn test_turtle_window_y_trigger_dmg() {
    let result = run_turtle_test(
        "turtle-tests/window_y_trigger/window_y_trigger.gb",
        "turtle-tests/window_y_trigger/window_y_trigger.png",
        Model::DmgB,
        TURTLE_TIMEOUT,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "window_y_trigger test failed (DMG)"
    );
}

#[test]
fn test_turtle_window_y_trigger_cgb() {
    let result = run_turtle_test(
        "turtle-tests/window_y_trigger/window_y_trigger.gb",
        "turtle-tests/window_y_trigger/window_y_trigger.png",
        Model::CgbE,
        TURTLE_TIMEOUT,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "window_y_trigger test failed (CGB)"
    );
}

// =============================================================================
// Window Y Trigger WX Offscreen Tests
// =============================================================================

#[test]
fn test_turtle_window_y_trigger_wx_offscreen_dmg() {
    let result = run_turtle_test(
        "turtle-tests/window_y_trigger_wx_offscreen/window_y_trigger_wx_offscreen.gb",
        "turtle-tests/window_y_trigger_wx_offscreen/window_y_trigger_wx_offscreen.png",
        Model::DmgB,
        TURTLE_TIMEOUT,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "window_y_trigger_wx_offscreen test failed (DMG)"
    );
}

#[test]
fn test_turtle_window_y_trigger_wx_offscreen_cgb() {
    let result = run_turtle_test(
        "turtle-tests/window_y_trigger_wx_offscreen/window_y_trigger_wx_offscreen.gb",
        "turtle-tests/window_y_trigger_wx_offscreen/window_y_trigger_wx_offscreen.png",
        Model::CgbE,
        TURTLE_TIMEOUT,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "window_y_trigger_wx_offscreen test failed (CGB)"
    );
}
