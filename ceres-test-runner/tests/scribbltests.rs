//! Integration tests using Hacktix's scribbltests: LY=LYC and HBlank STAT
//! interrupts changing SCX, SCY and BGP mid-frame, and the STAT mode timing
//! (statcount-auto).
//!
//! The final screen is compared with the screenshot of the hardware the
//! author verified them on (an MGB and a CPU CGB D). fairylake (an
//! unfinished demo) and winpos (interactive) have no reference screenshot.

use ceres_core::Model;
use ceres_test_runner::{
    run_rom, test_roms_dir,
    test_runner::{RankedScreenshotCheck, TestResult},
};

/// The boot ROM, then 10 frames for most tests and 270 for statcount-auto,
/// with slack.
const TIMEOUT_FRAMES: u32 = 900;

fn run_scribbltest(rom: &str, screenshot: &str, model: Model) -> TestResult {
    let dir = test_roms_dir().join("scribbltests");
    match RankedScreenshotCheck::new(&dir.join(screenshot)) {
        Ok(check) => run_rom(
            &format!("scribbltests/{rom}"),
            model,
            TIMEOUT_FRAMES,
            Box::new(check),
            &[],
        ),
        Err(e) => TestResult::Error(format!("Failed to load the screenshot: {e}")),
    }
}

macro_rules! scribbltest {
    ($name:ident, $rom:literal, $screenshot:literal, $model:expr) => {
        #[test]
        fn $name() {
            let result = run_scribbltest($rom, $screenshot, $model);
            assert!(result.is_passed(), "{result:?}");
        }
    };
}

scribbltest!(
    lycscx_dmg,
    "lycscx/lycscx.gb",
    "lycscx/lycscx-cgb-dmg.png",
    Model::Mgb
);
scribbltest!(
    lycscx_cgb,
    "lycscx/lycscx.gb",
    "lycscx/lycscx-cgb-dmg.png",
    Model::CgbD
);
scribbltest!(
    lycscy_dmg,
    "lycscy/lycscy.gb",
    "lycscy/lycscy-cgb-dmg.png",
    Model::Mgb
);
scribbltest!(
    lycscy_cgb,
    "lycscy/lycscy.gb",
    "lycscy/lycscy-cgb-dmg.png",
    Model::CgbD
);
scribbltest!(
    palettely_dmg,
    "palettely/palettely.gb",
    "palettely/palettely-dmg.png",
    Model::Mgb
);
scribbltest!(
    palettely_cgb,
    "palettely/palettely.gb",
    "palettely/palettely-cgb.png",
    Model::CgbD
);
scribbltest!(
    scxly_dmg,
    "scxly/scxly.gb",
    "scxly/scxly-dmg.png",
    Model::Mgb
);
scribbltest!(
    scxly_cgb,
    "scxly/scxly.gb",
    "scxly/scxly-cgb.png",
    Model::CgbD
);
scribbltest!(
    statcount_auto_dmg,
    "statcount/statcount-auto.gb",
    "statcount/statcount_auto-cgb-dmg.png",
    Model::Mgb
);
scribbltest!(
    statcount_auto_cgb,
    "statcount/statcount-auto.gb",
    "statcount/statcount_auto-cgb-dmg.png",
    Model::CgbD
);
