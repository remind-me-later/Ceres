//! Integration tests using the strikethrough ROM, which tests some unusual
//! OAM DMA behaviour.
//!
//! The final screen is compared with the screenshot of the real hardware.
//! The quirk is not emulated yet: real hardware garbles the sprites of the
//! "i"s ("Everyth+ng is OK!"), Ceres shows them intact.

use ceres_core::Model;
use ceres_test_runner::{checks::TestResult, rom_test, run_ranked_screenshot, timeouts};

fn strikethrough(screenshot: &str, model: Model) -> TestResult {
    run_ranked_screenshot(
        "strikethrough/strikethrough.gb",
        &format!("strikethrough/{screenshot}"),
        model,
        timeouts::SCREENSHOT,
    )
}

rom_test!(
    #[ignore = "the OAM DMA quirk is not emulated"]
    strikethrough_dmg: strikethrough("strikethrough-dmg.png", Model::DmgB)
);
rom_test!(
    #[ignore = "the OAM DMA quirk is not emulated"]
    strikethrough_cgbc: strikethrough("strikethrough-cgb.png", Model::CgbC)
);
rom_test!(
    #[ignore = "the OAM DMA quirk is not emulated"]
    strikethrough_cgbe: strikethrough("strikethrough-cgb.png", Model::CgbE)
);
