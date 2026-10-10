//! Integration tests using rtc3test, which tests the MBC3's real-time clock.
//!
//! The subtest is picked from the ROM's menu with the buttons, then its final
//! screen is compared with the screenshot of the real hardware. The third
//! subtest (sub-second writes) has no reference screenshot.

use ceres_core::{Button, Model};
use ceres_test_runner::{
    run_rom, test_roms_dir,
    test_runner::{RankedScreenshotCheck, TestResult},
};

/// When the menu takes the first button (the boot ROM has finished by then),
/// and how far apart the presses are.
const MENU_FRAME: u32 = 450;
const PRESS_INTERVAL: u32 = 30;

/// Runs the subtest the `menu` buttons select, for `seconds` of emulated time
/// (plus some slack), and compares the screen with `screenshot`.
fn run_subtest(menu: &[Button], seconds: u32, screenshot: &str, model: Model) -> TestResult {
    let buttons: Vec<_> = (MENU_FRAME..)
        .step_by(PRESS_INTERVAL as usize)
        .zip(menu.iter().copied())
        .collect();
    let timeout = MENU_FRAME + (seconds + 4) * 60;
    match RankedScreenshotCheck::new(&test_roms_dir().join("rtc3test").join(screenshot)) {
        Ok(check) => run_rom(
            "rtc3test/rtc3test.gb",
            model,
            timeout,
            Box::new(check),
            &buttons,
        ),
        Err(e) => TestResult::Error(format!("Failed to load the screenshot: {e}")),
    }
}

macro_rules! rtc3test {
    ($name:ident, $menu:expr, $seconds:literal, $screenshot:literal, $model:expr) => {
        #[test]
        fn $name() {
            let result = run_subtest(&$menu, $seconds, $screenshot, $model);
            assert!(result.is_passed(), "{result:?}");
        }
    };
}

rtc3test!(
    rtc3test_basic_dmg,
    [Button::A],
    13,
    "rtc3test-basic-tests-dmg.png",
    Model::DmgB
);
rtc3test!(
    rtc3test_basic_cgb,
    [Button::A],
    13,
    "rtc3test-basic-tests-cgb.png",
    Model::CgbE
);
rtc3test!(
    rtc3test_range_dmg,
    [Button::Down, Button::A],
    8,
    "rtc3test-range-tests-dmg.png",
    Model::DmgB
);
rtc3test!(
    rtc3test_range_cgb,
    [Button::Down, Button::A],
    8,
    "rtc3test-range-tests-cgb.png",
    Model::CgbE
);
