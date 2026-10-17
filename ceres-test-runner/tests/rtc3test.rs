//! Integration tests using rtc3test, which tests the MBC3's real-time clock.
//!
//! Each subtest is picked from the ROM's menu with the buttons, then its final
//! screen is compared with the screenshot of the real hardware.

use ceres_core::{Button, Model};
use ceres_test_runner::{
    Run, checks::RankedScreenshotCheck, checks::TestResult, rom_test, test_roms_dir,
};

/// When the menu takes the first button (the boot ROM has finished by then),
/// and how far apart the presses are.
const MENU_FRAME: u32 = 450;
const PRESS_INTERVAL: u32 = 30;

/// Runs the subtest the `menu` buttons select, for `seconds` of emulated time
/// (plus some slack), and compares the screen with `rtc3test-<subtest>-<hw>.png`.
fn subtest(menu: &[Button], seconds: u32, subtest: &str, model: Model) -> TestResult {
    let buttons: Vec<_> = (MENU_FRAME..)
        .step_by(PRESS_INTERVAL as usize)
        .zip(menu.iter().copied())
        .collect();
    let hardware = if model.is_cgb_hardware() {
        "cgb"
    } else {
        "dmg"
    };
    let screenshot = test_roms_dir()
        .join("rtc3test")
        .join(format!("rtc3test-{subtest}-{hardware}.png"));
    match RankedScreenshotCheck::new(&screenshot) {
        Ok(check) => Run::new("rtc3test/rtc3test.gb", model)
            .timeout(MENU_FRAME + (seconds + 4) * 60)
            .press(&buttons)
            .check(check),
        Err(e) => TestResult::Error(format!("Failed to load the screenshot: {e}")),
    }
}

const BASIC: &[Button] = &[Button::A];
const RANGE: &[Button] = &[Button::Down, Button::A];
const SUB_SECOND: &[Button] = &[Button::Down, Button::Down, Button::A];

rom_test!(rtc3test_basic_dmg: subtest(BASIC, 13, "basic-tests", Model::DmgB));
rom_test!(rtc3test_basic_cgbc: subtest(BASIC, 13, "basic-tests", Model::CgbC));
rom_test!(rtc3test_basic_cgb: subtest(BASIC, 13, "basic-tests", Model::CgbE));
rom_test!(rtc3test_range_dmg: subtest(RANGE, 8, "range-tests", Model::DmgB));
rom_test!(rtc3test_range_cgbc: subtest(RANGE, 8, "range-tests", Model::CgbC));
rom_test!(rtc3test_range_cgb: subtest(RANGE, 8, "range-tests", Model::CgbE));
rom_test!(rtc3test_sub_second_dmg: subtest(SUB_SECOND, 30, "sub-second-writes", Model::DmgB));
rom_test!(rtc3test_sub_second_cgbc: subtest(SUB_SECOND, 30, "sub-second-writes", Model::CgbC));
rom_test!(rtc3test_sub_second_cgb: subtest(SUB_SECOND, 30, "sub-second-writes", Model::CgbE));
