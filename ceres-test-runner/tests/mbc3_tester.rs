//! Integration tests using HyperHacker's mbc3-tester, which checks that each
//! of the 128 banks of its MBC3 ROM can be mapped.
//!
//! The final screen is compared with the screenshot of the real hardware.

use ceres_core::Model;
use ceres_test_runner::{checks::TestResult, rom_test, run_ranked_screenshot, timeouts};

fn mbc3_tester(screenshot: &str, model: Model) -> TestResult {
    run_ranked_screenshot(
        "mbc3-tester/mbc3-tester.gb",
        &format!("mbc3-tester/{screenshot}"),
        model,
        timeouts::MBC3_TESTER,
    )
}

rom_test!(mbc3_tester_dmg: mbc3_tester("mbc3-tester-dmg.png", Model::DmgB));
rom_test!(mbc3_tester_cgbc: mbc3_tester("mbc3-tester-cgb.png", Model::CgbC));
rom_test!(mbc3_tester_cgbe: mbc3_tester("mbc3-tester-cgb.png", Model::CgbE));
