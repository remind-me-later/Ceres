//! Integration tests using the Mealybug Tearoom Tests ROM suite: the PPU
//! tests compare the screen, the DMA and MBC ones report in the registers.
//! Source: <https://github.com/mealybug/mealybug-tearoom-tests>

use ceres_core::Model;
use ceres_test_runner::{
    Run,
    checks::{RegisterCheck, TestResult},
    rom_test, run_exact_screenshot, timeouts,
};

/// The screen must be the reference screenshot (`*_dmg_blob.png`,
/// `*_cgb_c.png` or `*_cgb_d.png`) when the ROM reaches its `ld b, b`
/// breakpoint.
fn mealybug(rom: &str, model: Model) -> TestResult {
    run_exact_screenshot(
        &format!("mealybug-tearoom-tests/ppu/{rom}"),
        model,
        timeouts::MEALYBUG,
    )
}

// PPU Tests from mealybug-tearoom-tests/ppu
// These tests check various Mode 3 (drawing) behavior changes.

// m2_win_en_toggle.gb
rom_test!(test_mb_m2_win_en_toggle_dmg_blob: mealybug("m2_win_en_toggle.gb", Model::DmgB));
rom_test!(test_mb_m2_win_en_toggle_cgb_c: mealybug("m2_win_en_toggle.gb", Model::CgbC));

// m3_bgp_change.gb
rom_test!(test_mb_m3_bgp_change_dmg_blob: mealybug("m3_bgp_change.gb", Model::DmgB));
rom_test!(test_mb_m3_bgp_change_cgb_c: mealybug("m3_bgp_change.gb", Model::CgbC));

// m3_bgp_change_sprites.gb
rom_test!(
    test_mb_m3_bgp_change_sprites_dmg_blob: mealybug("m3_bgp_change_sprites.gb", Model::DmgB)
);
rom_test!(test_mb_m3_bgp_change_sprites_cgb_c: mealybug("m3_bgp_change_sprites.gb", Model::CgbC));

// m3_lcdc_bg_en_change.gb

rom_test!(
    #[ignore = "fails (not investigated yet)"]
    test_mb_m3_lcdc_bg_en_change_dmg_blob: mealybug("m3_lcdc_bg_en_change.gb", Model::DmgB)
);

rom_test!(test_mb_m3_lcdc_bg_en_change_cgb_c: mealybug("m3_lcdc_bg_en_change.gb", Model::CgbC));

// m3_lcdc_bg_en_change2.gb
// Note: Missing reference screenshot in upstream mealybug repository for DMG
rom_test!(
    #[ignore = "no reference screenshot upstream"]
    test_mb_m3_lcdc_bg_en_change2_dmg_blob: mealybug("m3_lcdc_bg_en_change2.gb", Model::DmgB)
);
rom_test!(test_mb_m3_lcdc_bg_en_change2_cgb_c: mealybug("m3_lcdc_bg_en_change2.gb", Model::CgbC));

// m3_lcdc_bg_map_change.gb
rom_test!(
    test_mb_m3_lcdc_bg_map_change_dmg_blob: mealybug("m3_lcdc_bg_map_change.gb", Model::DmgB)
);
rom_test!(test_mb_m3_lcdc_bg_map_change_cgb_c: mealybug("m3_lcdc_bg_map_change.gb", Model::CgbC));

// m3_lcdc_bg_map_change2.gb
// Note: Missing reference screenshot in upstream mealybug repository for DMG
rom_test!(
    #[ignore = "no reference screenshot upstream"]
    test_mb_m3_lcdc_bg_map_change2_dmg_blob: mealybug("m3_lcdc_bg_map_change2.gb", Model::DmgB)
);
rom_test!(test_mb_m3_lcdc_bg_map_change2_cgb_c: mealybug("m3_lcdc_bg_map_change2.gb", Model::CgbC));

// m3_lcdc_obj_en_change.gb
rom_test!(
    test_mb_m3_lcdc_obj_en_change_dmg_blob: mealybug("m3_lcdc_obj_en_change.gb", Model::DmgB)
);
rom_test!(test_mb_m3_lcdc_obj_en_change_cgb_c: mealybug("m3_lcdc_obj_en_change.gb", Model::CgbC));

// m3_lcdc_obj_en_change_variant.gb
rom_test!(
    test_mb_m3_lcdc_obj_en_change_variant_dmg_blob: mealybug("m3_lcdc_obj_en_change_variant.gb", Model::DmgB)
);
rom_test!(
    test_mb_m3_lcdc_obj_en_change_variant_cgb_c: mealybug("m3_lcdc_obj_en_change_variant.gb", Model::CgbC)
);

// m3_lcdc_obj_size_change.gb
rom_test!(
    test_mb_m3_lcdc_obj_size_change_dmg_blob: mealybug("m3_lcdc_obj_size_change.gb", Model::DmgB)
);
rom_test!(
    test_mb_m3_lcdc_obj_size_change_cgb_c: mealybug("m3_lcdc_obj_size_change.gb", Model::CgbC)
);

// m3_lcdc_obj_size_change_scx.gb
rom_test!(
    test_mb_m3_lcdc_obj_size_change_scx_dmg_blob: mealybug("m3_lcdc_obj_size_change_scx.gb", Model::DmgB)
);
rom_test!(
    test_mb_m3_lcdc_obj_size_change_scx_cgb_c: mealybug("m3_lcdc_obj_size_change_scx.gb", Model::CgbC)
);

// m3_lcdc_tile_sel_change.gb
rom_test!(
    test_mb_m3_lcdc_tile_sel_change_dmg_blob: mealybug("m3_lcdc_tile_sel_change.gb", Model::DmgB)
);
rom_test!(
    #[ignore = "fails (not investigated yet)"]
    test_mb_m3_lcdc_tile_sel_change_cgb_c: mealybug("m3_lcdc_tile_sel_change.gb", Model::CgbC)
);

// m3_lcdc_tile_sel_change2.gb
// Note: Missing reference screenshot in upstream mealybug repository for DMG
rom_test!(
    #[ignore = "no reference screenshot upstream"]
    test_mb_m3_lcdc_tile_sel_change2_dmg_blob: mealybug("m3_lcdc_tile_sel_change2.gb", Model::DmgB)
);
rom_test!(
    #[ignore = "fails (not investigated yet)"]
    test_mb_m3_lcdc_tile_sel_change2_cgb_c: mealybug("m3_lcdc_tile_sel_change2.gb", Model::CgbC)
);

// m3_lcdc_tile_sel_win_change.gb
rom_test!(
    test_mb_m3_lcdc_tile_sel_win_change_dmg_blob: mealybug("m3_lcdc_tile_sel_win_change.gb", Model::DmgB)
);
rom_test!(
    #[ignore = "fails (not investigated yet)"]
    test_mb_m3_lcdc_tile_sel_win_change_cgb_c: mealybug("m3_lcdc_tile_sel_win_change.gb", Model::CgbC)
);

// m3_lcdc_tile_sel_win_change2.gb
// Note: Missing reference screenshot in upstream mealybug repository for DMG
rom_test!(
    #[ignore = "no reference screenshot upstream"]
    test_mb_m3_lcdc_tile_sel_win_change2_dmg_blob: mealybug("m3_lcdc_tile_sel_win_change2.gb", Model::DmgB)
);
rom_test!(
    #[ignore = "fails (not investigated yet)"]
    test_mb_m3_lcdc_tile_sel_win_change2_cgb_c: mealybug("m3_lcdc_tile_sel_win_change2.gb", Model::CgbC)
);

// m3_lcdc_win_en_change_multiple.gb
rom_test!(
    test_mb_m3_lcdc_win_en_change_multiple_dmg_blob: mealybug("m3_lcdc_win_en_change_multiple.gb", Model::DmgB)
);
rom_test!(
    test_mb_m3_lcdc_win_en_change_multiple_cgb_c: mealybug("m3_lcdc_win_en_change_multiple.gb", Model::CgbC)
);

// m3_lcdc_win_en_change_multiple_wx.gb
rom_test!(
    #[ignore = "fails (not investigated yet)"]
    test_mb_m3_lcdc_win_en_change_multiple_wx_dmg_blob: mealybug("m3_lcdc_win_en_change_multiple_wx.gb", Model::DmgB)
);
// Note: Missing reference screenshot in upstream mealybug repository for CGB
rom_test!(
    #[ignore = "no reference screenshot upstream"]
    test_mb_m3_lcdc_win_en_change_multiple_wx_cgb_c: mealybug("m3_lcdc_win_en_change_multiple_wx.gb", Model::CgbC)
);

// m3_lcdc_win_map_change.gb
rom_test!(
    test_mb_m3_lcdc_win_map_change_dmg_blob: mealybug("m3_lcdc_win_map_change.gb", Model::DmgB)
);
rom_test!(test_mb_m3_lcdc_win_map_change_cgb_c: mealybug("m3_lcdc_win_map_change.gb", Model::CgbC));

// m3_lcdc_win_map_change2.gb
// Note: Missing reference screenshot in upstream mealybug repository for DMG
rom_test!(
    #[ignore = "no reference screenshot upstream"]
    test_mb_m3_lcdc_win_map_change2_dmg_blob: mealybug("m3_lcdc_win_map_change2.gb", Model::DmgB)
);
rom_test!(
    test_mb_m3_lcdc_win_map_change2_cgb_c: mealybug("m3_lcdc_win_map_change2.gb", Model::CgbC)
);

// m3_obp0_change.gb
rom_test!(test_mb_m3_obp0_change_dmg_blob: mealybug("m3_obp0_change.gb", Model::DmgB));
rom_test!(test_mb_m3_obp0_change_cgb_c: mealybug("m3_obp0_change.gb", Model::CgbC));

// m3_scx_high_5_bits.gb
rom_test!(test_mb_m3_scx_high_5_bits_dmg_blob: mealybug("m3_scx_high_5_bits.gb", Model::DmgB));
rom_test!(test_mb_m3_scx_high_5_bits_cgb_c: mealybug("m3_scx_high_5_bits.gb", Model::CgbC));

// m3_scx_high_5_bits_change2.gb
// Note: Missing reference screenshot in upstream mealybug repository for DMG
rom_test!(
    #[ignore = "no reference screenshot upstream"]
    test_mb_m3_scx_high_5_bits_change2_dmg_blob: mealybug("m3_scx_high_5_bits_change2.gb", Model::DmgB)
);
rom_test!(
    test_mb_m3_scx_high_5_bits_change2_cgb_c: mealybug("m3_scx_high_5_bits_change2.gb", Model::CgbC)
);

// m3_scx_low_3_bits.gb
rom_test!(test_mb_m3_scx_low_3_bits_dmg_blob: mealybug("m3_scx_low_3_bits.gb", Model::DmgB));
rom_test!(test_mb_m3_scx_low_3_bits_cgb_c: mealybug("m3_scx_low_3_bits.gb", Model::CgbC));

// m3_scy_change.gb
rom_test!(test_mb_m3_scy_change_dmg_blob: mealybug("m3_scy_change.gb", Model::DmgB));
rom_test!(test_mb_m3_scy_change_cgb_c: mealybug("m3_scy_change.gb", Model::CgbC));

// m3_scy_change2.gb
// Note: Missing reference screenshot in upstream mealybug repository for DMG
rom_test!(
    #[ignore = "no reference screenshot upstream"]
    test_mb_m3_scy_change2_dmg_blob: mealybug("m3_scy_change2.gb", Model::DmgB)
);
rom_test!(test_mb_m3_scy_change2_cgb_c: mealybug("m3_scy_change2.gb", Model::CgbC));

// m3_window_timing.gb
rom_test!(test_mb_m3_window_timing_dmg_blob: mealybug("m3_window_timing.gb", Model::DmgB));
rom_test!(test_mb_m3_window_timing_cgb_c: mealybug("m3_window_timing.gb", Model::CgbC));

// m3_window_timing_wx_0.gb
rom_test!(
    test_mb_m3_window_timing_wx_0_dmg_blob: mealybug("m3_window_timing_wx_0.gb", Model::DmgB)
);
rom_test!(test_mb_m3_window_timing_wx_0_cgb_c: mealybug("m3_window_timing_wx_0.gb", Model::CgbC));

// m3_wx_4_change.gb
rom_test!(test_mb_m3_wx_4_change_dmg_blob: mealybug("m3_wx_4_change.gb", Model::DmgB));
// Note: Missing reference screenshot in upstream mealybug repository for CGB
rom_test!(
    #[ignore = "no reference screenshot upstream"]
    test_mb_m3_wx_4_change_cgb_c: mealybug("m3_wx_4_change.gb", Model::CgbC)
);

// m3_wx_4_change_sprites.gb
rom_test!(
    test_mb_m3_wx_4_change_sprites_dmg_blob: mealybug("m3_wx_4_change_sprites.gb", Model::DmgB)
);
rom_test!(
    #[ignore = "fails (not investigated yet)"]
    test_mb_m3_wx_4_change_sprites_cgb_c: mealybug("m3_wx_4_change_sprites.gb", Model::CgbC)
);

// m3_wx_5_change.gb
rom_test!(test_mb_m3_wx_5_change_dmg_blob: mealybug("m3_wx_5_change.gb", Model::DmgB));
// Note: Missing reference screenshot in upstream mealybug repository for CGB
rom_test!(
    #[ignore = "no reference screenshot upstream"]
    test_mb_m3_wx_5_change_cgb_c: mealybug("m3_wx_5_change.gb", Model::CgbC)
);

// m3_wx_6_change.gb
rom_test!(test_mb_m3_wx_6_change_dmg_blob: mealybug("m3_wx_6_change.gb", Model::DmgB));
// Note: Missing reference screenshot in upstream mealybug repository for CGB
rom_test!(
    #[ignore = "no reference screenshot upstream"]
    test_mb_m3_wx_6_change_cgb_c: mealybug("m3_wx_6_change.gb", Model::CgbC)
);

// win_without_bg.gb
// Note: Missing reference screenshot in upstream mealybug repository for DMG and CGB
rom_test!(
    #[ignore = "no reference screenshot upstream"]
    test_mb_win_without_bg_dmg_blob: mealybug("win_without_bg.gb", Model::DmgB)
);
rom_test!(
    #[ignore = "no reference screenshot upstream"]
    test_mb_win_without_bg_cgb_c: mealybug("win_without_bg.gb", Model::CgbC)
);

// CGB-D references (`*_cgb_d.png`). The tile_sel and wx_4 sprite tests fail
// the same way in SameBoy.
rom_test!(test_mb_m2_win_en_toggle_cgb_d: mealybug("m2_win_en_toggle.gb", Model::CgbD));
rom_test!(test_mb_m3_bgp_change_cgb_d: mealybug("m3_bgp_change.gb", Model::CgbD));
rom_test!(test_mb_m3_bgp_change_sprites_cgb_d: mealybug("m3_bgp_change_sprites.gb", Model::CgbD));
rom_test!(test_mb_m3_lcdc_bg_en_change_cgb_d: mealybug("m3_lcdc_bg_en_change.gb", Model::CgbD));
rom_test!(test_mb_m3_lcdc_bg_map_change_cgb_d: mealybug("m3_lcdc_bg_map_change.gb", Model::CgbD));
rom_test!(test_mb_m3_lcdc_obj_en_change_cgb_d: mealybug("m3_lcdc_obj_en_change.gb", Model::CgbD));
rom_test!(
    test_mb_m3_lcdc_obj_en_change_variant_cgb_d: mealybug("m3_lcdc_obj_en_change_variant.gb", Model::CgbD)
);
rom_test!(
    test_mb_m3_lcdc_obj_size_change_cgb_d: mealybug("m3_lcdc_obj_size_change.gb", Model::CgbD)
);
rom_test!(
    test_mb_m3_lcdc_obj_size_change_scx_cgb_d: mealybug("m3_lcdc_obj_size_change_scx.gb", Model::CgbD)
);
rom_test!(
    #[ignore = "fails the same way in SameBoy"]
    test_mb_m3_lcdc_tile_sel_change_cgb_d: mealybug("m3_lcdc_tile_sel_change.gb", Model::CgbD)
);
rom_test!(
    #[ignore = "fails the same way in SameBoy"]
    test_mb_m3_lcdc_tile_sel_win_change_cgb_d: mealybug("m3_lcdc_tile_sel_win_change.gb", Model::CgbD)
);
rom_test!(
    test_mb_m3_lcdc_win_en_change_multiple_cgb_d: mealybug("m3_lcdc_win_en_change_multiple.gb", Model::CgbD)
);
rom_test!(test_mb_m3_lcdc_win_map_change_cgb_d: mealybug("m3_lcdc_win_map_change.gb", Model::CgbD));
rom_test!(test_mb_m3_obp0_change_cgb_d: mealybug("m3_obp0_change.gb", Model::CgbD));
rom_test!(test_mb_m3_scx_high_5_bits_cgb_d: mealybug("m3_scx_high_5_bits.gb", Model::CgbD));
rom_test!(test_mb_m3_scx_low_3_bits_cgb_d: mealybug("m3_scx_low_3_bits.gb", Model::CgbD));
rom_test!(test_mb_m3_scy_change_cgb_d: mealybug("m3_scy_change.gb", Model::CgbD));
rom_test!(test_mb_m3_window_timing_cgb_d: mealybug("m3_window_timing.gb", Model::CgbD));
rom_test!(test_mb_m3_window_timing_wx_0_cgb_d: mealybug("m3_window_timing_wx_0.gb", Model::CgbD));
rom_test!(
    #[ignore = "fails the same way in SameBoy"]
    test_mb_m3_wx_4_change_sprites_cgb_d: mealybug("m3_wx_4_change_sprites.gb", Model::CgbD)
);

/// The registers hold the result (Mooneye's protocol, see `RegisterCheck`).
fn mealybug_registers(rom: &str, model: Model, frames: u32) -> TestResult {
    Run::new(format!("mealybug-tearoom-tests/{rom}"), model)
        .skip_boot_rom()
        .timeout(frames)
        .check(RegisterCheck)
}

// DMA tests from mealybug-tearoom-tests/dma (`-C`: any CGB)
rom_test!(
    test_mb_hdma_during_halt_cgb_c: mealybug_registers("dma/hdma_during_halt-C.gb", Model::CgbC, timeouts::MEALYBUG)
);
rom_test!(
    test_mb_hdma_during_halt_cgb_e: mealybug_registers("dma/hdma_during_halt-C.gb", Model::CgbE, timeouts::MEALYBUG)
);
rom_test!(test_mb_hdma_timing_cgb_c: mealybug_registers("dma/hdma_timing-C.gb", Model::CgbC, timeouts::MEALYBUG));
rom_test!(test_mb_hdma_timing_cgb_e: mealybug_registers("dma/hdma_timing-C.gb", Model::CgbE, timeouts::MEALYBUG));

// MBC tests from mealybug-tearoom-tests/mbc
rom_test!(test_mb_mbc3_rtc: mealybug_registers("mbc/mbc3_rtc.gb", Model::DmgB, timeouts::MEALYBUG_MBC3_RTC));
