//! Integration tests using Hacktix's scribbltests: LY=LYC and HBlank STAT
//! interrupts changing SCX, SCY and BGP mid-frame, and the STAT mode timing
//! (statcount-auto).
//!
//! The final screen is compared with the screenshot of the hardware the
//! author verified them on (an MGB and a CPU CGB D). fairylake (an
//! unfinished demo) and winpos (interactive) have no reference screenshot.

use ceres_core::Model;
use ceres_test_runner::{checks::TestResult, rom_test, run_ranked_screenshot, timeouts};

fn scribbltest(rom: &str, screenshot: &str, model: Model) -> TestResult {
    run_ranked_screenshot(
        &format!("scribbltests/{rom}"),
        &format!("scribbltests/{screenshot}"),
        model,
        timeouts::SCREENSHOT,
    )
}

rom_test!(lycscx_dmg: scribbltest("lycscx/lycscx.gb", "lycscx/lycscx-cgb-dmg.png", Model::Mgb));
rom_test!(lycscx_cgb: scribbltest("lycscx/lycscx.gb", "lycscx/lycscx-cgb-dmg.png", Model::CgbD));
rom_test!(lycscy_dmg: scribbltest("lycscy/lycscy.gb", "lycscy/lycscy-cgb-dmg.png", Model::Mgb));
rom_test!(lycscy_cgb: scribbltest("lycscy/lycscy.gb", "lycscy/lycscy-cgb-dmg.png", Model::CgbD));
rom_test!(
    palettely_dmg: scribbltest("palettely/palettely.gb", "palettely/palettely-dmg.png", Model::Mgb)
);
rom_test!(
    palettely_cgb: scribbltest("palettely/palettely.gb", "palettely/palettely-cgb.png", Model::CgbD)
);
rom_test!(scxly_dmg: scribbltest("scxly/scxly.gb", "scxly/scxly-dmg.png", Model::Mgb));
rom_test!(scxly_cgb: scribbltest("scxly/scxly.gb", "scxly/scxly-cgb.png", Model::CgbD));
rom_test!(
    statcount_auto_dmg: scribbltest("statcount/statcount-auto.gb", "statcount/statcount_auto-cgb-dmg.png", Model::Mgb)
);
rom_test!(
    statcount_auto_cgb: scribbltest("statcount/statcount-auto.gb", "statcount/statcount_auto-cgb-dmg.png", Model::CgbD)
);
