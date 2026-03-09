use ceres_core::Model;
use ceres_test_runner::{
    expected_screenshot_path, load_test_rom,
    test_runner::{
        timeouts, CompletionCheck, DummyAudioCallback, ScreenshotCheck, TestConfig, TestResult,
        TestRunner,
    },
};

/// Check for Mooneye test completion (register values on ld b,b)
pub struct MooneyeCheck;

impl CompletionCheck for MooneyeCheck {
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
                "Mooneye failure: B={b:#04X}, C={c:#04X}, D={d:#04X}, E={e:#04X}, H={h:#04X}, L={l:#04X}"
            )));
        }

        None
    }

    fn on_timeout(&self, _gb: &mut ceres_core::Gb<DummyAudioCallback>) -> TestResult {
        TestResult::Failed("Mooneye test timed out".to_string())
    }
}

/// Helper function to run a Mooneye acceptance test
fn run_mooneye_test(path: &str, model: Model) -> TestResult {
    let rom = match load_test_rom(path) {
        Ok(rom) => rom,
        Err(e) => return TestResult::Error(format!("Failed to load test ROM: {e}")),
    };

    let config = TestConfig {
        model,
        timeout_frames: timeouts::MOONEYE_ACCEPTANCE,
        ..TestConfig::default()
    };

    let mut runner = match TestRunner::new(rom, config, Box::new(MooneyeCheck)) {
        Ok(runner) => runner,
        Err(e) => return TestResult::Error(format!("Failed to create test runner: {e}")),
    };

    runner.run()
}

/// Helper function to run a Mooneye screenshot test
fn run_mooneye_screenshot_test(path: &str, model: Model) -> TestResult {
    let rom = match load_test_rom(path) {
        Ok(rom) => rom,
        Err(e) => return TestResult::Error(format!("Failed to load test ROM: {e}")),
    };

    let Some(screenshot_path) = expected_screenshot_path(path, model) else {
        return TestResult::Error("Expected screenshot not found".to_string());
    };

    let config = TestConfig {
        model,
        timeout_frames: timeouts::MOONEYE_ACCEPTANCE,
        ..TestConfig::default()
    };

    let mut runner =
        match TestRunner::new(rom, config, Box::new(ScreenshotCheck::new(screenshot_path))) {
            Ok(runner) => runner,
            Err(e) => return TestResult::Error(format!("Failed to create test runner: {e}")),
        };

    runner.run()
}

// =============================================================================
// Root Level Tests
// =============================================================================

#[test]
fn test_mooneye_add_sp_e_timing() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/add_sp_e_timing.gb",
        Model::CgbE,
    );
    assert!(result.is_passed(), "add_sp_e_timing test failed");
}

#[test]
fn test_mooneye_call_cc_timing() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/call_cc_timing.gb",
        Model::CgbE,
    );
    assert!(result.is_passed(), "call_cc_timing test failed");
}

#[test]
fn test_mooneye_call_cc_timing2() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/call_cc_timing2.gb",
        Model::CgbE,
    );
    assert!(result.is_passed(), "call_cc_timing2 test failed");
}

#[test]
fn test_mooneye_call_timing() {
    let result = run_mooneye_test("mooneye-test-suite/acceptance/call_timing.gb", Model::CgbE);
    assert!(result.is_passed(), "call_timing test failed");
}

#[test]
fn test_mooneye_call_timing2() {
    let result = run_mooneye_test("mooneye-test-suite/acceptance/call_timing2.gb", Model::CgbE);
    assert!(result.is_passed(), "call_timing2 test failed");
}

#[test]
fn test_mooneye_di_timing_gs() {
    let result = run_mooneye_test("mooneye-test-suite/acceptance/di_timing-GS.gb", Model::DmgB);
    assert!(result.is_passed(), "di_timing-GS test failed");
}

#[test]
fn test_mooneye_div_timing() {
    let result = run_mooneye_test("mooneye-test-suite/acceptance/div_timing.gb", Model::CgbE);
    assert!(result.is_passed(), "div_timing test failed");
}

#[test]
fn test_mooneye_ei_sequence() {
    let result = run_mooneye_test("mooneye-test-suite/acceptance/ei_sequence.gb", Model::CgbE);
    assert!(result.is_passed(), "ei_sequence test failed");
}

#[test]
fn test_mooneye_ei_timing() {
    let result = run_mooneye_test("mooneye-test-suite/acceptance/ei_timing.gb", Model::CgbE);
    assert!(result.is_passed(), "ei_timing test failed");
}

#[test]
fn test_mooneye_halt_ime0_ei() {
    let result = run_mooneye_test("mooneye-test-suite/acceptance/halt_ime0_ei.gb", Model::CgbE);
    assert!(result.is_passed(), "halt_ime0_ei test failed");
}

#[test]
fn test_mooneye_halt_ime0_nointr_timing() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/halt_ime0_nointr_timing.gb",
        Model::CgbE,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "halt_ime0_nointr_timing test failed"
    );
}

#[test]
fn test_mooneye_halt_ime1_timing() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/halt_ime1_timing.gb",
        Model::CgbE,
    );
    assert!(result.is_passed(), "halt_ime1_timing test failed");
}

#[test]
fn test_mooneye_halt_ime1_timing2_gs() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/halt_ime1_timing2-GS.gb",
        Model::DmgB,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "halt_ime1_timing2-GS test failed"
    );
}

#[test]
fn test_mooneye_if_ie_registers() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/if_ie_registers.gb",
        Model::CgbE,
    );
    assert!(result.is_passed(), "if_ie_registers test failed");
}

#[test]
fn test_mooneye_intr_timing() {
    let result = run_mooneye_test("mooneye-test-suite/acceptance/intr_timing.gb", Model::CgbE);
    assert!(result.is_passed(), "intr_timing test failed");
}

#[test]
fn test_mooneye_jp_cc_timing() {
    let result = run_mooneye_test("mooneye-test-suite/acceptance/jp_cc_timing.gb", Model::CgbE);
    assert!(result.is_passed(), "jp_cc_timing test failed");
}

#[test]
fn test_mooneye_jp_timing() {
    let result = run_mooneye_test("mooneye-test-suite/acceptance/jp_timing.gb", Model::CgbE);
    assert!(result.is_passed(), "jp_timing test failed");
}

#[test]
fn test_mooneye_ld_hl_sp_e_timing() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/ld_hl_sp_e_timing.gb",
        Model::CgbE,
    );
    assert!(result.is_passed(), "ld_hl_sp_e_timing test failed");
}

#[test]
fn test_mooneye_oam_dma_restart() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/oam_dma_restart.gb",
        Model::CgbE,
    );
    assert!(result.is_passed(), "oam_dma_restart test failed");
}

#[test]
fn test_mooneye_oam_dma_start() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/oam_dma_start.gb",
        Model::CgbE,
    );
    assert!(result.is_passed(), "oam_dma_start test failed");
}

#[test]
fn test_mooneye_oam_dma_timing() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/oam_dma_timing.gb",
        Model::CgbE,
    );
    assert!(result.is_passed(), "oam_dma_timing test failed");
}

#[test]
fn test_mooneye_pop_timing() {
    let result = run_mooneye_test("mooneye-test-suite/acceptance/pop_timing.gb", Model::CgbE);
    assert!(result.is_passed(), "pop_timing test failed");
}

#[test]
fn test_mooneye_push_timing() {
    let result = run_mooneye_test("mooneye-test-suite/acceptance/push_timing.gb", Model::CgbE);
    assert!(result.is_passed(), "push_timing test failed");
}

#[test]
fn test_mooneye_rapid_di_ei() {
    let result = run_mooneye_test("mooneye-test-suite/acceptance/rapid_di_ei.gb", Model::CgbE);
    assert!(result.is_passed(), "rapid_di_ei test failed");
}

#[test]
fn test_mooneye_ret_cc_timing() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/ret_cc_timing.gb",
        Model::CgbE,
    );
    assert!(result.is_passed(), "ret_cc_timing test failed");
}

#[test]
fn test_mooneye_ret_timing() {
    let result = run_mooneye_test("mooneye-test-suite/acceptance/ret_timing.gb", Model::CgbE);
    assert!(result.is_passed(), "ret_timing test failed");
}

#[test]
fn test_mooneye_reti_intr_timing() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/reti_intr_timing.gb",
        Model::CgbE,
    );
    assert!(result.is_passed(), "reti_intr_timing test failed");
}

#[test]
fn test_mooneye_reti_timing() {
    let result = run_mooneye_test("mooneye-test-suite/acceptance/reti_timing.gb", Model::CgbE);
    assert!(result.is_passed(), "reti_timing test failed");
}

// Boot register tests - model-specific
#[test]
fn test_mooneye_boot_div_cgbabcde() {
    let result = run_mooneye_test("mooneye-test-suite/misc/boot_div-cgbABCDE.gb", Model::CgbE);
    assert!(
        result.is_passed(),
        "boot_div-cgbABCDE test failed: {:?}",
        result
    );
}

#[test]
fn test_mooneye_boot_div_dmgabcmgb() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/boot_div-dmgABCmgb.gb",
        Model::DmgB,
    );
    assert!(
        result.is_passed(),
        "boot_div-dmgABCmgb test failed: {:?}",
        result
    );
}

#[test]
#[ignore = "SGB not yet supported"]
fn test_mooneye_boot_div_s() {
    let result = run_mooneye_test("mooneye-test-suite/acceptance/boot_div-S.gb", Model::DmgB);
    assert!(result.is_passed(), "boot_div-S test failed");
}

#[test]
#[ignore = "SGB not yet supported"]
fn test_mooneye_boot_div2_s() {
    let result = run_mooneye_test("mooneye-test-suite/acceptance/boot_div2-S.gb", Model::DmgB);
    assert!(result.is_passed(), "boot_div2-S test failed");
}

#[test]
#[ignore = "DMG 0 not supported yet"]
fn test_mooneye_boot_hwio_dmg0() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/boot_hwio-dmg0.gb",
        Model::DmgB,
    );
    assert!(result.is_passed(), "boot_hwio-dmg0 test failed");
}

#[test]
#[ignore = "Expect A and B to be pressed after boot?"]
fn test_mooneye_boot_hwio_dmgabcmgb() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/boot_hwio-dmgABCmgb.gb",
        Model::DmgB,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "boot_hwio-dmgABCmgb test failed"
    );
}

#[test]
#[ignore = "SGB not yet supported"]
fn test_mooneye_boot_hwio_s() {
    let result = run_mooneye_test("mooneye-test-suite/acceptance/boot_hwio-S.gb", Model::DmgB);
    assert!(result.is_passed(), "boot_hwio-S test failed");
}

#[test]
#[ignore] // TODO: Enable when passing - DMG CPU revision 0
fn test_mooneye_boot_regs_dmg0() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/boot_regs-dmg0.gb",
        Model::DmgB,
    );
    assert!(result.is_passed(), "boot_regs-dmg0 test failed");
}

#[test]
fn test_mooneye_boot_regs_dmgabc() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/boot_regs-dmgABC.gb",
        Model::DmgB,
    );
    assert!(result.is_passed(), "boot_regs-dmgABC test failed");
}

#[test]
fn test_mooneye_boot_regs_mgb() {
    let result = run_mooneye_test("mooneye-test-suite/acceptance/boot_regs-mgb.gb", Model::Mgb);
    assert!(result.is_passed(), "boot_regs-mgb test failed");
}

#[test]
#[ignore] // TODO: Enable when passing - SGB hint
fn test_mooneye_boot_regs_sgb() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/boot_regs-sgb.gb",
        Model::DmgB,
    );
    assert!(result.is_passed(), "boot_regs-sgb test failed");
}

#[test]
#[ignore] // TODO: Enable when passing - SGB2 hint
fn test_mooneye_boot_regs_sgb2() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/boot_regs-sgb2.gb",
        Model::DmgB,
    );
    assert!(result.is_passed(), "boot_regs-sgb2 test failed");
}

// =============================================================================
// bits/ Tests
// =============================================================================

#[test]
fn test_mooneye_bits_mem_oam() {
    let result = run_mooneye_test("mooneye-test-suite/acceptance/bits/mem_oam.gb", Model::CgbE);
    assert!(result.is_passed(), "bits/mem_oam test failed");
}

#[test]
fn test_mooneye_bits_reg_f() {
    let result = run_mooneye_test("mooneye-test-suite/acceptance/bits/reg_f.gb", Model::CgbE);
    assert!(result.is_passed(), "bits/reg_f test failed");
}

#[test]
fn test_mooneye_bits_unused_hwio_gs() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/bits/unused_hwio-GS.gb",
        Model::DmgB,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "bits/unused_hwio-GS test failed"
    );
}

// =============================================================================
// instr/ Tests
// =============================================================================

#[test]
fn test_mooneye_instr_daa() {
    let result = run_mooneye_test("mooneye-test-suite/acceptance/instr/daa.gb", Model::CgbE);
    assert!(result.is_passed(), "instr/daa test failed");
}

// =============================================================================
// interrupts/ Tests
// =============================================================================

#[test]
fn test_mooneye_interrupts_ie_push() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/interrupts/ie_push.gb",
        Model::CgbE,
    );
    assert!(result.is_passed(), "interrupts/ie_push test failed");
}

// =============================================================================
// oam_dma/ Tests
// =============================================================================

#[test]
fn test_mooneye_oam_dma_basic() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/oam_dma/basic.gb",
        Model::CgbE,
    );
    assert!(result.is_passed(), "oam_dma/basic test failed");
}

#[test]
fn test_mooneye_oam_dma_reg_read() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/oam_dma/reg_read.gb",
        Model::CgbE,
    );
    assert!(result.is_passed(), "oam_dma/reg_read test failed");
}

#[test]
fn test_mooneye_oam_dma_sources_gs() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/oam_dma/sources-GS.gb",
        Model::DmgB,
    );
    assert!(result.is_passed(), "oam_dma/sources-GS test failed");
}

// =============================================================================
// ppu/ Tests
// =============================================================================

#[test]
#[ignore]
fn test_mooneye_ppu_hblank_ly_scx_timing_gs() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/ppu/hblank_ly_scx_timing-GS.gb",
        Model::DmgB,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "ppu/hblank_ly_scx_timing-GS test failed"
    );
}

#[test]
#[ignore]
fn test_mooneye_ppu_intr_1_2_timing_gs() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/ppu/intr_1_2_timing-GS.gb",
        Model::DmgB,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "ppu/intr_1_2_timing-GS test failed"
    );
}

#[test]
#[ignore]
fn test_mooneye_ppu_intr_2_0_timing() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/ppu/intr_2_0_timing.gb",
        Model::DmgB,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "ppu/intr_2_0_timing test failed"
    );
}

#[test]
#[ignore]
fn test_mooneye_ppu_intr_2_mode0_timing() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/ppu/intr_2_mode0_timing.gb",
        Model::DmgB,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "ppu/intr_2_mode0_timing test failed"
    );
}

#[test]
#[ignore]
fn test_mooneye_ppu_intr_2_mode0_timing_sprites() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/ppu/intr_2_mode0_timing_sprites.gb",
        Model::DmgB,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "ppu/intr_2_mode0_timing_sprites test failed"
    );
}

#[test]
#[ignore]
fn test_mooneye_ppu_intr_2_mode3_timing() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/ppu/intr_2_mode3_timing.gb",
        Model::DmgB,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "ppu/intr_2_mode3_timing test failed"
    );
}

#[test]
#[ignore]
fn test_mooneye_ppu_intr_2_oam_ok_timing() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/ppu/intr_2_oam_ok_timing.gb",
        Model::DmgB,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "ppu/intr_2_oam_ok_timing test failed"
    );
}

#[test]
#[ignore]
fn test_mooneye_ppu_lcdon_timing_gs() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/ppu/lcdon_timing-GS.gb",
        Model::DmgB,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "ppu/lcdon_timing-GS test failed"
    );
}

#[test]
#[ignore]
fn test_mooneye_ppu_lcdon_write_timing_gs() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/ppu/lcdon_write_timing-GS.gb",
        Model::DmgB,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "ppu/lcdon_write_timing-GS test failed"
    );
}

#[test]
fn test_mooneye_ppu_stat_irq_blocking() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/ppu/stat_irq_blocking.gb",
        Model::DmgB,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "ppu/stat_irq_blocking test failed"
    );
}

#[test]
fn test_mooneye_ppu_stat_lyc_onoff() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/ppu/stat_lyc_onoff.gb",
        Model::DmgB,
    );
    assert!(result.is_passed(), "ppu/stat_lyc_onoff test failed");
}

#[test]
fn test_mooneye_ppu_vblank_stat_intr_gs() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/ppu/vblank_stat_intr-GS.gb",
        Model::DmgB,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "ppu/vblank_stat_intr-GS test failed"
    );
}

// =============================================================================
// serial/ Tests
// =============================================================================

#[test]
fn test_mooneye_serial_boot_sclk_align_dmgabcmgb() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/serial/boot_sclk_align-dmgABCmgb.gb",
        Model::DmgB,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "serial/boot_sclk_align-dmgABCmgb test failed"
    );
}

// =============================================================================
// timer/ Tests
// =============================================================================

#[test]
fn test_mooneye_timer_div_write() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/timer/div_write.gb",
        Model::CgbE,
    );
    assert!(result.is_passed(), "timer/div_write test failed");
}

#[test]
#[ignore = "timer/rapid_toggle: BC at interrupt time does not match expected $FFD9 — TAC glitch fires but loop timing is off"]
fn test_mooneye_timer_rapid_toggle() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/timer/rapid_toggle.gb",
        Model::CgbE,
    );
    assert!(result.is_passed(), "timer/rapid_toggle test failed");
}

#[test]
fn test_mooneye_timer_tim00() {
    let result = run_mooneye_test("mooneye-test-suite/acceptance/timer/tim00.gb", Model::CgbE);
    assert!(result.is_passed(), "timer/tim00 test failed");
}

#[test]
fn test_mooneye_timer_tim00_div_trigger() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/timer/tim00_div_trigger.gb",
        Model::CgbE,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "timer/tim00_div_trigger test failed"
    );
}

#[test]
fn test_mooneye_timer_tim01() {
    let result = run_mooneye_test("mooneye-test-suite/acceptance/timer/tim01.gb", Model::CgbE);
    assert!(result.is_passed(), "timer/tim01 test failed");
}

#[test]
fn test_mooneye_timer_tim01_div_trigger() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/timer/tim01_div_trigger.gb",
        Model::CgbE,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "timer/tim01_div_trigger test failed"
    );
}

#[test]
fn test_mooneye_timer_tim10() {
    let result = run_mooneye_test("mooneye-test-suite/acceptance/timer/tim10.gb", Model::CgbE);
    assert!(result.is_passed(), "timer/tim10 test failed");
}

#[test]
fn test_mooneye_timer_tim10_div_trigger() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/timer/tim10_div_trigger.gb",
        Model::CgbE,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "timer/tim10_div_trigger test failed"
    );
}

#[test]
fn test_mooneye_timer_tim11() {
    let result = run_mooneye_test("mooneye-test-suite/acceptance/timer/tim11.gb", Model::CgbE);
    assert!(result.is_passed(), "timer/tim11 test failed");
}

#[test]
fn test_mooneye_timer_tim11_div_trigger() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/timer/tim11_div_trigger.gb",
        Model::CgbE,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "timer/tim11_div_trigger test failed"
    );
}

#[test]
fn test_mooneye_timer_tima_reload() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/timer/tima_reload.gb",
        Model::CgbE,
    );
    assert!(result.is_passed(), "timer/tima_reload test failed");
}

#[test]
fn test_mooneye_timer_tima_write_reloading() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/timer/tima_write_reloading.gb",
        Model::CgbE,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "timer/tima_write_reloading test failed"
    );
}

#[test]
fn test_mooneye_timer_tma_write_reloading() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/timer/tma_write_reloading.gb",
        Model::CgbE,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "timer/tma_write_reloading test failed"
    );
}

// =============================================================================
// Emulator-Only MBC1 Tests
// =============================================================================

#[test]
fn test_mooneye_mbc1_bits_bank1() {
    let result = run_mooneye_test(
        "mooneye-test-suite/emulator-only/mbc1/bits_bank1.gb",
        Model::CgbE,
    );
    assert!(result.is_passed(), "mbc1/bits_bank1 test failed");
}

#[test]
fn test_mooneye_mbc1_bits_bank2() {
    let result = run_mooneye_test(
        "mooneye-test-suite/emulator-only/mbc1/bits_bank2.gb",
        Model::CgbE,
    );
    assert!(result.is_passed(), "mbc1/bits_bank2 test failed");
}

#[test]
fn test_mooneye_mbc1_bits_mode() {
    let result = run_mooneye_test(
        "mooneye-test-suite/emulator-only/mbc1/bits_mode.gb",
        Model::CgbE,
    );
    assert!(result.is_passed(), "mbc1/bits_mode test failed");
}

#[test]
fn test_mooneye_mbc1_bits_ramg() {
    let result = run_mooneye_test(
        "mooneye-test-suite/emulator-only/mbc1/bits_ramg.gb",
        Model::CgbE,
    );
    assert!(result.is_passed(), "mbc1/bits_ramg test failed");
}

#[test]
#[ignore] // Requires MBC1M multicart wiring support (different from standard MBC1)
fn test_mooneye_mbc1_multicart_rom_8mb() {
    let result = run_mooneye_test(
        "mooneye-test-suite/emulator-only/mbc1/multicart_rom_8Mb.gb",
        Model::CgbE,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "mbc1/multicart_rom_8Mb test failed"
    );
}

#[test]
fn test_mooneye_mbc1_ram_64kb() {
    let result = run_mooneye_test(
        "mooneye-test-suite/emulator-only/mbc1/ram_64kb.gb",
        Model::CgbE,
    );
    assert!(result.is_passed(), "mbc1/ram_64kb test failed");
}

#[test]
fn test_mooneye_mbc1_ram_256kb() {
    let result = run_mooneye_test(
        "mooneye-test-suite/emulator-only/mbc1/ram_256kb.gb",
        Model::CgbE,
    );
    assert!(result.is_passed(), "mbc1/ram_256kb test failed");
}

#[test]
fn test_mooneye_mbc1_rom_512kb() {
    let result = run_mooneye_test(
        "mooneye-test-suite/emulator-only/mbc1/rom_512kb.gb",
        Model::CgbE,
    );
    assert!(result.is_passed(), "mbc1/rom_512kb test failed");
}

#[test]
fn test_mooneye_mbc1_rom_1mb() {
    let result = run_mooneye_test(
        "mooneye-test-suite/emulator-only/mbc1/rom_1Mb.gb",
        Model::CgbE,
    );
    assert!(result.is_passed(), "mbc1/rom_1Mb test failed");
}

#[test]
fn test_mooneye_mbc1_rom_2mb() {
    let result = run_mooneye_test(
        "mooneye-test-suite/emulator-only/mbc1/rom_2Mb.gb",
        Model::CgbE,
    );
    assert!(result.is_passed(), "mbc1/rom_2Mb test failed");
}

#[test]
fn test_mooneye_mbc1_rom_4mb() {
    let result = run_mooneye_test(
        "mooneye-test-suite/emulator-only/mbc1/rom_4Mb.gb",
        Model::CgbE,
    );
    assert!(result.is_passed(), "mbc1/rom_4Mb test failed");
}

#[test]
fn test_mooneye_mbc1_rom_8mb() {
    let result = run_mooneye_test(
        "mooneye-test-suite/emulator-only/mbc1/rom_8Mb.gb",
        Model::CgbE,
    );
    assert!(result.is_passed(), "mbc1/rom_8Mb test failed");
}

#[test]
fn test_mooneye_mbc1_rom_16mb() {
    let result = run_mooneye_test(
        "mooneye-test-suite/emulator-only/mbc1/rom_16Mb.gb",
        Model::CgbE,
    );
    assert!(result.is_passed(), "mbc1/rom_16Mb test failed");
}

// =============================================================================
// Emulator-Only MBC2 Tests
// =============================================================================

#[test]
fn test_mooneye_mbc2_bits_ramg() {
    let result = run_mooneye_test(
        "mooneye-test-suite/emulator-only/mbc2/bits_ramg.gb",
        Model::CgbE,
    );
    assert!(result.is_passed(), "mbc2/bits_ramg test failed");
}

#[test]
fn test_mooneye_mbc2_bits_romb() {
    let result = run_mooneye_test(
        "mooneye-test-suite/emulator-only/mbc2/bits_romb.gb",
        Model::CgbE,
    );
    assert!(result.is_passed(), "mbc2/bits_romb test failed");
}

#[test]
fn test_mooneye_mbc2_bits_unused() {
    let result = run_mooneye_test(
        "mooneye-test-suite/emulator-only/mbc2/bits_unused.gb",
        Model::CgbE,
    );
    assert!(result.is_passed(), "mbc2/bits_unused test failed");
}

#[test]
fn test_mooneye_mbc2_ram() {
    let result = run_mooneye_test("mooneye-test-suite/emulator-only/mbc2/ram.gb", Model::CgbE);
    assert!(result.is_passed(), "mbc2/ram test failed");
}

#[test]
fn test_mooneye_mbc2_rom_512kb() {
    let result = run_mooneye_test(
        "mooneye-test-suite/emulator-only/mbc2/rom_512kb.gb",
        Model::CgbE,
    );
    assert!(result.is_passed(), "mbc2/rom_512kb test failed");
}

#[test]
fn test_mooneye_mbc2_rom_1mb() {
    let result = run_mooneye_test(
        "mooneye-test-suite/emulator-only/mbc2/rom_1Mb.gb",
        Model::CgbE,
    );
    assert!(result.is_passed(), "mbc2/rom_1Mb test failed");
}

#[test]
fn test_mooneye_mbc2_rom_2mb() {
    let result = run_mooneye_test(
        "mooneye-test-suite/emulator-only/mbc2/rom_2Mb.gb",
        Model::CgbE,
    );
    assert!(result.is_passed(), "mbc2/rom_2Mb test failed");
}

// =============================================================================
// Emulator-Only MBC5 Tests
// =============================================================================

#[test]
fn test_mooneye_mbc5_rom_512kb() {
    let result = run_mooneye_test(
        "mooneye-test-suite/emulator-only/mbc5/rom_512kb.gb",
        Model::CgbE,
    );
    assert!(result.is_passed(), "mbc5/rom_512kb test failed");
}

#[test]
fn test_mooneye_mbc5_rom_1mb() {
    let result = run_mooneye_test(
        "mooneye-test-suite/emulator-only/mbc5/rom_1Mb.gb",
        Model::CgbE,
    );
    assert!(result.is_passed(), "mbc5/rom_1Mb test failed");
}

#[test]
fn test_mooneye_mbc5_rom_2mb() {
    let result = run_mooneye_test(
        "mooneye-test-suite/emulator-only/mbc5/rom_2Mb.gb",
        Model::CgbE,
    );
    assert!(result.is_passed(), "mbc5/rom_2Mb test failed");
}

#[test]
fn test_mooneye_mbc5_rom_4mb() {
    let result = run_mooneye_test(
        "mooneye-test-suite/emulator-only/mbc5/rom_4Mb.gb",
        Model::CgbE,
    );
    assert!(result.is_passed(), "mbc5/rom_4Mb test failed");
}

#[test]
fn test_mooneye_mbc5_rom_8mb() {
    let result = run_mooneye_test(
        "mooneye-test-suite/emulator-only/mbc5/rom_8Mb.gb",
        Model::CgbE,
    );
    assert!(result.is_passed(), "mbc5/rom_8Mb test failed");
}

#[test]
fn test_mooneye_mbc5_rom_16mb() {
    let result = run_mooneye_test(
        "mooneye-test-suite/emulator-only/mbc5/rom_16Mb.gb",
        Model::CgbE,
    );
    assert!(result.is_passed(), "mbc5/rom_16Mb test failed");
}

#[test]
fn test_mooneye_mbc5_rom_32mb() {
    let result = run_mooneye_test(
        "mooneye-test-suite/emulator-only/mbc5/rom_32Mb.gb",
        Model::CgbE,
    );
    assert!(result.is_passed(), "mbc5/rom_32Mb test failed");
}

#[test]
fn test_mooneye_mbc5_rom_64mb() {
    let result = run_mooneye_test(
        "mooneye-test-suite/emulator-only/mbc5/rom_64Mb.gb",
        Model::CgbE,
    );
    assert!(result.is_passed(), "mbc5/rom_64Mb test failed");
}

// =============================================================================
// manual-only/ Tests
// =============================================================================

#[test]
fn test_mooneye_manual_sprite_priority_dmg() {
    let result = run_mooneye_screenshot_test(
        "mooneye-test-suite/manual-only/sprite_priority.gb",
        Model::DmgB,
    );
    assert!(
        result.is_passed(),
        "manual-only/sprite_priority DMG test failed"
    );
}

#[test]
fn test_mooneye_manual_sprite_priority_cgb() {
    let result = run_mooneye_screenshot_test(
        "mooneye-test-suite/manual-only/sprite_priority.gb",
        Model::CgbE,
    );
    assert!(
        result.is_passed(),
        "manual-only/sprite_priority CGB test failed"
    );
}
