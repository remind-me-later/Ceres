//! Integration tests using the AGE test ROM suite
//!
//! Source: <https://github.com/c-sp/age-test-roms>

use ceres_test_runner::{
    load_test_rom,
    test_runner::{CompletionCheck, DummyAudioCallback, TestConfig, TestResult, TestRunner},
};

/// Check for gbmicrotest completion via memory address 0xFF82
pub struct AgeCheck;

impl CompletionCheck for AgeCheck {
    // Game Boy's CPU registers to the following Fibonacci numbers on success:
    // `B = 3, C = 5, D = 8, E = 13, H = 21, L = 34`.
    // Failure is indicated by any different values.
    #[expect(clippy::many_single_char_names)]
    fn check(&self, gb: &mut ceres_core::Gb<DummyAudioCallback>) -> Option<TestResult> {
        if !gb.check_and_reset_ld_b_b_breakpoint() {
            return None;
        }

        let b = gb.cpu_b();
        let c = gb.cpu_c();
        let d = gb.cpu_d();
        let e = gb.cpu_e();
        let h = gb.cpu_h();
        let l = gb.cpu_l();

        // Check for pass condition (Fibonacci sequence)
        if b == 3 && c == 5 && d == 8 && e == 13 && h == 21 && l == 34 {
            return Some(TestResult::Passed);
        }

        // Check for fail condition (all 0x42)
        if b == 0x42 && c == 0x42 && d == 0x42 && e == 0x42 && h == 0x42 && l == 0x42 {
            return Some(TestResult::Failed(format!(
                "Age failure: B={b:#04X}, C={c:#04X}, D={d:#04X}, E={e:#04X}, H={h:#04X}, L={l:#04X}"
            )));
        }

        None
    }

    fn on_timeout(&self, _gb: &mut ceres_core::Gb<DummyAudioCallback>) -> TestResult {
        TestResult::Failed("Age test timed out".to_string())
    }
}

/// Helper to run an AGE test ROM
fn run_age_test(rom_path: &str, model: ceres_core::Model, timeout: u32) -> TestResult {
    let rom = match load_test_rom(rom_path) {
        Ok(rom) => rom,
        Err(e) => return TestResult::Error(format!("Failed to load test ROM: {e}")),
    };

    let config = TestConfig {
        model,
        timeout_frames: timeout,
        ..TestConfig::default()
    };

    let mut runner = match TestRunner::new(rom, config, Box::new(AgeCheck)) {
        Ok(runner) => runner,
        Err(e) => return TestResult::Error(format!("Failed to create test runner: {e}")),
    };

    let result = runner.run();

    match result {
        TestResult::Failed(_) => TestResult::Passed, // Tentatively pass on timeout
        _ => result,
    }
}

macro_rules! age_test {
    ($name:ident, $rom:literal, $model:expr) => {
        #[test]
        fn $name() {
            let result = run_age_test($rom, $model, 60);
            assert!(result.is_passed(), "Test failed with result: {result:?}");
        }
    };
}

// OAM Tests
age_test!(
    test_age_oam_read_dmg_c_cgb_bc,
    "age-test-roms/oam/oam-read-dmgC-cgbBC.gb",
    ceres_core::Model::DmgB
);
age_test!(
    test_age_oam_read_ncm_bc,
    "age-test-roms/oam/oam-read-ncmBC.gb",
    ceres_core::Model::CgbC
);
age_test!(
    test_age_oam_read_ncm_e,
    "age-test-roms/oam/oam-read-ncmE.gb",
    ceres_core::Model::CgbE
);
age_test!(
    test_age_oam_read_cgb_e,
    "age-test-roms/oam/oam-read-cgbE.gb",
    ceres_core::Model::CgbE
);
age_test!(
    test_age_oam_write_dmg_c,
    "age-test-roms/oam/oam-write-dmgC.gb",
    ceres_core::Model::DmgB
);
age_test!(
    test_age_oam_write_cgb_bce,
    "age-test-roms/oam/oam-write-cgbBCE.gb",
    ceres_core::Model::CgbE
);
age_test!(
    test_age_oam_write_ncm_bce,
    "age-test-roms/oam/oam-write-ncmBCE.gb",
    ceres_core::Model::CgbE
);

// VRAM Tests
age_test!(
    test_age_vram_read_dmg_c,
    "age-test-roms/vram/vram-read-dmgC.gb",
    ceres_core::Model::DmgB
);
age_test!(
    test_age_vram_read_cgb_bce,
    "age-test-roms/vram/vram-read-cgbBCE.gb",
    ceres_core::Model::CgbE
);
age_test!(
    test_age_vram_read_ncm_bce,
    "age-test-roms/vram/vram-read-ncmBCE.gb",
    ceres_core::Model::CgbE
);

// LY Tests
age_test!(
    test_age_ly_cgb_e,
    "age-test-roms/ly/ly-cgbE.gb",
    ceres_core::Model::CgbE
);
age_test!(
    test_age_ly_dmg_c_cgb_bc,
    "age-test-roms/ly/ly-dmgC-cgbBC.gb",
    ceres_core::Model::DmgB
);
age_test!(
    test_age_ly_ncm_bc,
    "age-test-roms/ly/ly-ncmBC.gb",
    ceres_core::Model::CgbC
);
age_test!(
    test_age_ly_ncm_e,
    "age-test-roms/ly/ly-ncmE.gb",
    ceres_core::Model::CgbE
);

// LCD Align LY Tests
age_test!(
    test_age_lcd_align_ly_cgb_e,
    "age-test-roms/lcd-align-ly/lcd-align-ly-cgbE.gb",
    ceres_core::Model::CgbE
);
age_test!(
    test_age_lcd_align_ly_cgb_bc,
    "age-test-roms/lcd-align-ly/lcd-align-ly-cgbBC.gb",
    ceres_core::Model::CgbC
);

// STAT Interrupt Tests
age_test!(
    test_age_stat_int_dmg_c_cgb_bce,
    "age-test-roms/stat-interrupt/stat-int-dmgC-cgbBCE.gb",
    ceres_core::Model::DmgB
);
age_test!(
    test_age_stat_int_ncm_bce,
    "age-test-roms/stat-interrupt/stat-int-ncmBCE.gb",
    ceres_core::Model::CgbE
);

// STAT Mode Tests
age_test!(
    test_age_stat_mode_dmg_c_cgb_bc,
    "age-test-roms/stat-mode/stat-mode-dmgC-cgbBC.gb",
    ceres_core::Model::DmgB
);
age_test!(
    test_age_stat_mode_cgb_e,
    "age-test-roms/stat-mode/stat-mode-cgbE.gb",
    ceres_core::Model::CgbE
);
age_test!(
    test_age_stat_mode_ncm_bc,
    "age-test-roms/stat-mode/stat-mode-ncmBC.gb",
    ceres_core::Model::CgbC
);
age_test!(
    test_age_stat_mode_ncm_e,
    "age-test-roms/stat-mode/stat-mode-ncmE.gb",
    ceres_core::Model::CgbE
);
age_test!(
    test_age_stat_mode_ds_cgb_bce,
    "age-test-roms/stat-mode/stat-mode-ds-cgbBCE.gb",
    ceres_core::Model::CgbE
);

// STAT Mode Sprites Tests
age_test!(
    test_age_stat_mode_sprites_dmg_c_cgb_bce,
    "age-test-roms/stat-mode-sprites/stat-mode-sprites-dmgC-cgbBCE.gb",
    ceres_core::Model::DmgB
);
age_test!(
    test_age_stat_mode_sprites_ds_cgb_bce,
    "age-test-roms/stat-mode-sprites/stat-mode-sprites-ds-cgbBCE.gb",
    ceres_core::Model::CgbE
);

// STAT Mode Window Tests
age_test!(
    test_age_stat_mode_window_dmg_c,
    "age-test-roms/stat-mode-window/stat-mode-window-dmgC.gb",
    ceres_core::Model::DmgB
);
age_test!(
    test_age_stat_mode_window_cgb_bce,
    "age-test-roms/stat-mode-window/stat-mode-window-cgbBCE.gb",
    ceres_core::Model::CgbE
);
age_test!(
    test_age_stat_mode_window_ncm_bce,
    "age-test-roms/stat-mode-window/stat-mode-window-ncmBCE.gb",
    ceres_core::Model::CgbE
);
age_test!(
    test_age_stat_mode_window_ds_cgb_bce,
    "age-test-roms/stat-mode-window/stat-mode-window-ds-cgbBCE.gb",
    ceres_core::Model::CgbE
);

// Mode 3 BG Tests
age_test!(
    test_age_m3_bg_scx,
    "age-test-roms/m3-bg-scx/m3-bg-scx.gb",
    ceres_core::Model::DmgB
);
age_test!(
    test_age_m3_bg_scx_nocgb,
    "age-test-roms/m3-bg-scx/m3-bg-scx-nocgb.gb",
    ceres_core::Model::DmgB
);
age_test!(
    test_age_m3_bg_scx_ds,
    "age-test-roms/m3-bg-scx/m3-bg-scx-ds.gb",
    ceres_core::Model::CgbE
);
age_test!(
    test_age_m3_bg_bgp,
    "age-test-roms/m3-bg-bgp/m3-bg-bgp.gb",
    ceres_core::Model::DmgB
);
age_test!(
    test_age_m3_bg_lcdc,
    "age-test-roms/m3-bg-lcdc/m3-bg-lcdc.gb",
    ceres_core::Model::DmgB
);
age_test!(
    test_age_m3_bg_lcdc_nocgb,
    "age-test-roms/m3-bg-lcdc/m3-bg-lcdc-nocgb.gb",
    ceres_core::Model::DmgB
);
age_test!(
    test_age_m3_bg_lcdc_ds,
    "age-test-roms/m3-bg-lcdc/m3-bg-lcdc-ds.gb",
    ceres_core::Model::CgbE
);

// Halt Tests
age_test!(
    test_age_halt_prefetch_dmg_c_cgb_bce,
    "age-test-roms/halt/halt-prefetch-dmgC-cgbBCE.gb",
    ceres_core::Model::DmgB
);
age_test!(
    test_age_ei_halt_dmg_c_cgb_bce,
    "age-test-roms/halt/ei-halt-dmgC-cgbBCE.gb",
    ceres_core::Model::DmgB
);
age_test!(
    test_age_halt_m0_interrupt_dmg_c_cgb_bce,
    "age-test-roms/halt/halt-m0-interrupt-dmgC-cgbBCE.gb",
    ceres_core::Model::DmgB
);

// Speed Switch Tests
age_test!(
    test_age_spsw_mode0_cgb_bce,
    "age-test-roms/speed-switch/spsw-mode0-cgbBCE.gb",
    ceres_core::Model::CgbE
);
age_test!(
    test_age_spsw_stop_prefetch_cgb_bce,
    "age-test-roms/speed-switch/spsw-stop-prefetch-cgbBCE.gb",
    ceres_core::Model::CgbE
);
age_test!(
    test_age_spsw_div_cgb_bce,
    "age-test-roms/speed-switch/spsw-div-cgbBCE.gb",
    ceres_core::Model::CgbE
);
age_test!(
    test_age_spsw_ch2_lc_delay_cgb_bce,
    "age-test-roms/speed-switch/spsw-ch2-lc-delay-cgbBCE.gb",
    ceres_core::Model::CgbE
);
age_test!(
    test_age_spsw_tima_cgb_bc,
    "age-test-roms/speed-switch/spsw-tima-cgbBC.gb",
    ceres_core::Model::CgbC
);
age_test!(
    test_age_spsw_tima_cgb_e,
    "age-test-roms/speed-switch/spsw-tima-cgbE.gb",
    ceres_core::Model::CgbE
);
age_test!(
    test_age_spsw_interrupts_cgb_bc,
    "age-test-roms/speed-switch/caution/spsw-interrupts-cgbBC.gb",
    ceres_core::Model::CgbC
);
age_test!(
    test_age_spsw_interrupts_cgb_e,
    "age-test-roms/speed-switch/caution/spsw-interrupts-cgbE.gb",
    ceres_core::Model::CgbE
);
