//! Integration tests using the little-things-gb ROMs.
//!
//! The final screen is compared with the screenshots of the real hardware.
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
    little_things_gb_firstwhite_dmg,
    "little-things-gb/firstwhite.gb",
    "little-things-gb/firstwhite-dmg-cgb.png",
    Model::DmgB
);
screenshot_test!(
    little_things_gb_firstwhite_cgbc,
    "little-things-gb/firstwhite.gb",
    "little-things-gb/firstwhite-dmg-cgb.png",
    Model::CgbC
);
screenshot_test!(
    little_things_gb_firstwhite_cgbe,
    "little-things-gb/firstwhite.gb",
    "little-things-gb/firstwhite-dmg-cgb.png",
    Model::CgbE
);
screenshot_test!(
    little_things_gb_tellinglys_cgbc,
    "little-things-gb/tellinglys.gb",
    "little-things-gb/tellinglys-cgb.png",
    Model::CgbC,
    ignore = "fails the same way in SameBoy"
);
screenshot_test!(
    little_things_gb_tellinglys_cgbe,
    "little-things-gb/tellinglys.gb",
    "little-things-gb/tellinglys-cgb.png",
    Model::CgbE,
    ignore = "fails the same way in SameBoy"
);
screenshot_test!(
    little_things_gb_tellinglys_dmg,
    "little-things-gb/tellinglys.gb",
    "little-things-gb/tellinglys-dmg.png",
    Model::DmgB,
    ignore = "fails the same way in SameBoy"
);
