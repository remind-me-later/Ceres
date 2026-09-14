//! Integration tests using the Mealybug Tearoom Tests ROM suite
//! PPU related tests
//! Source: <https://github.com/mealybug/mealybug-tearoom-tests>

use ceres_core::Model;
use ceres_test_runner::{
    expected_screenshot_path, load_test_rom,
    test_runner::{ScreenshotCheck, TestConfig, TestResult, TestRunner},
};

const MEALYBUG_TIMEOUT_FRAMES: u32 = 500; // Typically 3 seconds for Mealybug tests

/// Helper to run a Mealybug PPU test
fn run_mealybug_ppu_test(rom_name: &str, model: Model) -> TestResult {
    let rom_path = format!("mealybug-tearoom-tests/ppu/{rom_name}");

    let rom = match load_test_rom(&rom_path) {
        Ok(rom) => rom,
        Err(e) => return TestResult::Error(format!("Failed to load test ROM: {e}")),
    };

    let Some(screenshot_path) = expected_screenshot_path(&rom_path, model) else {
        return TestResult::Error(format!("No expected screenshot found for {rom_path}"));
    };

    let config = TestConfig {
        model,
        timeout_frames: MEALYBUG_TIMEOUT_FRAMES,
        ..TestConfig::default()
    };

    let check = Box::new(ScreenshotCheck::new(screenshot_path));

    let mut runner = match TestRunner::new(rom, config, check) {
        Ok(runner) => runner,
        Err(e) => return TestResult::Error(format!("Failed to create test runner: {e}")),
    };

    runner.run()
}

macro_rules! mealybug_ppu_test {
    ($name:ident, $rom:literal, $model:expr) => {
        #[test]
        fn $name() {
            let result = run_mealybug_ppu_test($rom, $model);
            assert!(result.is_passed(), "Test failed with result: {result:?}");
        }
    };
    ($name:ident, $rom:literal, $model:expr, ignore) => {
        #[test]
        #[ignore = "Currently failing - needs fixing"]
        fn $name() {
            let result = run_mealybug_ppu_test($rom, $model);
            assert!(result.is_passed(), "Test failed with result: {result:?}");
        }
    };
    ($name:ident, $rom:literal, $model:expr, ignore_no_screenshot) => {
        #[test]
        #[ignore = "Missing reference screenshot in upstream repository - do not take into account"]
        fn $name() {
            let result = run_mealybug_ppu_test($rom, $model);
            assert!(result.is_passed(), "Test failed with result: {result:?}");
        }
    };
    ($name:ident, $rom:literal) => {
        // Default to DMG model if not specified
        mealybug_ppu_test!($name, $rom, Model::DmgB);
    };
    ($name:ident, $rom:literal, ignore) => {
        // Default to DMG model if not specified
        mealybug_ppu_test!($name, $rom, Model::DmgB, ignore);
    };
    ($name:ident, $rom:literal, ignore_no_screenshot) => {
        // Default to DMG model if not specified
        mealybug_ppu_test!($name, $rom, Model::DmgB, ignore_no_screenshot);
    };
}

// PPU Tests from mealybug-tearoom-tests/ppu
// These tests check various Mode 3 (drawing) behavior changes.

// m2_win_en_toggle.gb
mealybug_ppu_test!(
    test_mb_m2_win_en_toggle_dmg_blob,
    "m2_win_en_toggle.gb",
    Model::DmgB
);
mealybug_ppu_test!(
    test_mb_m2_win_en_toggle_cgb_c,
    "m2_win_en_toggle.gb",
    Model::CgbE
);

// m3_bgp_change.gb
mealybug_ppu_test!(
    test_mb_m3_bgp_change_dmg_blob,
    "m3_bgp_change.gb",
    Model::DmgB,
    ignore
);
mealybug_ppu_test!(
    test_mb_m3_bgp_change_cgb_c,
    "m3_bgp_change.gb",
    Model::CgbE,
    ignore
);

// m3_bgp_change_sprites.gb
mealybug_ppu_test!(
    test_mb_m3_bgp_change_sprites_dmg_blob,
    "m3_bgp_change_sprites.gb",
    Model::DmgB,
    ignore
);
mealybug_ppu_test!(
    test_mb_m3_bgp_change_sprites_cgb_c,
    "m3_bgp_change_sprites.gb",
    Model::CgbE,
    ignore
);

// m3_lcdc_bg_en_change.gb

mealybug_ppu_test!(
    test_mb_m3_lcdc_bg_en_change_dmg_blob,
    "m3_lcdc_bg_en_change.gb",
    Model::DmgB,
    ignore
);

mealybug_ppu_test!(
    test_mb_m3_lcdc_bg_en_change_cgb_c,
    "m3_lcdc_bg_en_change.gb",
    Model::CgbE,
    ignore
);

// m3_lcdc_bg_en_change2.gb
// Note: Missing reference screenshot in upstream mealybug repository for DMG
mealybug_ppu_test!(
    test_mb_m3_lcdc_bg_en_change2_dmg_blob,
    "m3_lcdc_bg_en_change2.gb",
    Model::DmgB,
    ignore_no_screenshot
);
mealybug_ppu_test!(
    test_mb_m3_lcdc_bg_en_change2_cgb_c,
    "m3_lcdc_bg_en_change2.gb",
    Model::CgbC
);

// m3_lcdc_bg_map_change.gb
mealybug_ppu_test!(
    test_mb_m3_lcdc_bg_map_change_dmg_blob,
    "m3_lcdc_bg_map_change.gb",
    Model::DmgB,
    ignore
);
mealybug_ppu_test!(
    test_mb_m3_lcdc_bg_map_change_cgb_c,
    "m3_lcdc_bg_map_change.gb",
    Model::CgbE,
    ignore
);

// m3_lcdc_bg_map_change2.gb
// Note: Missing reference screenshot in upstream mealybug repository for DMG
mealybug_ppu_test!(
    test_mb_m3_lcdc_bg_map_change2_dmg_blob,
    "m3_lcdc_bg_map_change2.gb",
    Model::DmgB,
    ignore_no_screenshot
);
mealybug_ppu_test!(
    test_mb_m3_lcdc_bg_map_change2_cgb_c,
    "m3_lcdc_bg_map_change2.gb",
    Model::CgbC
);

// m3_lcdc_obj_en_change.gb
// Hardware limitation / sub-dot latch behavior:
// These tests toggle LCDC bit 1 (OBJ Enable) mid-scanline during active pixel FIFO output.
// In hardware, the pixel serializer and OBJ enable gate evaluate at sub-dot latch phases,
// causing a 1-pixel boundary difference when LCDC is modified mid-operation (a limitation
// also documented as "off by one" in cycle/gate-level emulators like MetroBoy).
mealybug_ppu_test!(
    test_mb_m3_lcdc_obj_en_change_dmg_blob,
    "m3_lcdc_obj_en_change.gb",
    Model::DmgB,
    ignore
);
mealybug_ppu_test!(
    test_mb_m3_lcdc_obj_en_change_cgb_c,
    "m3_lcdc_obj_en_change.gb",
    Model::CgbC,
    ignore
);

// m3_lcdc_obj_en_change_variant.gb
// Same sub-dot latch behavior limitation as m3_lcdc_obj_en_change above.
mealybug_ppu_test!(
    test_mb_m3_lcdc_obj_en_change_variant_dmg_blob,
    "m3_lcdc_obj_en_change_variant.gb",
    Model::DmgB,
    ignore
);
mealybug_ppu_test!(
    test_mb_m3_lcdc_obj_en_change_variant_cgb_c,
    "m3_lcdc_obj_en_change_variant.gb",
    Model::CgbC,
    ignore
);

// m3_lcdc_obj_size_change.gb
mealybug_ppu_test!(
    test_mb_m3_lcdc_obj_size_change_dmg_blob,
    "m3_lcdc_obj_size_change.gb",
    Model::DmgB
);
mealybug_ppu_test!(
    test_mb_m3_lcdc_obj_size_change_cgb_c,
    "m3_lcdc_obj_size_change.gb",
    Model::CgbC
);

// m3_lcdc_obj_size_change_scx.gb
mealybug_ppu_test!(
    test_mb_m3_lcdc_obj_size_change_scx_dmg_blob,
    "m3_lcdc_obj_size_change_scx.gb",
    Model::DmgB
);
mealybug_ppu_test!(
    test_mb_m3_lcdc_obj_size_change_scx_cgb_c,
    "m3_lcdc_obj_size_change_scx.gb",
    Model::CgbC
);

// m3_lcdc_tile_sel_change.gb
mealybug_ppu_test!(
    test_mb_m3_lcdc_tile_sel_change_dmg_blob,
    "m3_lcdc_tile_sel_change.gb",
    Model::DmgB,
    ignore
);
mealybug_ppu_test!(
    test_mb_m3_lcdc_tile_sel_change_cgb_c,
    "m3_lcdc_tile_sel_change.gb",
    Model::CgbE,
    ignore
);

// m3_lcdc_tile_sel_change2.gb
// Note: Missing reference screenshot in upstream mealybug repository for DMG
mealybug_ppu_test!(
    test_mb_m3_lcdc_tile_sel_change2_dmg_blob,
    "m3_lcdc_tile_sel_change2.gb",
    Model::DmgB,
    ignore_no_screenshot
);
mealybug_ppu_test!(
    test_mb_m3_lcdc_tile_sel_change2_cgb_c,
    "m3_lcdc_tile_sel_change2.gb",
    Model::CgbE,
    ignore
);

// m3_lcdc_tile_sel_win_change.gb
mealybug_ppu_test!(
    test_mb_m3_lcdc_tile_sel_win_change_dmg_blob,
    "m3_lcdc_tile_sel_win_change.gb",
    Model::DmgB,
    ignore
);
mealybug_ppu_test!(
    test_mb_m3_lcdc_tile_sel_win_change_cgb_c,
    "m3_lcdc_tile_sel_win_change.gb",
    Model::CgbE,
    ignore
);

// m3_lcdc_tile_sel_win_change2.gb
// Note: Missing reference screenshot in upstream mealybug repository for DMG
mealybug_ppu_test!(
    test_mb_m3_lcdc_tile_sel_win_change2_dmg_blob,
    "m3_lcdc_tile_sel_win_change2.gb",
    Model::DmgB,
    ignore_no_screenshot
);
mealybug_ppu_test!(
    test_mb_m3_lcdc_tile_sel_win_change2_cgb_c,
    "m3_lcdc_tile_sel_win_change2.gb",
    Model::CgbE,
    ignore
);

// m3_lcdc_win_en_change_multiple.gb
mealybug_ppu_test!(
    test_mb_m3_lcdc_win_en_change_multiple_dmg_blob,
    "m3_lcdc_win_en_change_multiple.gb",
    Model::DmgB,
    ignore
);
mealybug_ppu_test!(
    test_mb_m3_lcdc_win_en_change_multiple_cgb_c,
    "m3_lcdc_win_en_change_multiple.gb",
    Model::CgbE,
    ignore
);

// m3_lcdc_win_en_change_multiple_wx.gb
mealybug_ppu_test!(
    test_mb_m3_lcdc_win_en_change_multiple_wx_dmg_blob,
    "m3_lcdc_win_en_change_multiple_wx.gb",
    Model::DmgB,
    ignore
);
// Note: Missing reference screenshot in upstream mealybug repository for CGB
mealybug_ppu_test!(
    test_mb_m3_lcdc_win_en_change_multiple_wx_cgb_c,
    "m3_lcdc_win_en_change_multiple_wx.gb",
    Model::CgbE,
    ignore_no_screenshot
);

// m3_lcdc_win_map_change.gb
mealybug_ppu_test!(
    test_mb_m3_lcdc_win_map_change_dmg_blob,
    "m3_lcdc_win_map_change.gb",
    Model::DmgB,
    ignore
);
mealybug_ppu_test!(
    test_mb_m3_lcdc_win_map_change_cgb_c,
    "m3_lcdc_win_map_change.gb",
    Model::CgbE,
    ignore
);

// m3_lcdc_win_map_change2.gb
// Note: Missing reference screenshot in upstream mealybug repository for DMG
mealybug_ppu_test!(
    test_mb_m3_lcdc_win_map_change2_dmg_blob,
    "m3_lcdc_win_map_change2.gb",
    Model::DmgB,
    ignore_no_screenshot
);
mealybug_ppu_test!(
    test_mb_m3_lcdc_win_map_change2_cgb_c,
    "m3_lcdc_win_map_change2.gb",
    Model::CgbE
);

// m3_obp0_change.gb
mealybug_ppu_test!(
    test_mb_m3_obp0_change_dmg_blob,
    "m3_obp0_change.gb",
    Model::DmgB
);
mealybug_ppu_test!(
    test_mb_m3_obp0_change_cgb_c,
    "m3_obp0_change.gb",
    Model::CgbC
);

// m3_scx_high_5_bits.gb
mealybug_ppu_test!(
    test_mb_m3_scx_high_5_bits_dmg_blob,
    "m3_scx_high_5_bits.gb",
    Model::DmgB
);
mealybug_ppu_test!(
    test_mb_m3_scx_high_5_bits_cgb_c,
    "m3_scx_high_5_bits.gb",
    Model::CgbE
);

// m3_scx_high_5_bits_change2.gb
// Note: Missing reference screenshot in upstream mealybug repository for DMG
mealybug_ppu_test!(
    test_mb_m3_scx_high_5_bits_change2_dmg_blob,
    "m3_scx_high_5_bits_change2.gb",
    Model::DmgB,
    ignore_no_screenshot
);
mealybug_ppu_test!(
    test_mb_m3_scx_high_5_bits_change2_cgb_c,
    "m3_scx_high_5_bits_change2.gb",
    Model::CgbE,
    ignore
);

// m3_scx_low_3_bits.gb
mealybug_ppu_test!(
    test_mb_m3_scx_low_3_bits_dmg_blob,
    "m3_scx_low_3_bits.gb",
    Model::DmgB
);
mealybug_ppu_test!(
    test_mb_m3_scx_low_3_bits_cgb_c,
    "m3_scx_low_3_bits.gb",
    Model::CgbE
);

// m3_scy_change.gb
mealybug_ppu_test!(
    test_mb_m3_scy_change_dmg_blob,
    "m3_scy_change.gb",
    Model::DmgB,
    ignore
);
mealybug_ppu_test!(
    test_mb_m3_scy_change_cgb_c,
    "m3_scy_change.gb",
    Model::CgbE,
    ignore
);

// m3_scy_change2.gb
// Note: Missing reference screenshot in upstream mealybug repository for DMG
mealybug_ppu_test!(
    test_mb_m3_scy_change2_dmg_blob,
    "m3_scy_change2.gb",
    Model::DmgB,
    ignore_no_screenshot
);
mealybug_ppu_test!(
    test_mb_m3_scy_change2_cgb_c,
    "m3_scy_change2.gb",
    Model::CgbC
);

// m3_window_timing.gb
mealybug_ppu_test!(
    test_mb_m3_window_timing_dmg_blob,
    "m3_window_timing.gb",
    Model::DmgB,
    ignore
);
mealybug_ppu_test!(
    test_mb_m3_window_timing_cgb_c,
    "m3_window_timing.gb",
    Model::CgbC
);

// m3_window_timing_wx_0.gb
mealybug_ppu_test!(
    test_mb_m3_window_timing_wx_0_dmg_blob,
    "m3_window_timing_wx_0.gb",
    Model::DmgB,
    ignore
);
mealybug_ppu_test!(
    test_mb_m3_window_timing_wx_0_cgb_c,
    "m3_window_timing_wx_0.gb",
    Model::CgbC
);

// m3_wx_4_change.gb
mealybug_ppu_test!(
    test_mb_m3_wx_4_change_dmg_blob,
    "m3_wx_4_change.gb",
    Model::DmgB
);
// Note: Missing reference screenshot in upstream mealybug repository for CGB
mealybug_ppu_test!(
    test_mb_m3_wx_4_change_cgb_c,
    "m3_wx_4_change.gb",
    Model::CgbE,
    ignore_no_screenshot
);

// m3_wx_4_change_sprites.gb
mealybug_ppu_test!(
    test_mb_m3_wx_4_change_sprites_dmg_blob,
    "m3_wx_4_change_sprites.gb",
    Model::DmgB
);
mealybug_ppu_test!(
    test_mb_m3_wx_4_change_sprites_cgb_c,
    "m3_wx_4_change_sprites.gb",
    Model::CgbC
);

// m3_wx_5_change.gb
mealybug_ppu_test!(
    test_mb_m3_wx_5_change_dmg_blob,
    "m3_wx_5_change.gb",
    Model::DmgB,
    ignore
);
// Note: Missing reference screenshot in upstream mealybug repository for CGB
mealybug_ppu_test!(
    test_mb_m3_wx_5_change_cgb_c,
    "m3_wx_5_change.gb",
    Model::CgbE,
    ignore_no_screenshot
);

// m3_wx_6_change.gb
mealybug_ppu_test!(
    test_mb_m3_wx_6_change_dmg_blob,
    "m3_wx_6_change.gb",
    Model::DmgB,
    ignore
);
// Note: Missing reference screenshot in upstream mealybug repository for CGB
mealybug_ppu_test!(
    test_mb_m3_wx_6_change_cgb_c,
    "m3_wx_6_change.gb",
    Model::CgbE,
    ignore_no_screenshot
);

// win_without_bg.gb
// Note: Missing reference screenshot in upstream mealybug repository for DMG and CGB
mealybug_ppu_test!(
    test_mb_win_without_bg_dmg_blob,
    "win_without_bg.gb",
    Model::DmgB,
    ignore_no_screenshot
);
mealybug_ppu_test!(
    test_mb_win_without_bg_cgb_c,
    "win_without_bg.gb",
    Model::CgbE,
    ignore_no_screenshot
);
