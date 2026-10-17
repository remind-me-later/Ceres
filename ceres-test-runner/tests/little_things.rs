//! Integration tests using the little-things-gb ROMs.
//!
//! The final screen is compared with the screenshots of the real hardware.
//! The tests that are ignored fail the same way in SameBoy, the reference this
//! emulator follows.

use ceres_core::Model;
use ceres_test_runner::{checks::TestResult, rom_test, run_ranked_screenshot, timeouts};

fn screenshot(rom: &str, screenshot: &str, model: Model) -> TestResult {
    run_ranked_screenshot(rom, screenshot, model, timeouts::SCREENSHOT)
}

rom_test!(
    little_things_gb_firstwhite_dmg: screenshot("little-things-gb/firstwhite.gb", "little-things-gb/firstwhite-dmg-cgb.png", Model::DmgB)
);
rom_test!(
    little_things_gb_firstwhite_cgbc: screenshot("little-things-gb/firstwhite.gb", "little-things-gb/firstwhite-dmg-cgb.png", Model::CgbC)
);
rom_test!(
    little_things_gb_firstwhite_cgbe: screenshot("little-things-gb/firstwhite.gb", "little-things-gb/firstwhite-dmg-cgb.png", Model::CgbE)
);
rom_test!(
    #[ignore = "fails the same way in SameBoy"]
    little_things_gb_tellinglys_cgbc: screenshot("little-things-gb/tellinglys.gb", "little-things-gb/tellinglys-cgb.png", Model::CgbC)
);
rom_test!(
    #[ignore = "fails the same way in SameBoy"]
    little_things_gb_tellinglys_cgbe: screenshot("little-things-gb/tellinglys.gb", "little-things-gb/tellinglys-cgb.png", Model::CgbE)
);
rom_test!(
    #[ignore = "fails the same way in SameBoy"]
    little_things_gb_tellinglys_dmg: screenshot("little-things-gb/tellinglys.gb", "little-things-gb/tellinglys-dmg.png", Model::DmgB)
);
