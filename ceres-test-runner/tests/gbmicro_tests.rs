//! Integration tests using the gbmicrotest ROM suite
//!
//! These tests validate specific low-level CPU, PPU, and timing behaviors
//! using the gbmicrotest suite. Each test ROM checks a single hardware behavior
//! and reports pass/fail via memory address 0xFF82.
//!
//! Test ROMs complete in 2 frames for most tests, except:
//! - `is_if_set_during_ime0.gb`: requires ~380ms emulated time (~24 frames)

use ceres_test_runner::{
    load_test_rom,
    test_runner::{TestConfig, TestResult, TestRunner},
};

/// Helper to run a gbmicrotest ROM
///
/// # Arguments
/// * `rom_name` - Name of the ROM file (without path)
/// * `frames` - Number of frames to run (default: 2, special: 24 for is_if_set_during_ime0)
fn run_gbmicrotest(rom_name: &str, frames: u32) -> TestResult {
    let path = format!("gbmicrotest/{}", rom_name);
    let rom = match load_test_rom(&path) {
        Ok(rom) => rom,
        Err(e) => return TestResult::Failed(format!("Failed to load test ROM: {e}")),
    };

    let config = TestConfig {
        timeout_frames: frames,
        model: ceres_core::Model::Dmg, // gbmicrotest targets DMG
        capture_serial: false,         // These tests don't use serial output
        ..TestConfig::default()
    };

    let mut runner = match TestRunner::new(rom, config) {
        Ok(runner) => runner,
        Err(e) => return TestResult::Failed(format!("Failed to create test runner: {e}")),
    };

    // Run the test (ignore return value since completion is signaled via memory)
    let _ = runner.run();

    // Check result at 0xFF82: 0x01 = pass, 0xFF = fail
    let result = runner.read_memory(0xFF82);
    match result {
        0x01 => TestResult::Passed,
        0xFF => {
            let actual = runner.read_memory(0xFF80);
            let expected = runner.read_memory(0xFF81);
            TestResult::Failed(format!(
                "Test failed: actual=0x{:02X}, expected=0x{:02X}",
                actual, expected
            ))
        }
        _ => TestResult::Unknown,
    }
}

/// Macro to generate test cases for gbmicrotest ROMs
///
/// All tests are marked as ignored by default since gbmicrotest requires
/// very accurate timing and hardware behavior. Tests can be individually
/// enabled as emulator accuracy improves.
macro_rules! gbmicrotest {
    ($name:ident, $rom:literal) => {
        #[test]
        #[ignore = "gbmicrotest requires cycle-accurate timing - enable individually as accuracy improves"]
        fn $name() {
            let result = run_gbmicrotest($rom, 2);
            assert_eq!(result, TestResult::Passed);
        }
    };
    ($name:ident, $rom:literal, frames = $frames:literal) => {
        #[test]
        #[ignore = "gbmicrotest requires cycle-accurate timing - enable individually as accuracy improves"]
        fn $name() {
            let result = run_gbmicrotest($rom, $frames);
            assert_eq!(result, TestResult::Passed);
        }
    };
}

// Generated test declarations
gbmicrotest!(test_n000_oam_lock, "000-oam_lock.gb");
gbmicrotest!(test_n000_write_to_x8000, "000-write_to_x8000.gb");
gbmicrotest!(test_n001_vram_unlocked, "001-vram_unlocked.gb");
gbmicrotest!(test_n002_vram_locked, "002-vram_locked.gb");
gbmicrotest!(test_n004_tima_boot_phase, "004-tima_boot_phase.gb");
gbmicrotest!(test_n004_tima_cycle_timer, "004-tima_cycle_timer.gb");
gbmicrotest!(test_n007_lcd_on_stat, "007-lcd_on_stat.gb");
gbmicrotest!(test_n400_dma, "400-dma.gb");
gbmicrotest!(test_n500_scx_timing, "500-scx-timing.gb");
gbmicrotest!(test_n800_ppu_latch_scx, "800-ppu-latch-scx.gb");
gbmicrotest!(test_n801_ppu_latch_scy, "801-ppu-latch-scy.gb");
gbmicrotest!(
    test_n802_ppu_latch_tileselect,
    "802-ppu-latch-tileselect.gb"
);
gbmicrotest!(test_n803_ppu_latch_bgdisplay, "803-ppu-latch-bgdisplay.gb");
gbmicrotest!(test_audio_testbench, "audio_testbench.gb");
gbmicrotest!(test_cpu_bus_1, "cpu_bus_1.gb");
gbmicrotest!(test_div_inc_timing_a, "div_inc_timing_a.gb");
gbmicrotest!(test_div_inc_timing_b, "div_inc_timing_b.gb");
gbmicrotest!(test_dma_0x1000, "dma_0x1000.gb");
gbmicrotest!(test_dma_0x9000, "dma_0x9000.gb");
gbmicrotest!(test_dma_0xA000, "dma_0xA000.gb");
gbmicrotest!(test_dma_0xC000, "dma_0xC000.gb");
gbmicrotest!(test_dma_0xE000, "dma_0xE000.gb");
gbmicrotest!(test_dma_basic, "dma_basic.gb");
gbmicrotest!(test_dma_timing_a, "dma_timing_a.gb");
gbmicrotest!(test_flood_vram, "flood_vram.gb");
gbmicrotest!(test_halt_bug, "halt_bug.gb");
gbmicrotest!(test_halt_op_dupe_delay, "halt_op_dupe_delay.gb");
gbmicrotest!(test_halt_op_dupe, "halt_op_dupe.gb");
gbmicrotest!(test_hblank_int_di_timing_a, "hblank_int_di_timing_a.gb");
gbmicrotest!(test_hblank_int_di_timing_b, "hblank_int_di_timing_b.gb");
gbmicrotest!(test_hblank_int_if_a, "hblank_int_if_a.gb");
gbmicrotest!(test_hblank_int_if_b, "hblank_int_if_b.gb");
gbmicrotest!(test_hblank_int_l0, "hblank_int_l0.gb");
gbmicrotest!(test_hblank_int_l1, "hblank_int_l1.gb");
gbmicrotest!(test_hblank_int_l2, "hblank_int_l2.gb");
gbmicrotest!(test_hblank_int_scx0, "hblank_int_scx0.gb");
gbmicrotest!(test_hblank_int_scx0_if_a, "hblank_int_scx0_if_a.gb");
gbmicrotest!(test_hblank_int_scx0_if_b, "hblank_int_scx0_if_b.gb");
gbmicrotest!(test_hblank_int_scx0_if_c, "hblank_int_scx0_if_c.gb");
gbmicrotest!(test_hblank_int_scx0_if_d, "hblank_int_scx0_if_d.gb");
gbmicrotest!(test_hblank_int_scx1, "hblank_int_scx1.gb");
gbmicrotest!(test_hblank_int_scx1_if_a, "hblank_int_scx1_if_a.gb");
gbmicrotest!(test_hblank_int_scx1_if_b, "hblank_int_scx1_if_b.gb");
gbmicrotest!(test_hblank_int_scx1_if_c, "hblank_int_scx1_if_c.gb");
gbmicrotest!(test_hblank_int_scx1_if_d, "hblank_int_scx1_if_d.gb");
gbmicrotest!(test_hblank_int_scx1_nops_a, "hblank_int_scx1_nops_a.gb");
gbmicrotest!(test_hblank_int_scx1_nops_b, "hblank_int_scx1_nops_b.gb");
gbmicrotest!(test_hblank_int_scx2, "hblank_int_scx2.gb");
gbmicrotest!(test_hblank_int_scx2_if_a, "hblank_int_scx2_if_a.gb");
gbmicrotest!(test_hblank_int_scx2_if_b, "hblank_int_scx2_if_b.gb");
gbmicrotest!(test_hblank_int_scx2_if_c, "hblank_int_scx2_if_c.gb");
gbmicrotest!(test_hblank_int_scx2_if_d, "hblank_int_scx2_if_d.gb");
gbmicrotest!(test_hblank_int_scx2_nops_a, "hblank_int_scx2_nops_a.gb");
gbmicrotest!(test_hblank_int_scx2_nops_b, "hblank_int_scx2_nops_b.gb");
gbmicrotest!(test_hblank_int_scx3, "hblank_int_scx3.gb");
gbmicrotest!(test_hblank_int_scx3_if_a, "hblank_int_scx3_if_a.gb");
gbmicrotest!(test_hblank_int_scx3_if_b, "hblank_int_scx3_if_b.gb");
gbmicrotest!(test_hblank_int_scx3_if_c, "hblank_int_scx3_if_c.gb");
gbmicrotest!(test_hblank_int_scx3_if_d, "hblank_int_scx3_if_d.gb");
gbmicrotest!(test_hblank_int_scx3_nops_a, "hblank_int_scx3_nops_a.gb");
gbmicrotest!(test_hblank_int_scx3_nops_b, "hblank_int_scx3_nops_b.gb");
gbmicrotest!(test_hblank_int_scx4, "hblank_int_scx4.gb");
gbmicrotest!(test_hblank_int_scx4_if_a, "hblank_int_scx4_if_a.gb");
gbmicrotest!(test_hblank_int_scx4_if_b, "hblank_int_scx4_if_b.gb");
gbmicrotest!(test_hblank_int_scx4_if_c, "hblank_int_scx4_if_c.gb");
gbmicrotest!(test_hblank_int_scx4_if_d, "hblank_int_scx4_if_d.gb");
gbmicrotest!(test_hblank_int_scx4_nops_a, "hblank_int_scx4_nops_a.gb");
gbmicrotest!(test_hblank_int_scx4_nops_b, "hblank_int_scx4_nops_b.gb");
gbmicrotest!(test_hblank_int_scx5, "hblank_int_scx5.gb");
gbmicrotest!(test_hblank_int_scx5_if_a, "hblank_int_scx5_if_a.gb");
gbmicrotest!(test_hblank_int_scx5_if_b, "hblank_int_scx5_if_b.gb");
gbmicrotest!(test_hblank_int_scx5_if_c, "hblank_int_scx5_if_c.gb");
gbmicrotest!(test_hblank_int_scx5_if_d, "hblank_int_scx5_if_d.gb");
gbmicrotest!(test_hblank_int_scx5_nops_a, "hblank_int_scx5_nops_a.gb");
gbmicrotest!(test_hblank_int_scx5_nops_b, "hblank_int_scx5_nops_b.gb");
gbmicrotest!(test_hblank_int_scx6, "hblank_int_scx6.gb");
gbmicrotest!(test_hblank_int_scx6_if_a, "hblank_int_scx6_if_a.gb");
gbmicrotest!(test_hblank_int_scx6_if_b, "hblank_int_scx6_if_b.gb");
gbmicrotest!(test_hblank_int_scx6_if_c, "hblank_int_scx6_if_c.gb");
gbmicrotest!(test_hblank_int_scx6_if_d, "hblank_int_scx6_if_d.gb");
gbmicrotest!(test_hblank_int_scx6_nops_a, "hblank_int_scx6_nops_a.gb");
gbmicrotest!(test_hblank_int_scx6_nops_b, "hblank_int_scx6_nops_b.gb");
gbmicrotest!(test_hblank_int_scx7, "hblank_int_scx7.gb");
gbmicrotest!(test_hblank_int_scx7_if_a, "hblank_int_scx7_if_a.gb");
gbmicrotest!(test_hblank_int_scx7_if_b, "hblank_int_scx7_if_b.gb");
gbmicrotest!(test_hblank_int_scx7_if_c, "hblank_int_scx7_if_c.gb");
gbmicrotest!(test_hblank_int_scx7_if_d, "hblank_int_scx7_if_d.gb");
gbmicrotest!(test_hblank_int_scx7_nops_a, "hblank_int_scx7_nops_a.gb");
gbmicrotest!(test_hblank_int_scx7_nops_b, "hblank_int_scx7_nops_b.gb");
gbmicrotest!(test_hblank_scx2_if_a, "hblank_scx2_if_a.gb");
gbmicrotest!(test_hblank_scx3_if_a, "hblank_scx3_if_a.gb");
gbmicrotest!(test_hblank_scx3_if_b, "hblank_scx3_if_b.gb");
gbmicrotest!(test_hblank_scx3_if_c, "hblank_scx3_if_c.gb");
gbmicrotest!(test_hblank_scx3_if_d, "hblank_scx3_if_d.gb");
gbmicrotest!(test_hblank_scx3_int_a, "hblank_scx3_int_a.gb");
gbmicrotest!(test_hblank_scx3_int_b, "hblank_scx3_int_b.gb");
gbmicrotest!(test_int_hblank_halt_bug_a, "int_hblank_halt_bug_a.gb");
gbmicrotest!(test_int_hblank_halt_bug_b, "int_hblank_halt_bug_b.gb");
gbmicrotest!(test_int_hblank_halt_scx0, "int_hblank_halt_scx0.gb");
gbmicrotest!(test_int_hblank_halt_scx1, "int_hblank_halt_scx1.gb");
gbmicrotest!(test_int_hblank_halt_scx2, "int_hblank_halt_scx2.gb");
gbmicrotest!(test_int_hblank_halt_scx3, "int_hblank_halt_scx3.gb");
gbmicrotest!(test_int_hblank_halt_scx4, "int_hblank_halt_scx4.gb");
gbmicrotest!(test_int_hblank_halt_scx5, "int_hblank_halt_scx5.gb");
gbmicrotest!(test_int_hblank_halt_scx6, "int_hblank_halt_scx6.gb");
gbmicrotest!(test_int_hblank_halt_scx7, "int_hblank_halt_scx7.gb");
gbmicrotest!(test_int_hblank_incs_scx0, "int_hblank_incs_scx0.gb");
gbmicrotest!(test_int_hblank_incs_scx1, "int_hblank_incs_scx1.gb");
gbmicrotest!(test_int_hblank_incs_scx2, "int_hblank_incs_scx2.gb");
gbmicrotest!(test_int_hblank_incs_scx3, "int_hblank_incs_scx3.gb");
gbmicrotest!(test_int_hblank_incs_scx4, "int_hblank_incs_scx4.gb");
gbmicrotest!(test_int_hblank_incs_scx5, "int_hblank_incs_scx5.gb");
gbmicrotest!(test_int_hblank_incs_scx6, "int_hblank_incs_scx6.gb");
gbmicrotest!(test_int_hblank_incs_scx7, "int_hblank_incs_scx7.gb");
gbmicrotest!(test_int_hblank_nops_scx0, "int_hblank_nops_scx0.gb");
gbmicrotest!(test_int_hblank_nops_scx1, "int_hblank_nops_scx1.gb");
gbmicrotest!(test_int_hblank_nops_scx2, "int_hblank_nops_scx2.gb");
gbmicrotest!(test_int_hblank_nops_scx3, "int_hblank_nops_scx3.gb");
gbmicrotest!(test_int_hblank_nops_scx4, "int_hblank_nops_scx4.gb");
gbmicrotest!(test_int_hblank_nops_scx5, "int_hblank_nops_scx5.gb");
gbmicrotest!(test_int_hblank_nops_scx6, "int_hblank_nops_scx6.gb");
gbmicrotest!(test_int_hblank_nops_scx7, "int_hblank_nops_scx7.gb");
gbmicrotest!(test_int_lyc_halt, "int_lyc_halt.gb");
gbmicrotest!(test_int_lyc_incs, "int_lyc_incs.gb");
gbmicrotest!(test_int_lyc_nops, "int_lyc_nops.gb");
gbmicrotest!(test_int_oam_halt, "int_oam_halt.gb");
gbmicrotest!(test_int_oam_incs, "int_oam_incs.gb");
gbmicrotest!(test_int_oam_nops, "int_oam_nops.gb");
gbmicrotest!(test_int_timer_halt_div_a, "int_timer_halt_div_a.gb");
gbmicrotest!(test_int_timer_halt_div_b, "int_timer_halt_div_b.gb");
gbmicrotest!(test_int_timer_halt, "int_timer_halt.gb");
gbmicrotest!(test_int_timer_incs, "int_timer_incs.gb");
gbmicrotest!(test_int_timer_nops_div_a, "int_timer_nops_div_a.gb");
gbmicrotest!(test_int_timer_nops_div_b, "int_timer_nops_div_b.gb");
gbmicrotest!(test_int_timer_nops, "int_timer_nops.gb");
gbmicrotest!(test_int_vblank1_halt, "int_vblank1_halt.gb");
gbmicrotest!(test_int_vblank1_incs, "int_vblank1_incs.gb");
gbmicrotest!(test_int_vblank1_nops, "int_vblank1_nops.gb");
gbmicrotest!(test_int_vblank2_halt, "int_vblank2_halt.gb");
gbmicrotest!(test_int_vblank2_incs, "int_vblank2_incs.gb");
gbmicrotest!(test_int_vblank2_nops, "int_vblank2_nops.gb");
gbmicrotest!(
    test_is_if_set_during_ime0,
    "is_if_set_during_ime0.gb",
    frames = 24
);
gbmicrotest!(
    test_lcdon_halt_to_vblank_int_a,
    "lcdon_halt_to_vblank_int_a.gb"
);
gbmicrotest!(
    test_lcdon_halt_to_vblank_int_b,
    "lcdon_halt_to_vblank_int_b.gb"
);
gbmicrotest!(
    test_lcdon_nops_to_vblank_int_a,
    "lcdon_nops_to_vblank_int_a.gb"
);
gbmicrotest!(
    test_lcdon_nops_to_vblank_int_b,
    "lcdon_nops_to_vblank_int_b.gb"
);
gbmicrotest!(test_lcdon_to_if_oam_a, "lcdon_to_if_oam_a.gb");
gbmicrotest!(test_lcdon_to_if_oam_b, "lcdon_to_if_oam_b.gb");
gbmicrotest!(test_lcdon_to_ly1_a, "lcdon_to_ly1_a.gb");
gbmicrotest!(test_lcdon_to_ly1_b, "lcdon_to_ly1_b.gb");
gbmicrotest!(test_lcdon_to_ly2_a, "lcdon_to_ly2_a.gb");
gbmicrotest!(test_lcdon_to_ly2_b, "lcdon_to_ly2_b.gb");
gbmicrotest!(test_lcdon_to_ly3_a, "lcdon_to_ly3_a.gb");
gbmicrotest!(test_lcdon_to_ly3_b, "lcdon_to_ly3_b.gb");
gbmicrotest!(test_lcdon_to_lyc1_int, "lcdon_to_lyc1_int.gb");
gbmicrotest!(test_lcdon_to_lyc2_int, "lcdon_to_lyc2_int.gb");
gbmicrotest!(test_lcdon_to_lyc3_int, "lcdon_to_lyc3_int.gb");
gbmicrotest!(test_lcdon_to_oam_int_l0, "lcdon_to_oam_int_l0.gb");
gbmicrotest!(test_lcdon_to_oam_int_l1, "lcdon_to_oam_int_l1.gb");
gbmicrotest!(test_lcdon_to_oam_int_l2, "lcdon_to_oam_int_l2.gb");
gbmicrotest!(test_lcdon_to_oam_unlock_a, "lcdon_to_oam_unlock_a.gb");
gbmicrotest!(test_lcdon_to_oam_unlock_b, "lcdon_to_oam_unlock_b.gb");
gbmicrotest!(test_lcdon_to_oam_unlock_c, "lcdon_to_oam_unlock_c.gb");
gbmicrotest!(test_lcdon_to_oam_unlock_d, "lcdon_to_oam_unlock_d.gb");
gbmicrotest!(test_lcdon_to_stat0_a, "lcdon_to_stat0_a.gb");
gbmicrotest!(test_lcdon_to_stat0_b, "lcdon_to_stat0_b.gb");
gbmicrotest!(test_lcdon_to_stat0_c, "lcdon_to_stat0_c.gb");
gbmicrotest!(test_lcdon_to_stat0_d, "lcdon_to_stat0_d.gb");
gbmicrotest!(test_lcdon_to_stat1_a, "lcdon_to_stat1_a.gb");
gbmicrotest!(test_lcdon_to_stat1_b, "lcdon_to_stat1_b.gb");
gbmicrotest!(test_lcdon_to_stat1_c, "lcdon_to_stat1_c.gb");
gbmicrotest!(test_lcdon_to_stat1_d, "lcdon_to_stat1_d.gb");
gbmicrotest!(test_lcdon_to_stat1_e, "lcdon_to_stat1_e.gb");
gbmicrotest!(test_lcdon_to_stat2_a, "lcdon_to_stat2_a.gb");
gbmicrotest!(test_lcdon_to_stat2_b, "lcdon_to_stat2_b.gb");
gbmicrotest!(test_lcdon_to_stat2_c, "lcdon_to_stat2_c.gb");
gbmicrotest!(test_lcdon_to_stat2_d, "lcdon_to_stat2_d.gb");
gbmicrotest!(test_lcdon_to_stat3_a, "lcdon_to_stat3_a.gb");
gbmicrotest!(test_lcdon_to_stat3_b, "lcdon_to_stat3_b.gb");
gbmicrotest!(test_lcdon_to_stat3_c, "lcdon_to_stat3_c.gb");
gbmicrotest!(test_lcdon_to_stat3_d, "lcdon_to_stat3_d.gb");
gbmicrotest!(test_lcdon_write_timing, "lcdon_write_timing.gb");
gbmicrotest!(test_line_144_oam_int_a, "line_144_oam_int_a.gb");
gbmicrotest!(test_line_144_oam_int_b, "line_144_oam_int_b.gb");
gbmicrotest!(test_line_144_oam_int_c, "line_144_oam_int_c.gb");
gbmicrotest!(test_line_144_oam_int_d, "line_144_oam_int_d.gb");
gbmicrotest!(test_line_153_ly_a, "line_153_ly_a.gb");
gbmicrotest!(test_line_153_ly_b, "line_153_ly_b.gb");
gbmicrotest!(
    test_line_153_lyc0_int_inc_sled,
    "line_153_lyc0_int_inc_sled.gb"
);
gbmicrotest!(
    test_line_153_lyc0_stat_timing_a,
    "line_153_lyc0_stat_timing_a.gb"
);
gbmicrotest!(
    test_line_153_lyc0_stat_timing_b,
    "line_153_lyc0_stat_timing_b.gb"
);
gbmicrotest!(
    test_line_153_lyc0_stat_timing_c,
    "line_153_lyc0_stat_timing_c.gb"
);
gbmicrotest!(
    test_line_153_lyc0_stat_timing_d,
    "line_153_lyc0_stat_timing_d.gb"
);
gbmicrotest!(
    test_line_153_lyc0_stat_timing_e,
    "line_153_lyc0_stat_timing_e.gb"
);
gbmicrotest!(
    test_line_153_lyc0_stat_timing_f,
    "line_153_lyc0_stat_timing_f.gb"
);
gbmicrotest!(
    test_line_153_lyc0_stat_timing_g,
    "line_153_lyc0_stat_timing_g.gb"
);
gbmicrotest!(
    test_line_153_lyc0_stat_timing_h,
    "line_153_lyc0_stat_timing_h.gb"
);
gbmicrotest!(
    test_line_153_lyc0_stat_timing_i,
    "line_153_lyc0_stat_timing_i.gb"
);
gbmicrotest!(
    test_line_153_lyc0_stat_timing_j,
    "line_153_lyc0_stat_timing_j.gb"
);
gbmicrotest!(
    test_line_153_lyc0_stat_timing_k,
    "line_153_lyc0_stat_timing_k.gb"
);
gbmicrotest!(
    test_line_153_lyc0_stat_timing_l,
    "line_153_lyc0_stat_timing_l.gb"
);
gbmicrotest!(
    test_line_153_lyc0_stat_timing_m,
    "line_153_lyc0_stat_timing_m.gb"
);
gbmicrotest!(
    test_line_153_lyc0_stat_timing_n,
    "line_153_lyc0_stat_timing_n.gb"
);
gbmicrotest!(
    test_line_153_lyc153_stat_timing_a,
    "line_153_lyc153_stat_timing_a.gb"
);
gbmicrotest!(
    test_line_153_lyc153_stat_timing_b,
    "line_153_lyc153_stat_timing_b.gb"
);
gbmicrotest!(
    test_line_153_lyc153_stat_timing_c,
    "line_153_lyc153_stat_timing_c.gb"
);
gbmicrotest!(
    test_line_153_lyc153_stat_timing_d,
    "line_153_lyc153_stat_timing_d.gb"
);
gbmicrotest!(
    test_line_153_lyc153_stat_timing_e,
    "line_153_lyc153_stat_timing_e.gb"
);
gbmicrotest!(
    test_line_153_lyc153_stat_timing_f,
    "line_153_lyc153_stat_timing_f.gb"
);
gbmicrotest!(test_line_153_lyc_a, "line_153_lyc_a.gb");
gbmicrotest!(test_line_153_lyc_b, "line_153_lyc_b.gb");
gbmicrotest!(test_line_153_lyc_c, "line_153_lyc_c.gb");
gbmicrotest!(test_line_153_ly_c, "line_153_ly_c.gb");
gbmicrotest!(test_line_153_lyc_int_a, "line_153_lyc_int_a.gb");
gbmicrotest!(test_line_153_lyc_int_b, "line_153_lyc_int_b.gb");
gbmicrotest!(test_line_153_ly_d, "line_153_ly_d.gb");
gbmicrotest!(test_line_153_ly_e, "line_153_ly_e.gb");
gbmicrotest!(test_line_153_ly_f, "line_153_ly_f.gb");
gbmicrotest!(test_line_65_ly, "line_65_ly.gb");
gbmicrotest!(test_lyc1_int_halt_a, "lyc1_int_halt_a.gb");
gbmicrotest!(test_lyc1_int_halt_b, "lyc1_int_halt_b.gb");
gbmicrotest!(test_lyc1_int_if_edge_a, "lyc1_int_if_edge_a.gb");
gbmicrotest!(test_lyc1_int_if_edge_b, "lyc1_int_if_edge_b.gb");
gbmicrotest!(test_lyc1_int_if_edge_c, "lyc1_int_if_edge_c.gb");
gbmicrotest!(test_lyc1_int_if_edge_d, "lyc1_int_if_edge_d.gb");
gbmicrotest!(test_lyc1_int_nops_a, "lyc1_int_nops_a.gb");
gbmicrotest!(test_lyc1_int_nops_b, "lyc1_int_nops_b.gb");
gbmicrotest!(test_lyc1_write_timing_a, "lyc1_write_timing_a.gb");
gbmicrotest!(test_lyc1_write_timing_b, "lyc1_write_timing_b.gb");
gbmicrotest!(test_lyc1_write_timing_c, "lyc1_write_timing_c.gb");
gbmicrotest!(test_lyc1_write_timing_d, "lyc1_write_timing_d.gb");
gbmicrotest!(test_lyc2_int_halt_a, "lyc2_int_halt_a.gb");
gbmicrotest!(test_lyc2_int_halt_b, "lyc2_int_halt_b.gb");
gbmicrotest!(test_lyc_int_halt_a, "lyc_int_halt_a.gb");
gbmicrotest!(test_lyc_int_halt_b, "lyc_int_halt_b.gb");
gbmicrotest!(test_ly_while_lcd_off, "ly_while_lcd_off.gb");
gbmicrotest!(test_mbc1_ram_banks, "mbc1_ram_banks.gb");
gbmicrotest!(test_mbc1_rom_banks, "mbc1_rom_banks.gb");
gbmicrotest!(test_minimal, "minimal.gb");
gbmicrotest!(
    test_mode2_stat_int_to_oam_unlock,
    "mode2_stat_int_to_oam_unlock.gb"
);
gbmicrotest!(test_oam_int_halt_a, "oam_int_halt_a.gb");
gbmicrotest!(test_oam_int_halt_b, "oam_int_halt_b.gb");
gbmicrotest!(test_oam_int_if_edge_a, "oam_int_if_edge_a.gb");
gbmicrotest!(test_oam_int_if_edge_b, "oam_int_if_edge_b.gb");
gbmicrotest!(test_oam_int_if_edge_c, "oam_int_if_edge_c.gb");
gbmicrotest!(test_oam_int_if_edge_d, "oam_int_if_edge_d.gb");
gbmicrotest!(test_oam_int_if_level_c, "oam_int_if_level_c.gb");
gbmicrotest!(test_oam_int_if_level_d, "oam_int_if_level_d.gb");
gbmicrotest!(test_oam_int_inc_sled, "oam_int_inc_sled.gb");
gbmicrotest!(test_oam_int_nops_a, "oam_int_nops_a.gb");
gbmicrotest!(test_oam_int_nops_b, "oam_int_nops_b.gb");
gbmicrotest!(test_oam_read_l0_a, "oam_read_l0_a.gb");
gbmicrotest!(test_oam_read_l0_b, "oam_read_l0_b.gb");
gbmicrotest!(test_oam_read_l0_c, "oam_read_l0_c.gb");
gbmicrotest!(test_oam_read_l0_d, "oam_read_l0_d.gb");
gbmicrotest!(test_oam_read_l1_a, "oam_read_l1_a.gb");
gbmicrotest!(test_oam_read_l1_b, "oam_read_l1_b.gb");
gbmicrotest!(test_oam_read_l1_c, "oam_read_l1_c.gb");
gbmicrotest!(test_oam_read_l1_d, "oam_read_l1_d.gb");
gbmicrotest!(test_oam_read_l1_e, "oam_read_l1_e.gb");
gbmicrotest!(test_oam_read_l1_f, "oam_read_l1_f.gb");
gbmicrotest!(test_oam_sprite_trashing, "oam_sprite_trashing.gb");
gbmicrotest!(test_oam_write_l0_a, "oam_write_l0_a.gb");
gbmicrotest!(test_oam_write_l0_b, "oam_write_l0_b.gb");
gbmicrotest!(test_oam_write_l0_c, "oam_write_l0_c.gb");
gbmicrotest!(test_oam_write_l0_d, "oam_write_l0_d.gb");
gbmicrotest!(test_oam_write_l0_e, "oam_write_l0_e.gb");
gbmicrotest!(test_oam_write_l1_a, "oam_write_l1_a.gb");
gbmicrotest!(test_oam_write_l1_b, "oam_write_l1_b.gb");
gbmicrotest!(test_oam_write_l1_c, "oam_write_l1_c.gb");
gbmicrotest!(test_oam_write_l1_d, "oam_write_l1_d.gb");
gbmicrotest!(test_oam_write_l1_e, "oam_write_l1_e.gb");
gbmicrotest!(test_oam_write_l1_f, "oam_write_l1_f.gb");
gbmicrotest!(test_poweron_bgp_000, "poweron_bgp_000.gb");
gbmicrotest!(test_poweron_div_000, "poweron_div_000.gb");
gbmicrotest!(test_poweron_div_004, "poweron_div_004.gb");
gbmicrotest!(test_poweron_div_005, "poweron_div_005.gb");
gbmicrotest!(test_poweron_dma_000, "poweron_dma_000.gb");
gbmicrotest!(test_poweron, "poweron.gb");
gbmicrotest!(test_poweron_if_000, "poweron_if_000.gb");
gbmicrotest!(test_poweron_joy_000, "poweron_joy_000.gb");
gbmicrotest!(test_poweron_lcdc_000, "poweron_lcdc_000.gb");
gbmicrotest!(test_poweron_ly_000, "poweron_ly_000.gb");
gbmicrotest!(test_poweron_ly_119, "poweron_ly_119.gb");
gbmicrotest!(test_poweron_ly_120, "poweron_ly_120.gb");
gbmicrotest!(test_poweron_ly_233, "poweron_ly_233.gb");
gbmicrotest!(test_poweron_ly_234, "poweron_ly_234.gb");
gbmicrotest!(test_poweron_lyc_000, "poweron_lyc_000.gb");
gbmicrotest!(test_poweron_oam_000, "poweron_oam_000.gb");
gbmicrotest!(test_poweron_oam_005, "poweron_oam_005.gb");
gbmicrotest!(test_poweron_oam_006, "poweron_oam_006.gb");
gbmicrotest!(test_poweron_oam_069, "poweron_oam_069.gb");
gbmicrotest!(test_poweron_oam_070, "poweron_oam_070.gb");
gbmicrotest!(test_poweron_oam_119, "poweron_oam_119.gb");
gbmicrotest!(test_poweron_oam_120, "poweron_oam_120.gb");
gbmicrotest!(test_poweron_oam_121, "poweron_oam_121.gb");
gbmicrotest!(test_poweron_oam_183, "poweron_oam_183.gb");
gbmicrotest!(test_poweron_oam_184, "poweron_oam_184.gb");
gbmicrotest!(test_poweron_oam_233, "poweron_oam_233.gb");
gbmicrotest!(test_poweron_oam_234, "poweron_oam_234.gb");
gbmicrotest!(test_poweron_oam_235, "poweron_oam_235.gb");
gbmicrotest!(test_poweron_obp0_000, "poweron_obp0_000.gb");
gbmicrotest!(test_poweron_obp1_000, "poweron_obp1_000.gb");
gbmicrotest!(test_poweron_sb_000, "poweron_sb_000.gb");
gbmicrotest!(test_poweron_sc_000, "poweron_sc_000.gb");
gbmicrotest!(test_poweron_scx_000, "poweron_scx_000.gb");
gbmicrotest!(test_poweron_scy_000, "poweron_scy_000.gb");
gbmicrotest!(test_poweron_stat_000, "poweron_stat_000.gb");
gbmicrotest!(test_poweron_stat_005, "poweron_stat_005.gb");
gbmicrotest!(test_poweron_stat_006, "poweron_stat_006.gb");
gbmicrotest!(test_poweron_stat_007, "poweron_stat_007.gb");
gbmicrotest!(test_poweron_stat_026, "poweron_stat_026.gb");
gbmicrotest!(test_poweron_stat_027, "poweron_stat_027.gb");
gbmicrotest!(test_poweron_stat_069, "poweron_stat_069.gb");
gbmicrotest!(test_poweron_stat_070, "poweron_stat_070.gb");
gbmicrotest!(test_poweron_stat_119, "poweron_stat_119.gb");
gbmicrotest!(test_poweron_stat_120, "poweron_stat_120.gb");
gbmicrotest!(test_poweron_stat_121, "poweron_stat_121.gb");
gbmicrotest!(test_poweron_stat_140, "poweron_stat_140.gb");
gbmicrotest!(test_poweron_stat_141, "poweron_stat_141.gb");
gbmicrotest!(test_poweron_stat_183, "poweron_stat_183.gb");
gbmicrotest!(test_poweron_stat_184, "poweron_stat_184.gb");
gbmicrotest!(test_poweron_stat_234, "poweron_stat_234.gb");
gbmicrotest!(test_poweron_stat_235, "poweron_stat_235.gb");
gbmicrotest!(test_poweron_tac_000, "poweron_tac_000.gb");
gbmicrotest!(test_poweron_tima_000, "poweron_tima_000.gb");
gbmicrotest!(test_poweron_tma_000, "poweron_tma_000.gb");
gbmicrotest!(test_poweron_vram_000, "poweron_vram_000.gb");
gbmicrotest!(test_poweron_vram_025, "poweron_vram_025.gb");
gbmicrotest!(test_poweron_vram_026, "poweron_vram_026.gb");
gbmicrotest!(test_poweron_vram_069, "poweron_vram_069.gb");
gbmicrotest!(test_poweron_vram_070, "poweron_vram_070.gb");
gbmicrotest!(test_poweron_vram_139, "poweron_vram_139.gb");
gbmicrotest!(test_poweron_vram_140, "poweron_vram_140.gb");
gbmicrotest!(test_poweron_vram_183, "poweron_vram_183.gb");
gbmicrotest!(test_poweron_vram_184, "poweron_vram_184.gb");
gbmicrotest!(test_poweron_wx_000, "poweron_wx_000.gb");
gbmicrotest!(test_poweron_wy_000, "poweron_wy_000.gb");
gbmicrotest!(test_ppu_scx_vs_bgp, "ppu_scx_vs_bgp.gb");
gbmicrotest!(test_ppu_sprite0_scx0_a, "ppu_sprite0_scx0_a.gb");
gbmicrotest!(test_ppu_sprite0_scx0_b, "ppu_sprite0_scx0_b.gb");
gbmicrotest!(test_ppu_sprite0_scx1_a, "ppu_sprite0_scx1_a.gb");
gbmicrotest!(test_ppu_sprite0_scx1_b, "ppu_sprite0_scx1_b.gb");
gbmicrotest!(test_ppu_sprite0_scx2_a, "ppu_sprite0_scx2_a.gb");
gbmicrotest!(test_ppu_sprite0_scx2_b, "ppu_sprite0_scx2_b.gb");
gbmicrotest!(test_ppu_sprite0_scx3_a, "ppu_sprite0_scx3_a.gb");
gbmicrotest!(test_ppu_sprite0_scx3_b, "ppu_sprite0_scx3_b.gb");
gbmicrotest!(test_ppu_sprite0_scx4_a, "ppu_sprite0_scx4_a.gb");
gbmicrotest!(test_ppu_sprite0_scx4_b, "ppu_sprite0_scx4_b.gb");
gbmicrotest!(test_ppu_sprite0_scx5_a, "ppu_sprite0_scx5_a.gb");
gbmicrotest!(test_ppu_sprite0_scx5_b, "ppu_sprite0_scx5_b.gb");
gbmicrotest!(test_ppu_sprite0_scx6_a, "ppu_sprite0_scx6_a.gb");
gbmicrotest!(test_ppu_sprite0_scx6_b, "ppu_sprite0_scx6_b.gb");
gbmicrotest!(test_ppu_sprite0_scx7_a, "ppu_sprite0_scx7_a.gb");
gbmicrotest!(test_ppu_sprite0_scx7_b, "ppu_sprite0_scx7_b.gb");
gbmicrotest!(test_ppu_sprite_testbench, "ppu_sprite_testbench.gb");
gbmicrotest!(test_ppu_spritex_vs_scx, "ppu_spritex_vs_scx.gb");
gbmicrotest!(test_ppu_win_vs_wx, "ppu_win_vs_wx.gb");
gbmicrotest!(test_ppu_wx_early, "ppu_wx_early.gb");
gbmicrotest!(test_sprite_0_a, "sprite_0_a.gb");
gbmicrotest!(test_sprite_0_b, "sprite_0_b.gb");
gbmicrotest!(test_sprite_1_a, "sprite_1_a.gb");
gbmicrotest!(test_sprite_1_b, "sprite_1_b.gb");
gbmicrotest!(test_sprite4_0_a, "sprite4_0_a.gb");
gbmicrotest!(test_sprite4_0_b, "sprite4_0_b.gb");
gbmicrotest!(test_sprite4_1_a, "sprite4_1_a.gb");
gbmicrotest!(test_sprite4_1_b, "sprite4_1_b.gb");
gbmicrotest!(test_sprite4_2_a, "sprite4_2_a.gb");
gbmicrotest!(test_sprite4_2_b, "sprite4_2_b.gb");
gbmicrotest!(test_sprite4_3_a, "sprite4_3_a.gb");
gbmicrotest!(test_sprite4_3_b, "sprite4_3_b.gb");
gbmicrotest!(test_sprite4_4_a, "sprite4_4_a.gb");
gbmicrotest!(test_sprite4_4_b, "sprite4_4_b.gb");
gbmicrotest!(test_sprite4_5_a, "sprite4_5_a.gb");
gbmicrotest!(test_sprite4_5_b, "sprite4_5_b.gb");
gbmicrotest!(test_sprite4_6_a, "sprite4_6_a.gb");
gbmicrotest!(test_sprite4_6_b, "sprite4_6_b.gb");
gbmicrotest!(test_sprite4_7_a, "sprite4_7_a.gb");
gbmicrotest!(test_sprite4_7_b, "sprite4_7_b.gb");
gbmicrotest!(test_stat_write_glitch_l0_a, "stat_write_glitch_l0_a.gb");
gbmicrotest!(test_stat_write_glitch_l0_b, "stat_write_glitch_l0_b.gb");
gbmicrotest!(test_stat_write_glitch_l0_c, "stat_write_glitch_l0_c.gb");
gbmicrotest!(test_stat_write_glitch_l143_a, "stat_write_glitch_l143_a.gb");
gbmicrotest!(test_stat_write_glitch_l143_b, "stat_write_glitch_l143_b.gb");
gbmicrotest!(test_stat_write_glitch_l143_c, "stat_write_glitch_l143_c.gb");
gbmicrotest!(test_stat_write_glitch_l143_d, "stat_write_glitch_l143_d.gb");
gbmicrotest!(test_stat_write_glitch_l154_a, "stat_write_glitch_l154_a.gb");
gbmicrotest!(test_stat_write_glitch_l154_b, "stat_write_glitch_l154_b.gb");
gbmicrotest!(test_stat_write_glitch_l154_c, "stat_write_glitch_l154_c.gb");
gbmicrotest!(test_stat_write_glitch_l154_d, "stat_write_glitch_l154_d.gb");
gbmicrotest!(test_stat_write_glitch_l1_a, "stat_write_glitch_l1_a.gb");
gbmicrotest!(test_stat_write_glitch_l1_b, "stat_write_glitch_l1_b.gb");
gbmicrotest!(test_stat_write_glitch_l1_c, "stat_write_glitch_l1_c.gb");
gbmicrotest!(test_stat_write_glitch_l1_d, "stat_write_glitch_l1_d.gb");
gbmicrotest!(test_temp, "temp.gb");
gbmicrotest!(test_timer_div_phase_c, "timer_div_phase_c.gb");
gbmicrotest!(test_timer_div_phase_d, "timer_div_phase_d.gb");
gbmicrotest!(test_timer_tima_inc_256k_a, "timer_tima_inc_256k_a.gb");
gbmicrotest!(test_timer_tima_inc_256k_b, "timer_tima_inc_256k_b.gb");
gbmicrotest!(test_timer_tima_inc_256k_c, "timer_tima_inc_256k_c.gb");
gbmicrotest!(test_timer_tima_inc_256k_d, "timer_tima_inc_256k_d.gb");
gbmicrotest!(test_timer_tima_inc_256k_e, "timer_tima_inc_256k_e.gb");
gbmicrotest!(test_timer_tima_inc_256k_f, "timer_tima_inc_256k_f.gb");
gbmicrotest!(test_timer_tima_inc_256k_g, "timer_tima_inc_256k_g.gb");
gbmicrotest!(test_timer_tima_inc_256k_h, "timer_tima_inc_256k_h.gb");
gbmicrotest!(test_timer_tima_inc_256k_i, "timer_tima_inc_256k_i.gb");
gbmicrotest!(test_timer_tima_inc_256k_j, "timer_tima_inc_256k_j.gb");
gbmicrotest!(test_timer_tima_inc_256k_k, "timer_tima_inc_256k_k.gb");
gbmicrotest!(test_timer_tima_inc_64k_a, "timer_tima_inc_64k_a.gb");
gbmicrotest!(test_timer_tima_inc_64k_b, "timer_tima_inc_64k_b.gb");
gbmicrotest!(test_timer_tima_inc_64k_c, "timer_tima_inc_64k_c.gb");
gbmicrotest!(test_timer_tima_inc_64k_d, "timer_tima_inc_64k_d.gb");
gbmicrotest!(test_timer_tima_phase_a, "timer_tima_phase_a.gb");
gbmicrotest!(test_timer_tima_phase_b, "timer_tima_phase_b.gb");
gbmicrotest!(test_timer_tima_phase_c, "timer_tima_phase_c.gb");
gbmicrotest!(test_timer_tima_phase_d, "timer_tima_phase_d.gb");
gbmicrotest!(test_timer_tima_phase_e, "timer_tima_phase_e.gb");
gbmicrotest!(test_timer_tima_phase_f, "timer_tima_phase_f.gb");
gbmicrotest!(test_timer_tima_phase_g, "timer_tima_phase_g.gb");
gbmicrotest!(test_timer_tima_phase_h, "timer_tima_phase_h.gb");
gbmicrotest!(test_timer_tima_phase_i, "timer_tima_phase_i.gb");
gbmicrotest!(test_timer_tima_phase_j, "timer_tima_phase_j.gb");
gbmicrotest!(test_timer_tima_reload_256k_a, "timer_tima_reload_256k_a.gb");
gbmicrotest!(test_timer_tima_reload_256k_b, "timer_tima_reload_256k_b.gb");
gbmicrotest!(test_timer_tima_reload_256k_c, "timer_tima_reload_256k_c.gb");
gbmicrotest!(test_timer_tima_reload_256k_d, "timer_tima_reload_256k_d.gb");
gbmicrotest!(test_timer_tima_reload_256k_e, "timer_tima_reload_256k_e.gb");
gbmicrotest!(test_timer_tima_reload_256k_f, "timer_tima_reload_256k_f.gb");
gbmicrotest!(test_timer_tima_reload_256k_g, "timer_tima_reload_256k_g.gb");
gbmicrotest!(test_timer_tima_reload_256k_h, "timer_tima_reload_256k_h.gb");
gbmicrotest!(test_timer_tima_reload_256k_i, "timer_tima_reload_256k_i.gb");
gbmicrotest!(test_timer_tima_reload_256k_j, "timer_tima_reload_256k_j.gb");
gbmicrotest!(test_timer_tima_reload_256k_k, "timer_tima_reload_256k_k.gb");
gbmicrotest!(test_timer_tima_write_a, "timer_tima_write_a.gb");
gbmicrotest!(test_timer_tima_write_b, "timer_tima_write_b.gb");
gbmicrotest!(test_timer_tima_write_c, "timer_tima_write_c.gb");
gbmicrotest!(test_timer_tima_write_d, "timer_tima_write_d.gb");
gbmicrotest!(test_timer_tima_write_e, "timer_tima_write_e.gb");
gbmicrotest!(test_timer_tima_write_f, "timer_tima_write_f.gb");
gbmicrotest!(test_timer_tma_write_a, "timer_tma_write_a.gb");
gbmicrotest!(test_timer_tma_write_b, "timer_tma_write_b.gb");
gbmicrotest!(test_toggle_lcdc, "toggle_lcdc.gb");
gbmicrotest!(test_vblank2_int_halt_a, "vblank2_int_halt_a.gb");
gbmicrotest!(test_vblank2_int_halt_b, "vblank2_int_halt_b.gb");
gbmicrotest!(test_vblank2_int_if_a, "vblank2_int_if_a.gb");
gbmicrotest!(test_vblank2_int_if_b, "vblank2_int_if_b.gb");
gbmicrotest!(test_vblank2_int_if_c, "vblank2_int_if_c.gb");
gbmicrotest!(test_vblank2_int_if_d, "vblank2_int_if_d.gb");
gbmicrotest!(test_vblank2_int_inc_sled, "vblank2_int_inc_sled.gb");
gbmicrotest!(test_vblank2_int_nops_a, "vblank2_int_nops_a.gb");
gbmicrotest!(test_vblank2_int_nops_b, "vblank2_int_nops_b.gb");
gbmicrotest!(test_vblank_int_halt_a, "vblank_int_halt_a.gb");
gbmicrotest!(test_vblank_int_halt_b, "vblank_int_halt_b.gb");
gbmicrotest!(test_vblank_int_if_a, "vblank_int_if_a.gb");
gbmicrotest!(test_vblank_int_if_b, "vblank_int_if_b.gb");
gbmicrotest!(test_vblank_int_if_c, "vblank_int_if_c.gb");
gbmicrotest!(test_vblank_int_if_d, "vblank_int_if_d.gb");
gbmicrotest!(test_vblank_int_inc_sled, "vblank_int_inc_sled.gb");
gbmicrotest!(test_vblank_int_nops_a, "vblank_int_nops_a.gb");
gbmicrotest!(test_vblank_int_nops_b, "vblank_int_nops_b.gb");
gbmicrotest!(test_vram_read_l0_a, "vram_read_l0_a.gb");
gbmicrotest!(test_vram_read_l0_b, "vram_read_l0_b.gb");
gbmicrotest!(test_vram_read_l0_c, "vram_read_l0_c.gb");
gbmicrotest!(test_vram_read_l0_d, "vram_read_l0_d.gb");
gbmicrotest!(test_vram_read_l1_a, "vram_read_l1_a.gb");
gbmicrotest!(test_vram_read_l1_b, "vram_read_l1_b.gb");
gbmicrotest!(test_vram_read_l1_c, "vram_read_l1_c.gb");
gbmicrotest!(test_vram_read_l1_d, "vram_read_l1_d.gb");
gbmicrotest!(test_vram_write_l0_a, "vram_write_l0_a.gb");
gbmicrotest!(test_vram_write_l0_b, "vram_write_l0_b.gb");
gbmicrotest!(test_vram_write_l0_c, "vram_write_l0_c.gb");
gbmicrotest!(test_vram_write_l0_d, "vram_write_l0_d.gb");
gbmicrotest!(test_vram_write_l1_a, "vram_write_l1_a.gb");
gbmicrotest!(test_vram_write_l1_b, "vram_write_l1_b.gb");
gbmicrotest!(test_vram_write_l1_c, "vram_write_l1_c.gb");
gbmicrotest!(test_vram_write_l1_d, "vram_write_l1_d.gb");
gbmicrotest!(test_wave_write_to_0xC003, "wave_write_to_0xC003.gb");
gbmicrotest!(test_win0_a, "win0_a.gb");
gbmicrotest!(test_win0_b, "win0_b.gb");
gbmicrotest!(test_win0_scx3_a, "win0_scx3_a.gb");
gbmicrotest!(test_win0_scx3_b, "win0_scx3_b.gb");
gbmicrotest!(test_win10_a, "win10_a.gb");
gbmicrotest!(test_win10_b, "win10_b.gb");
gbmicrotest!(test_win10_scx3_a, "win10_scx3_a.gb");
gbmicrotest!(test_win10_scx3_b, "win10_scx3_b.gb");
gbmicrotest!(test_win11_a, "win11_a.gb");
gbmicrotest!(test_win11_b, "win11_b.gb");
gbmicrotest!(test_win12_a, "win12_a.gb");
gbmicrotest!(test_win12_b, "win12_b.gb");
gbmicrotest!(test_win13_a, "win13_a.gb");
gbmicrotest!(test_win13_b, "win13_b.gb");
gbmicrotest!(test_win14_a, "win14_a.gb");
gbmicrotest!(test_win14_b, "win14_b.gb");
gbmicrotest!(test_win15_a, "win15_a.gb");
gbmicrotest!(test_win15_b, "win15_b.gb");
gbmicrotest!(test_win1_a, "win1_a.gb");
gbmicrotest!(test_win1_b, "win1_b.gb");
gbmicrotest!(test_win2_a, "win2_a.gb");
gbmicrotest!(test_win2_b, "win2_b.gb");
gbmicrotest!(test_win3_a, "win3_a.gb");
gbmicrotest!(test_win3_b, "win3_b.gb");
gbmicrotest!(test_win4_a, "win4_a.gb");
gbmicrotest!(test_win4_b, "win4_b.gb");
gbmicrotest!(test_win5_a, "win5_a.gb");
gbmicrotest!(test_win5_b, "win5_b.gb");
gbmicrotest!(test_win6_a, "win6_a.gb");
gbmicrotest!(test_win6_b, "win6_b.gb");
gbmicrotest!(test_win7_a, "win7_a.gb");
gbmicrotest!(test_win7_b, "win7_b.gb");
gbmicrotest!(test_win8_a, "win8_a.gb");
gbmicrotest!(test_win8_b, "win8_b.gb");
gbmicrotest!(test_win9_a, "win9_a.gb");
gbmicrotest!(test_win9_b, "win9_b.gb");
