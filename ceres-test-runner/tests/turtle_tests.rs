//! Integration tests using Turtle's TurtleTests: the window's Y trigger, with
//! the window on and off screen.
//!
//! The final screen is compared with the screenshot of the real hardware, the
//! same on the DMG and the CGB.

use ceres_core::Model;
use ceres_test_runner::{checks::TestResult, rom_test, run_ranked_screenshot, timeouts};

fn turtle_test(name: &str, model: Model) -> TestResult {
    run_ranked_screenshot(
        &format!("turtle-tests/{name}/{name}.gb"),
        &format!("turtle-tests/{name}/{name}.png"),
        model,
        timeouts::SCREENSHOT,
    )
}

rom_test!(window_y_trigger_dmg: turtle_test("window_y_trigger", Model::DmgB));
rom_test!(window_y_trigger_cgbe: turtle_test("window_y_trigger", Model::CgbE));
rom_test!(
    window_y_trigger_wx_offscreen_dmg: turtle_test("window_y_trigger_wx_offscreen", Model::DmgB)
);
rom_test!(
    window_y_trigger_wx_offscreen_cgbe: turtle_test("window_y_trigger_wx_offscreen", Model::CgbE)
);
