//! Integration tests using the bully ROM.
//!
//! The final screen is compared with the screenshot of the real hardware.
//! The tests that are ignored fail the same way in SameBoy, the reference this
//! emulator follows.

use ceres_core::Model;
use ceres_test_runner::{checks::TestResult, rom_test, run_ranked_screenshot, timeouts};

fn screenshot(rom: &str, screenshot: &str, model: Model) -> TestResult {
    run_ranked_screenshot(rom, screenshot, model, timeouts::SCREENSHOT)
}

rom_test!(bully_bully_dmg: screenshot("bully/bully.gb", "bully/bully.png", Model::DmgB));
rom_test!(
    #[ignore = "fails the same way in SameBoy"]
    bully_bully_cgbe: screenshot("bully/bully.gb", "bully/bully.png", Model::CgbE)
);
