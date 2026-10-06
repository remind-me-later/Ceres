//! Integration tests using the bully ROM.
//!
//! The final screen is compared with the screenshot of the real hardware.
//! The tests that are ignored fail the same way in SameBoy, the reference this
//! emulator follows.

use ceres_core::Model;
use ceres_test_runner::run_screenshot_test;

macro_rules! screenshot_test {
    ($name:ident, $rom:literal, $screenshot:literal, $model:expr) => {
        #[test]
        fn $name() {
            let result = run_screenshot_test($rom, $screenshot, $model, 900);
            assert!(result.is_passed(), "{result:?}");
        }
    };
    ($name:ident, $rom:literal, $screenshot:literal, $model:expr, ignore = $reason:literal) => {
        #[test]
        #[ignore = $reason]
        fn $name() {
            let result = run_screenshot_test($rom, $screenshot, $model, 900);
            assert!(result.is_passed(), "{result:?}");
        }
    };
}

screenshot_test!(
    bully_bully_dmg,
    "bully/bully.gb",
    "bully/bully.png",
    Model::DmgB,
    ignore = "fails the same way in SameBoy"
);
screenshot_test!(
    bully_bully_cgbe,
    "bully/bully.gb",
    "bully/bully.png",
    Model::CgbE,
    ignore = "fails the same way in SameBoy"
);
