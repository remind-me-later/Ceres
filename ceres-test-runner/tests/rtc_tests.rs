//! Integration tests for MBC3 Real-Time Clock (RTC) functionality
//!
//! These tests validate RTC behavior using the rtc3test ROM, which requires
//! button input to select and run specific test suites. Only the "basic tests"
//! and "range tests" subtests are included, as the "sub-second writes" test
//! currently fails due to incomplete RTC implementation.
//!
//! Note: Tests account for the ~4 second CGB boot intro animation before
//! button presses are sent. Timeouts include intro duration + test duration.

use ceres_test_runner::{
    load_test_rom, test_roms_dir,
    test_runner::{ButtonAction, ButtonEvent, TestConfig, TestResult, TestRunner, timeouts},
};

/// Helper to run rtc3test with scheduled button presses and screenshot comparison
fn run_rtc3test(
    button_events: Vec<ButtonEvent>,
    timeout: u32,
    screenshot_name: &str,
) -> TestResult {
    let rom = match load_test_rom("rtc3test/rtc3test.gb") {
        Ok(rom) => rom,
        Err(e) => return TestResult::Failed(format!("Failed to load test ROM: {e}")),
    };

    let config = TestConfig {
        timeout_frames: timeout,
        expected_screenshot: Some(test_roms_dir().join(format!("rtc3test/{screenshot_name}"))),
        button_events,
        ..TestConfig::default()
    };

    let mut runner = match TestRunner::new(rom, config) {
        Ok(runner) => runner,
        Err(e) => return TestResult::Failed(format!("Failed to create test runner: {e}")),
    };

    runner.run()
}

#[test]
fn test_rtc3test_basic_cgb() {
    // Press A at frame 240 (~4 seconds) to select "basic tests" after CGB intro
    let button_events = vec![
        ButtonEvent {
            frame: 240,
            button: ceres_core::Button::A,
            action: ButtonAction::Press,
        },
        ButtonEvent {
            frame: 250,
            button: ceres_core::Button::A,
            action: ButtonAction::Release,
        },
    ];

    let result = run_rtc3test(
        button_events,
        timeouts::RTC3TEST_BASIC,
        "rtc3test-basic-tests-cgb.png",
    );

    assert_eq!(
        result,
        TestResult::Passed,
        "RTC basic tests failed. This test validates: RTC enable/disable, tick timing, \
         register writes, seconds increment, rollovers, overflow flag handling, and \
         overflow stickiness."
    );
}

#[test]
fn test_rtc3test_range_cgb() {
    // Press Down at frame 240 to navigate menu, release it, then press A at frame 270 to select "range tests"
    let button_events = vec![
        ButtonEvent {
            frame: 240,
            button: ceres_core::Button::Down,
            action: ButtonAction::Press,
        },
        ButtonEvent {
            frame: 250,
            button: ceres_core::Button::Down,
            action: ButtonAction::Release,
        },
        ButtonEvent {
            frame: 270,
            button: ceres_core::Button::A,
            action: ButtonAction::Press,
        },
        ButtonEvent {
            frame: 280,
            button: ceres_core::Button::A,
            action: ButtonAction::Release,
        },
    ];

    let result = run_rtc3test(
        button_events,
        timeouts::RTC3TEST_RANGE,
        "rtc3test-range-tests-cgb.png",
    );

    assert_eq!(
        result,
        TestResult::Passed,
        "RTC range tests failed. This test validates: all bits clear, all valid bits set, \
         valid bits mask, invalid value tick handling, invalid rollovers, high minutes \
         rollover, and high hours rollover."
    );
}
