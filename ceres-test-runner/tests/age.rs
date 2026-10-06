//! Integration tests using the AGE test ROMs.
//!
//! Source: <https://github.com/c-sp/age-test-roms>. Most ROMs report in the
//! registers (the Fibonacci numbers on success); the `m3-*` ones are compared
//! with the screenshots of the real hardware. Each ROM runs on the hardware its
//! name says it was verified on. The tests that are ignored fail the same way in
//! SameBoy, the reference this emulator follows.

use ceres_core::Model;
use ceres_test_runner::{run_register_test, run_screenshot_test};

macro_rules! register_test {
    ($name:ident, $rom:literal, $model:expr) => {
        #[test]
        fn $name() {
            let result = run_register_test($rom, $model, 800);
            assert!(result.is_passed(), "{result:?}");
        }
    };
    ($name:ident, $rom:literal, $model:expr, ignore = $reason:literal) => {
        #[test]
        #[ignore = $reason]
        fn $name() {
            let result = run_register_test($rom, $model, 800);
            assert!(result.is_passed(), "{result:?}");
        }
    };
}

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

// ROMs that report in the registers.
register_test!(
    age_halt_ei_halt_dmgc_cgbbce_dmg,
    "age-test-roms/halt/ei-halt-dmgC-cgbBCE.gb",
    Model::DmgB
);
register_test!(
    age_halt_ei_halt_dmgc_cgbbce_cgbc,
    "age-test-roms/halt/ei-halt-dmgC-cgbBCE.gb",
    Model::CgbC
);
register_test!(
    age_halt_ei_halt_dmgc_cgbbce_cgbe,
    "age-test-roms/halt/ei-halt-dmgC-cgbBCE.gb",
    Model::CgbE
);
register_test!(
    age_halt_m0_interrupt_dmgc_cgbbce_dmg,
    "age-test-roms/halt/halt-m0-interrupt-dmgC-cgbBCE.gb",
    Model::DmgB
);
register_test!(
    age_halt_m0_interrupt_dmgc_cgbbce_cgbc,
    "age-test-roms/halt/halt-m0-interrupt-dmgC-cgbBCE.gb",
    Model::CgbC
);
register_test!(
    age_halt_m0_interrupt_dmgc_cgbbce_cgbe,
    "age-test-roms/halt/halt-m0-interrupt-dmgC-cgbBCE.gb",
    Model::CgbE
);
register_test!(
    age_halt_prefetch_dmgc_cgbbce_dmg,
    "age-test-roms/halt/halt-prefetch-dmgC-cgbBCE.gb",
    Model::DmgB
);
register_test!(
    age_halt_prefetch_dmgc_cgbbce_cgbc,
    "age-test-roms/halt/halt-prefetch-dmgC-cgbBCE.gb",
    Model::CgbC
);
register_test!(
    age_halt_prefetch_dmgc_cgbbce_cgbe,
    "age-test-roms/halt/halt-prefetch-dmgC-cgbBCE.gb",
    Model::CgbE
);
register_test!(
    age_lcd_align_ly_cgbbc_cgbc,
    "age-test-roms/lcd-align-ly/lcd-align-ly-cgbBC.gb",
    Model::CgbC,
    ignore = "fails the same way in SameBoy"
);
register_test!(
    age_lcd_align_ly_cgbe_cgbe,
    "age-test-roms/lcd-align-ly/lcd-align-ly-cgbE.gb",
    Model::CgbE,
    ignore = "fails the same way in SameBoy"
);
register_test!(age_ly_cgbe_cgbe, "age-test-roms/ly/ly-cgbE.gb", Model::CgbE);
register_test!(
    age_ly_dmgc_cgbbc_dmg,
    "age-test-roms/ly/ly-dmgC-cgbBC.gb",
    Model::DmgB
);
register_test!(
    age_ly_dmgc_cgbbc_cgbc,
    "age-test-roms/ly/ly-dmgC-cgbBC.gb",
    Model::CgbC
);
register_test!(
    age_ly_ncmbc_cgbc,
    "age-test-roms/ly/ly-ncmBC.gb",
    Model::CgbC
);
register_test!(age_ly_ncme_cgbe, "age-test-roms/ly/ly-ncmE.gb", Model::CgbE);
register_test!(
    age_oam_read_cgbe_cgbe,
    "age-test-roms/oam/oam-read-cgbE.gb",
    Model::CgbE
);
register_test!(
    age_oam_read_dmgc_cgbbc_dmg,
    "age-test-roms/oam/oam-read-dmgC-cgbBC.gb",
    Model::DmgB
);
register_test!(
    age_oam_read_dmgc_cgbbc_cgbc,
    "age-test-roms/oam/oam-read-dmgC-cgbBC.gb",
    Model::CgbC
);
register_test!(
    age_oam_read_ncmbc_cgbc,
    "age-test-roms/oam/oam-read-ncmBC.gb",
    Model::CgbC
);
register_test!(
    age_oam_read_ncme_cgbe,
    "age-test-roms/oam/oam-read-ncmE.gb",
    Model::CgbE
);
register_test!(
    age_oam_write_cgbbce_cgbc,
    "age-test-roms/oam/oam-write-cgbBCE.gb",
    Model::CgbC
);
register_test!(
    age_oam_write_cgbbce_cgbe,
    "age-test-roms/oam/oam-write-cgbBCE.gb",
    Model::CgbE
);
register_test!(
    age_oam_write_dmgc_dmg,
    "age-test-roms/oam/oam-write-dmgC.gb",
    Model::DmgB
);
register_test!(
    age_oam_write_ncmbce_cgbc,
    "age-test-roms/oam/oam-write-ncmBCE.gb",
    Model::CgbC
);
register_test!(
    age_oam_write_ncmbce_cgbe,
    "age-test-roms/oam/oam-write-ncmBCE.gb",
    Model::CgbE
);
register_test!(
    age_speed_switch_caution_spsw_interrupts_cgbbc_cgbc,
    "age-test-roms/speed-switch/caution/spsw-interrupts-cgbBC.gb",
    Model::CgbC,
    ignore = "fails the same way in SameBoy"
);
register_test!(
    age_speed_switch_caution_spsw_interrupts_cgbe_cgbe,
    "age-test-roms/speed-switch/caution/spsw-interrupts-cgbE.gb",
    Model::CgbE
);
register_test!(
    age_speed_switch_spsw_ch2_lc_delay_cgbbce_cgbc,
    "age-test-roms/speed-switch/spsw-ch2-lc-delay-cgbBCE.gb",
    Model::CgbC,
    ignore = "fails the same way in SameBoy"
);
register_test!(
    age_speed_switch_spsw_ch2_lc_delay_cgbbce_cgbe,
    "age-test-roms/speed-switch/spsw-ch2-lc-delay-cgbBCE.gb",
    Model::CgbE,
    ignore = "fails the same way in SameBoy"
);
register_test!(
    age_speed_switch_spsw_div_cgbbce_cgbc,
    "age-test-roms/speed-switch/spsw-div-cgbBCE.gb",
    Model::CgbC
);
register_test!(
    age_speed_switch_spsw_div_cgbbce_cgbe,
    "age-test-roms/speed-switch/spsw-div-cgbBCE.gb",
    Model::CgbE
);
register_test!(
    age_speed_switch_spsw_mode0_cgbbce_cgbc,
    "age-test-roms/speed-switch/spsw-mode0-cgbBCE.gb",
    Model::CgbC
);
register_test!(
    age_speed_switch_spsw_mode0_cgbbce_cgbe,
    "age-test-roms/speed-switch/spsw-mode0-cgbBCE.gb",
    Model::CgbE
);
register_test!(
    age_speed_switch_spsw_stop_prefetch_cgbbce_cgbc,
    "age-test-roms/speed-switch/spsw-stop-prefetch-cgbBCE.gb",
    Model::CgbC
);
register_test!(
    age_speed_switch_spsw_stop_prefetch_cgbbce_cgbe,
    "age-test-roms/speed-switch/spsw-stop-prefetch-cgbBCE.gb",
    Model::CgbE
);
register_test!(
    age_speed_switch_spsw_tima_cgbbc_cgbc,
    "age-test-roms/speed-switch/spsw-tima-cgbBC.gb",
    Model::CgbC,
    ignore = "fails the same way in SameBoy"
);
register_test!(
    age_speed_switch_spsw_tima_cgbe_cgbe,
    "age-test-roms/speed-switch/spsw-tima-cgbE.gb",
    Model::CgbE,
    ignore = "fails the same way in SameBoy"
);
register_test!(
    age_stat_interrupt_stat_int_dmgc_cgbbce_dmg,
    "age-test-roms/stat-interrupt/stat-int-dmgC-cgbBCE.gb",
    Model::DmgB
);
register_test!(
    age_stat_interrupt_stat_int_dmgc_cgbbce_cgbc,
    "age-test-roms/stat-interrupt/stat-int-dmgC-cgbBCE.gb",
    Model::CgbC
);
register_test!(
    age_stat_interrupt_stat_int_dmgc_cgbbce_cgbe,
    "age-test-roms/stat-interrupt/stat-int-dmgC-cgbBCE.gb",
    Model::CgbE
);
register_test!(
    age_stat_interrupt_stat_int_ncmbce_cgbc,
    "age-test-roms/stat-interrupt/stat-int-ncmBCE.gb",
    Model::CgbC
);
register_test!(
    age_stat_interrupt_stat_int_ncmbce_cgbe,
    "age-test-roms/stat-interrupt/stat-int-ncmBCE.gb",
    Model::CgbE
);
register_test!(
    age_stat_mode_sprites_dmgc_cgbbce_dmg,
    "age-test-roms/stat-mode-sprites/stat-mode-sprites-dmgC-cgbBCE.gb",
    Model::DmgB
);
register_test!(
    age_stat_mode_sprites_dmgc_cgbbce_cgbc,
    "age-test-roms/stat-mode-sprites/stat-mode-sprites-dmgC-cgbBCE.gb",
    Model::CgbC
);
register_test!(
    age_stat_mode_sprites_dmgc_cgbbce_cgbe,
    "age-test-roms/stat-mode-sprites/stat-mode-sprites-dmgC-cgbBCE.gb",
    Model::CgbE
);
register_test!(
    age_stat_mode_sprites_ds_cgbbce_cgbc,
    "age-test-roms/stat-mode-sprites/stat-mode-sprites-ds-cgbBCE.gb",
    Model::CgbC
);
register_test!(
    age_stat_mode_sprites_ds_cgbbce_cgbe,
    "age-test-roms/stat-mode-sprites/stat-mode-sprites-ds-cgbBCE.gb",
    Model::CgbE
);
register_test!(
    age_stat_mode_window_cgbbce_cgbc,
    "age-test-roms/stat-mode-window/stat-mode-window-cgbBCE.gb",
    Model::CgbC
);
register_test!(
    age_stat_mode_window_cgbbce_cgbe,
    "age-test-roms/stat-mode-window/stat-mode-window-cgbBCE.gb",
    Model::CgbE
);
register_test!(
    age_stat_mode_window_dmgc_dmg,
    "age-test-roms/stat-mode-window/stat-mode-window-dmgC.gb",
    Model::DmgB
);
register_test!(
    age_stat_mode_window_ds_cgbbce_cgbc,
    "age-test-roms/stat-mode-window/stat-mode-window-ds-cgbBCE.gb",
    Model::CgbC
);
register_test!(
    age_stat_mode_window_ds_cgbbce_cgbe,
    "age-test-roms/stat-mode-window/stat-mode-window-ds-cgbBCE.gb",
    Model::CgbE
);
register_test!(
    age_stat_mode_window_ncmbce_cgbc,
    "age-test-roms/stat-mode-window/stat-mode-window-ncmBCE.gb",
    Model::CgbC
);
register_test!(
    age_stat_mode_window_ncmbce_cgbe,
    "age-test-roms/stat-mode-window/stat-mode-window-ncmBCE.gb",
    Model::CgbE
);
register_test!(
    age_stat_mode_cgbe_cgbe,
    "age-test-roms/stat-mode/stat-mode-cgbE.gb",
    Model::CgbE
);
register_test!(
    age_stat_mode_dmgc_cgbbc_dmg,
    "age-test-roms/stat-mode/stat-mode-dmgC-cgbBC.gb",
    Model::DmgB
);
register_test!(
    age_stat_mode_dmgc_cgbbc_cgbc,
    "age-test-roms/stat-mode/stat-mode-dmgC-cgbBC.gb",
    Model::CgbC,
    ignore = "fails the same way in SameBoy"
);
register_test!(
    age_stat_mode_ds_cgbbce_cgbc,
    "age-test-roms/stat-mode/stat-mode-ds-cgbBCE.gb",
    Model::CgbC
);
register_test!(
    age_stat_mode_ds_cgbbce_cgbe,
    "age-test-roms/stat-mode/stat-mode-ds-cgbBCE.gb",
    Model::CgbE
);
register_test!(
    age_stat_mode_ncmbc_cgbc,
    "age-test-roms/stat-mode/stat-mode-ncmBC.gb",
    Model::CgbC,
    ignore = "fails the same way in SameBoy"
);
register_test!(
    age_stat_mode_ncme_cgbe,
    "age-test-roms/stat-mode/stat-mode-ncmE.gb",
    Model::CgbE
);
register_test!(
    age_vram_read_cgbbce_cgbc,
    "age-test-roms/vram/vram-read-cgbBCE.gb",
    Model::CgbC
);
register_test!(
    age_vram_read_cgbbce_cgbe,
    "age-test-roms/vram/vram-read-cgbBCE.gb",
    Model::CgbE
);
register_test!(
    age_vram_read_dmgc_dmg,
    "age-test-roms/vram/vram-read-dmgC.gb",
    Model::DmgB
);
register_test!(
    age_vram_read_ncmbce_cgbc,
    "age-test-roms/vram/vram-read-ncmBCE.gb",
    Model::CgbC
);
register_test!(
    age_vram_read_ncmbce_cgbe,
    "age-test-roms/vram/vram-read-ncmBCE.gb",
    Model::CgbE
);

// ROMs compared with a screenshot.
screenshot_test!(
    age_m3_bg_bgp_cgbc,
    "age-test-roms/m3-bg-bgp/m3-bg-bgp.gb",
    "age-test-roms/m3-bg-bgp/m3-bg-bgp-ncmBC.png",
    Model::CgbC
);
screenshot_test!(
    age_m3_bg_bgp_cgbe,
    "age-test-roms/m3-bg-bgp/m3-bg-bgp.gb",
    "age-test-roms/m3-bg-bgp/m3-bg-bgp-ncmE.png",
    Model::CgbE
);
screenshot_test!(
    age_m3_bg_bgp_dmg,
    "age-test-roms/m3-bg-bgp/m3-bg-bgp.gb",
    "age-test-roms/m3-bg-bgp/m3-bg-bgp-dmgC.png",
    Model::DmgB,
    ignore = "fails the same way in SameBoy"
);
screenshot_test!(
    age_m3_bg_lcdc_ds_cgbc,
    "age-test-roms/m3-bg-lcdc/m3-bg-lcdc-ds.gb",
    "age-test-roms/m3-bg-lcdc/m3-bg-lcdc-ds-cgbBCE.png",
    Model::CgbC
);
screenshot_test!(
    age_m3_bg_lcdc_ds_cgbe,
    "age-test-roms/m3-bg-lcdc/m3-bg-lcdc-ds.gb",
    "age-test-roms/m3-bg-lcdc/m3-bg-lcdc-ds-cgbBCE.png",
    Model::CgbE
);
screenshot_test!(
    age_m3_bg_lcdc_nocgb_cgbc,
    "age-test-roms/m3-bg-lcdc/m3-bg-lcdc-nocgb.gb",
    "age-test-roms/m3-bg-lcdc/m3-bg-lcdc-nocgb-ncmBCE.png",
    Model::CgbC,
    ignore = "fails the same way in SameBoy"
);
screenshot_test!(
    age_m3_bg_lcdc_nocgb_cgbe,
    "age-test-roms/m3-bg-lcdc/m3-bg-lcdc-nocgb.gb",
    "age-test-roms/m3-bg-lcdc/m3-bg-lcdc-nocgb-ncmBCE.png",
    Model::CgbE,
    ignore = "fails the same way in SameBoy"
);
screenshot_test!(
    age_m3_bg_lcdc_cgbc,
    "age-test-roms/m3-bg-lcdc/m3-bg-lcdc.gb",
    "age-test-roms/m3-bg-lcdc/m3-bg-lcdc-cgbBCE.png",
    Model::CgbC,
    ignore = "fails the same way in SameBoy"
);
screenshot_test!(
    age_m3_bg_lcdc_cgbe,
    "age-test-roms/m3-bg-lcdc/m3-bg-lcdc.gb",
    "age-test-roms/m3-bg-lcdc/m3-bg-lcdc-cgbBCE.png",
    Model::CgbE,
    ignore = "fails the same way in SameBoy"
);
screenshot_test!(
    age_m3_bg_lcdc_dmg,
    "age-test-roms/m3-bg-lcdc/m3-bg-lcdc.gb",
    "age-test-roms/m3-bg-lcdc/m3-bg-lcdc-dmgC.png",
    Model::DmgB
);
screenshot_test!(
    age_m3_bg_scx_ds_cgbc,
    "age-test-roms/m3-bg-scx/m3-bg-scx-ds.gb",
    "age-test-roms/m3-bg-scx/m3-bg-scx-ds-cgbBCE.png",
    Model::CgbC
);
screenshot_test!(
    age_m3_bg_scx_ds_cgbe,
    "age-test-roms/m3-bg-scx/m3-bg-scx-ds.gb",
    "age-test-roms/m3-bg-scx/m3-bg-scx-ds-cgbBCE.png",
    Model::CgbE
);
screenshot_test!(
    age_m3_bg_scx_nocgb_cgbc,
    "age-test-roms/m3-bg-scx/m3-bg-scx-nocgb.gb",
    "age-test-roms/m3-bg-scx/m3-bg-scx-nocgb-ncmBCE.png",
    Model::CgbC
);
screenshot_test!(
    age_m3_bg_scx_nocgb_cgbe,
    "age-test-roms/m3-bg-scx/m3-bg-scx-nocgb.gb",
    "age-test-roms/m3-bg-scx/m3-bg-scx-nocgb-ncmBCE.png",
    Model::CgbE
);
screenshot_test!(
    age_m3_bg_scx_cgbc,
    "age-test-roms/m3-bg-scx/m3-bg-scx.gb",
    "age-test-roms/m3-bg-scx/m3-bg-scx-cgbBCE.png",
    Model::CgbC
);
screenshot_test!(
    age_m3_bg_scx_cgbe,
    "age-test-roms/m3-bg-scx/m3-bg-scx.gb",
    "age-test-roms/m3-bg-scx/m3-bg-scx-cgbBCE.png",
    Model::CgbE
);
screenshot_test!(
    age_m3_bg_scx_dmg,
    "age-test-roms/m3-bg-scx/m3-bg-scx.gb",
    "age-test-roms/m3-bg-scx/m3-bg-scx-dmgC.png",
    Model::DmgB
);
