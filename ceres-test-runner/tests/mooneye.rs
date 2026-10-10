//! Integration tests for the Mooneye and Wilbertpol test suites.
//!
//! - Mooneye tests live under `external/test-roms/mooneye-test-suite/` and
//!   use the `mooneye-test-suite/...` path prefix.
//! - Wilbertpol tests are an extended fork under
//!   `external/test-roms/mooneye-test-suite-wilbertpol/` and use the
//!   `mooneye-test-suite-wilbertpol/...` path prefix.
//!
//! Tests that exist in both suites run from the Wilbertpol ROMs to exercise
//! the most up-to-date versions of those tests.

use ceres_core::Model;
use ceres_test_runner::{
    expected_screenshot_path, load_test_rom,
    test_runner::{
        CompletionCheck, DummyAudioCallback, ScreenshotCheck, TestConfig, TestResult, TestRunner,
        timeouts,
    },
};

/// Root paths to the two test suites.
const MOONEYE: &str = "mooneye-test-suite";
const WILBERTPOL: &str = "mooneye-test-suite-wilbertpol";

/// Check for Mooneye/Wilbertpol test completion.
///
/// Both test suites report success/failure via the CPU registers:
/// - On pass, `B=3, C=5, D=8, E=13, H=21, L=34` (Fibonacci).
/// - On fail, all six registers contain `0x42`.
///
/// The two suites signal completion in different ways:
/// - Mooneye uses the `ld b, b` debug breakpoint (opcode `0x40`).
/// - Wilbertpol executes the undefined opcode `0xED`, which the SM83
///   handles by entering HALT and clearing `IE`.
pub struct MooneyeCheck;

impl CompletionCheck for MooneyeCheck {
    #[expect(clippy::many_single_char_names)]
    fn check(&self, gb: &mut ceres_core::Gb<DummyAudioCallback>) -> Option<TestResult> {
        // Wait for either completion signal: ld b,b breakpoint or illegal opcode (0xED).
        let triggered = gb.take_ld_b_b_breakpoint()
            || gb.take_illegal_opcode();
        if !triggered {
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

        let mut lines = Vec::new();
        for base in [0x9800, 0x9C00] {
            for row in 0..18 {
                let mut line = String::new();
                for col in 0..20 {
                    let b = gb.read_mem(base + row * 32 + col);
                    if b == 0x19 || b == 0 {
                        line.push(' ');
                    } else if (0x1A..=0x7E).contains(&b) {
                        line.push((b + 0x20 - 0x1A) as char);
                    } else {
                        line.push('.');
                    }
                }
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    lines.push(trimmed.to_string());
                }
            }
            if !lines.is_empty() {
                break;
            }
        }
        let text = lines.join(" | ");

        let mut c000_buf = Vec::new();
        for addr in 0xC000..0xC040 {
            c000_buf.push(format!("{:02X}", gb.read_mem(addr)));
        }
        let c000_str = c000_buf.join(" ");

        let mut hram_buf = Vec::new();
        for addr in 0xFF80..0xFFA0 {
            hram_buf.push(format!("{:02X}", gb.read_mem(addr)));
        }
        let hram_str = hram_buf.join(" ");

        Some(TestResult::Failed(format!(
            "Mooneye failure: B={b:#04X}, C={c:#04X}, D={d:#04X}, E={e:#04X}, H={h:#04X}, L={l:#04X}, C000=[{c000_str}], HRAM=[{hram_str}], text: \"{text}\""
        )))
    }

    fn on_timeout(&self, _gb: &mut ceres_core::Gb<DummyAudioCallback>) -> TestResult {
        TestResult::Failed("Mooneye test timed out".to_string())
    }
}

/// Helper function to run a Mooneye/Wilbertpol acceptance test.
///
/// `relative_path` is the path under the suite's root (e.g.
/// `acceptance/add_sp_e_timing.gb`); `suite_root` selects which suite
/// (`MOONEYE` or `WILBERTPOL`).
fn run_test(suite_root: &str, relative_path: &str, model: Model) -> TestResult {
    run_test_with_bootrom(suite_root, relative_path, model, false)
}

/// Runs a boot-state test twice: from the injected post-boot state and from
/// the real boot ROM, which must leave the same machine behind.
fn run_boot_test(suite_root: &str, relative_path: &str, model: Model) -> TestResult {
    let skipped = run_test_with_bootrom(suite_root, relative_path, model, false);
    if !skipped.is_passed() {
        return skipped;
    }
    run_test_with_bootrom(suite_root, relative_path, model, true)
}

fn run_test_with_bootrom(
    suite_root: &str,
    relative_path: &str,
    model: Model,
    run_bootrom: bool,
) -> TestResult {
    let path = format!("{suite_root}/{relative_path}");
    let rom = match load_test_rom(&path) {
        Ok(rom) => rom,
        Err(e) => return TestResult::Error(format!("Failed to load test ROM: {e}")),
    };

    let config = TestConfig {
        model,
        timeout_frames: timeouts::MOONEYE_ACCEPTANCE,
        run_bootrom,
        ..TestConfig::default()
    };

    let mut runner = match TestRunner::new(rom, config, Box::new(MooneyeCheck)) {
        Ok(runner) => runner,
        Err(e) => return TestResult::Error(format!("Failed to create test runner: {e}")),
    };

    runner.run()
}

/// Helper function to run a screenshot-based test (e.g. `manual-only/sprite_priority`).
fn run_screenshot_test(suite_root: &str, relative_path: &str, model: Model) -> TestResult {
    let path = format!("{suite_root}/{relative_path}");
    let rom = match load_test_rom(&path) {
        Ok(rom) => rom,
        Err(e) => return TestResult::Error(format!("Failed to load test ROM: {e}")),
    };

    let Some(screenshot_path) = expected_screenshot_path(&path, model) else {
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
// Root level acceptance tests
// =============================================================================

#[test]
fn test_add_sp_e_timing() {
    let result = run_test(WILBERTPOL, "acceptance/add_sp_e_timing.gb", Model::CgbE);
    assert!(result.is_passed(), "add_sp_e_timing test failed");
}

#[test]
fn test_call_cc_timing() {
    let result = run_test(WILBERTPOL, "acceptance/call_cc_timing.gb", Model::CgbE);
    assert!(result.is_passed(), "call_cc_timing test failed");
}

#[test]
fn test_call_cc_timing2() {
    let result = run_test(WILBERTPOL, "acceptance/call_cc_timing2.gb", Model::CgbE);
    assert!(result.is_passed(), "call_cc_timing2 test failed");
}

#[test]
fn test_call_timing() {
    let result = run_test(WILBERTPOL, "acceptance/call_timing.gb", Model::CgbE);
    assert!(result.is_passed(), "call_timing test failed");
}

#[test]
fn test_call_timing2() {
    let result = run_test(WILBERTPOL, "acceptance/call_timing2.gb", Model::CgbE);
    assert!(result.is_passed(), "call_timing2 test failed");
}

#[test]
fn test_di_timing_gs() {
    let result = run_test(WILBERTPOL, "acceptance/di_timing-GS.gb", Model::DmgB);
    assert_eq!(result, TestResult::Passed, "di_timing-GS test failed");
}

#[test]
fn test_div_timing() {
    let result = run_test(WILBERTPOL, "acceptance/div_timing.gb", Model::CgbE);
    assert!(result.is_passed(), "div_timing test failed");
}

#[test]
fn test_ei_sequence() {
    let result = run_test(MOONEYE, "acceptance/ei_sequence.gb", Model::CgbE);
    assert!(result.is_passed(), "ei_sequence test failed");
}

#[test]
fn test_ei_timing() {
    let result = run_test(WILBERTPOL, "acceptance/ei_timing.gb", Model::CgbE);
    assert!(result.is_passed(), "ei_timing test failed");
}

#[test]
fn test_halt_ime0_ei() {
    let result = run_test(WILBERTPOL, "acceptance/halt_ime0_ei.gb", Model::CgbE);
    assert!(result.is_passed(), "halt_ime0_ei test failed");
}

#[test]
fn test_halt_ime0_nointr_timing() {
    let result = run_test(
        WILBERTPOL,
        "acceptance/halt_ime0_nointr_timing.gb",
        Model::CgbE,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "halt_ime0_nointr_timing test failed"
    );
}

#[test]
fn test_halt_ime1_timing() {
    let result = run_test(WILBERTPOL, "acceptance/halt_ime1_timing.gb", Model::CgbE);
    assert!(result.is_passed(), "halt_ime1_timing test failed");
}

#[test]
fn test_halt_ime1_timing2_gs() {
    let result = run_test(
        WILBERTPOL,
        "acceptance/halt_ime1_timing2-GS.gb",
        Model::DmgB,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "halt_ime1_timing2-GS test failed"
    );
}

#[test]
fn test_if_ie_registers() {
    let result = run_test(WILBERTPOL, "acceptance/if_ie_registers.gb", Model::CgbE);
    assert!(result.is_passed(), "if_ie_registers test failed");
}

#[test]
fn test_intr_timing() {
    let result = run_test(WILBERTPOL, "acceptance/intr_timing.gb", Model::CgbE);
    assert!(result.is_passed(), "intr_timing test failed");
}

#[test]
fn test_jp_cc_timing() {
    let result = run_test(WILBERTPOL, "acceptance/jp_cc_timing.gb", Model::CgbE);
    assert!(result.is_passed(), "jp_cc_timing test failed");
}

#[test]
fn test_jp_timing() {
    let result = run_test(WILBERTPOL, "acceptance/jp_timing.gb", Model::CgbE);
    assert!(result.is_passed(), "jp_timing test failed");
}

#[test]
fn test_ld_hl_sp_e_timing() {
    let result = run_test(WILBERTPOL, "acceptance/ld_hl_sp_e_timing.gb", Model::CgbE);
    assert!(result.is_passed(), "ld_hl_sp_e_timing test failed");
}

#[test]
fn test_oam_dma_restart() {
    let result = run_test(WILBERTPOL, "acceptance/oam_dma_restart.gb", Model::CgbE);
    assert!(result.is_passed(), "oam_dma_restart test failed");
}

#[test]
fn test_oam_dma_start() {
    let result = run_test(WILBERTPOL, "acceptance/oam_dma_start.gb", Model::CgbE);
    assert!(result.is_passed(), "oam_dma_start test failed");
}

#[test]
fn test_oam_dma_timing() {
    let result = run_test(WILBERTPOL, "acceptance/oam_dma_timing.gb", Model::CgbE);
    assert!(result.is_passed(), "oam_dma_timing test failed");
}

#[test]
fn test_pop_timing() {
    let result = run_test(WILBERTPOL, "acceptance/pop_timing.gb", Model::CgbE);
    assert!(result.is_passed(), "pop_timing test failed");
}

#[test]
fn test_push_timing() {
    let result = run_test(WILBERTPOL, "acceptance/push_timing.gb", Model::CgbE);
    assert!(result.is_passed(), "push_timing test failed");
}

#[test]
fn test_rapid_di_ei() {
    let result = run_test(WILBERTPOL, "acceptance/rapid_di_ei.gb", Model::CgbE);
    assert!(result.is_passed(), "rapid_di_ei test failed: {result:?}");
}

#[test]
fn test_ret_cc_timing() {
    let result = run_test(WILBERTPOL, "acceptance/ret_cc_timing.gb", Model::CgbE);
    assert!(result.is_passed(), "ret_cc_timing test failed");
}

#[test]
fn test_ret_timing() {
    let result = run_test(WILBERTPOL, "acceptance/ret_timing.gb", Model::CgbE);
    assert!(result.is_passed(), "ret_timing test failed");
}

#[test]
fn test_reti_intr_timing() {
    let result = run_test(WILBERTPOL, "acceptance/reti_intr_timing.gb", Model::CgbE);
    assert!(result.is_passed(), "reti_intr_timing test failed");
}

#[test]
fn test_reti_timing() {
    let result = run_test(WILBERTPOL, "acceptance/reti_timing.gb", Model::CgbE);
    assert!(result.is_passed(), "reti_timing test failed");
}

#[test]
fn test_rst_timing() {
    let result = run_test(WILBERTPOL, "acceptance/rst_timing.gb", Model::CgbE);
    assert!(result.is_passed(), "rst_timing test failed");
}

// =============================================================================
// Boot register tests (model-specific)
// =============================================================================

#[test]
fn test_boot_div2_s() {
    let result = run_boot_test(MOONEYE, "acceptance/boot_div2-S.gb", Model::Sgb2);
    assert!(result.is_passed(), "boot_div2-S test failed");
}

#[test]
fn test_boot_div_cgb0() {
    let result = run_boot_test(MOONEYE, "misc/boot_div-cgb0.gb", Model::Cgb0);
    assert!(result.is_passed(), "boot_div-cgb0 test failed: {result:?}");
}

#[test]
fn test_boot_div_cgbabcde() {
    let result = run_boot_test(MOONEYE, "misc/boot_div-cgbABCDE.gb", Model::CgbE);
    assert!(
        result.is_passed(),
        "boot_div-cgbABCDE test failed: {result:?}"
    );
}

#[test]
fn test_boot_div_a() {
    let result = run_boot_test(MOONEYE, "misc/boot_div-A.gb", Model::Agb);
    assert!(result.is_passed(), "boot_div-A test failed: {result:?}");
}

#[test]
fn test_boot_div_dmg0() {
    let result = run_boot_test(MOONEYE, "acceptance/boot_div-dmg0.gb", Model::Dmg0);
    assert!(result.is_passed(), "boot_div-dmg0 test failed");
}

#[test]
fn test_boot_div_dmgabcmgb() {
    let result = run_boot_test(MOONEYE, "acceptance/boot_div-dmgABCmgb.gb", Model::DmgB);
    assert!(
        result.is_passed(),
        "boot_div-dmgABCmgb test failed: {result:?}"
    );
}

#[test]
fn test_boot_div_s() {
    let result = run_boot_test(MOONEYE, "acceptance/boot_div-S.gb", Model::Sgb);
    assert!(result.is_passed(), "boot_div-S test failed");
}

#[test]
fn test_boot_hwio_c() {
    let result = run_boot_test(WILBERTPOL, "misc/boot_hwio-C.gb", Model::CgbE);
    assert_eq!(result, TestResult::Passed, "boot_hwio-C test failed");
}

#[test]
fn test_boot_hwio_dmg0() {
    let result = run_boot_test(MOONEYE, "acceptance/boot_hwio-dmg0.gb", Model::Dmg0);
    assert_eq!(result, TestResult::Passed, "boot_hwio-dmg0 test failed");
}

#[test]
fn test_boot_hwio_g() {
    let result = run_boot_test(WILBERTPOL, "acceptance/boot_hwio-G.gb", Model::DmgB);
    assert_eq!(result, TestResult::Passed, "boot_hwio-G test failed");
}

#[test]
fn test_boot_hwio_s() {
    let result = run_boot_test(WILBERTPOL, "misc/boot_hwio-S.gb", Model::Sgb);
    assert_eq!(result, TestResult::Passed, "boot_hwio-S test failed");
}

#[test]
fn test_boot_regs_a() {
    let result = run_boot_test(WILBERTPOL, "misc/boot_regs-A.gb", Model::Agb);
    assert!(result.is_passed(), "boot_regs-A test failed: {result:?}");
}

#[test]
fn test_boot_regs_cgb() {
    let result = run_boot_test(WILBERTPOL, "misc/boot_regs-cgb.gb", Model::Cgb0);
    assert!(result.is_passed(), "boot_regs-cgb test failed");
}

#[test]
fn test_boot_regs_dmg() {
    let result = run_boot_test(WILBERTPOL, "acceptance/boot_regs-dmg.gb", Model::DmgB);
    assert!(result.is_passed(), "boot_regs-dmg test failed");
}

#[test]
fn test_boot_regs_dmg0() {
    let result = run_boot_test(MOONEYE, "acceptance/boot_regs-dmg0.gb", Model::Dmg0);
    assert!(result.is_passed(), "boot_regs-dmg0 test failed");
}

#[test]
fn test_boot_regs_mgb() {
    let result = run_boot_test(WILBERTPOL, "misc/boot_regs-mgb.gb", Model::Mgb);
    assert!(result.is_passed(), "boot_regs-mgb test failed");
}

#[test]
fn test_boot_regs_sgb() {
    let result = run_boot_test(WILBERTPOL, "misc/boot_regs-sgb.gb", Model::Sgb);
    assert!(result.is_passed(), "boot_regs-sgb test failed");
}

#[test]
fn test_boot_regs_sgb2() {
    let result = run_boot_test(WILBERTPOL, "misc/boot_regs-sgb2.gb", Model::Sgb2);
    assert!(result.is_passed(), "boot_regs-sgb2 test failed");
}

// =============================================================================
// bits/ tests
// =============================================================================

#[test]
fn test_bits_mem_oam() {
    let result = run_test(WILBERTPOL, "acceptance/bits/mem_oam.gb", Model::CgbE);
    assert!(result.is_passed(), "bits/mem_oam test failed");
}

#[test]
fn test_bits_reg_f() {
    let result = run_test(WILBERTPOL, "acceptance/bits/reg_f.gb", Model::CgbE);
    assert!(result.is_passed(), "bits/reg_f test failed");
}

#[test]
fn test_bits_unused_hwio_c() {
    let result = run_test(WILBERTPOL, "misc/bits/unused_hwio-C.gb", Model::CgbC);
    assert_eq!(result, TestResult::Passed, "bits/unused_hwio-C test failed");
}

#[test]
fn test_bits_unused_hwio_gs() {
    let result = run_test(WILBERTPOL, "acceptance/bits/unused_hwio-GS.gb", Model::DmgB);
    assert_eq!(
        result,
        TestResult::Passed,
        "bits/unused_hwio-GS test failed"
    );
}

// =============================================================================
// instr/ tests
// =============================================================================

#[test]
fn test_instr_daa() {
    let result = run_test(MOONEYE, "acceptance/instr/daa.gb", Model::CgbE);
    assert!(result.is_passed(), "instr/daa test failed");
}

// =============================================================================
// interrupts/ tests
// =============================================================================

#[test]
fn test_interrupts_ie_push() {
    let result = run_test(MOONEYE, "acceptance/interrupts/ie_push.gb", Model::CgbE);
    assert!(result.is_passed(), "interrupts/ie_push test failed");
}

// =============================================================================
// oam_dma/ tests
// =============================================================================

#[test]
fn test_oam_dma_basic() {
    let result = run_test(MOONEYE, "acceptance/oam_dma/basic.gb", Model::CgbE);
    assert!(result.is_passed(), "oam_dma/basic test failed");
}

#[test]
fn test_oam_dma_reg_read() {
    let result = run_test(MOONEYE, "acceptance/oam_dma/reg_read.gb", Model::CgbE);
    assert!(result.is_passed(), "oam_dma/reg_read test failed");
}

#[test]
fn test_oam_dma_sources_gs() {
    let result = run_test(MOONEYE, "acceptance/oam_dma/sources-GS.gb", Model::DmgB);
    assert!(result.is_passed(), "oam_dma/sources-GS test failed");
}

// =============================================================================
// gpu/ tests (PPU)
// =============================================================================

#[test]
fn test_gpu_hblank_ly_scx_timing_c() {
    let result = run_test(
        WILBERTPOL,
        "acceptance/gpu/hblank_ly_scx_timing-C.gb",
        Model::CgbE,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "gpu/hblank_ly_scx_timing-C test failed"
    );
}

#[test]
fn test_gpu_hblank_ly_scx_timing_gs() {
    let result = run_test(
        WILBERTPOL,
        "acceptance/gpu/hblank_ly_scx_timing-GS.gb",
        Model::DmgB,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "gpu/hblank_ly_scx_timing-GS test failed"
    );
}

#[test]
fn test_gpu_hblank_ly_scx_timing_nops() {
    let result = run_test(
        WILBERTPOL,
        "acceptance/gpu/hblank_ly_scx_timing_nops.gb",
        Model::CgbE,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "gpu/hblank_ly_scx_timing_nops test failed"
    );
}

#[test]
fn test_gpu_hblank_ly_scx_timing_variant_nops() {
    let result = run_test(
        WILBERTPOL,
        "acceptance/gpu/hblank_ly_scx_timing_variant_nops.gb",
        Model::CgbE,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "gpu/hblank_ly_scx_timing_variant_nops test failed"
    );
}

#[test]
fn test_gpu_intr_0_timing() {
    let result = run_test(WILBERTPOL, "acceptance/gpu/intr_0_timing.gb", Model::DmgB);
    assert_eq!(result, TestResult::Passed, "gpu/intr_0_timing test failed");
}

#[test]
fn test_gpu_intr_1_2_timing_gs() {
    let result = run_test(
        WILBERTPOL,
        "acceptance/gpu/intr_1_2_timing-GS.gb",
        Model::DmgB,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "gpu/intr_1_2_timing-GS test failed"
    );
}

#[test]
fn test_gpu_intr_1_timing() {
    let result = run_test(WILBERTPOL, "acceptance/gpu/intr_1_timing.gb", Model::DmgB);
    assert_eq!(result, TestResult::Passed, "gpu/intr_1_timing test failed");
}

#[test]
fn test_gpu_intr_2_0_timing() {
    let result = run_test(WILBERTPOL, "acceptance/gpu/intr_2_0_timing.gb", Model::DmgB);
    assert_eq!(
        result,
        TestResult::Passed,
        "gpu/intr_2_0_timing test failed"
    );
}

#[test]
fn test_gpu_intr_2_mode0_scx1_timing_nops() {
    let result = run_test(
        WILBERTPOL,
        "acceptance/gpu/intr_2_mode0_scx1_timing_nops.gb",
        Model::DmgB,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "gpu/intr_2_mode0_scx1_timing_nops test failed"
    );
}

#[test]
fn test_gpu_intr_2_mode0_scx2_timing_nops() {
    let result = run_test(
        WILBERTPOL,
        "acceptance/gpu/intr_2_mode0_scx2_timing_nops.gb",
        Model::DmgB,
    );
    assert!(
        result.is_passed(),
        "gpu/intr_2_mode0_scx2_timing_nops test failed"
    );
}

#[test]
fn test_gpu_intr_2_mode0_scx3_timing_nops() {
    let result = run_test(
        WILBERTPOL,
        "acceptance/gpu/intr_2_mode0_scx3_timing_nops.gb",
        Model::DmgB,
    );
    assert!(
        result.is_passed(),
        "gpu/intr_2_mode0_scx3_timing_nops test failed"
    );
}

#[test]
fn test_gpu_intr_2_mode0_scx4_timing_nops() {
    let result = run_test(
        WILBERTPOL,
        "acceptance/gpu/intr_2_mode0_scx4_timing_nops.gb",
        Model::DmgB,
    );
    assert!(
        result.is_passed(),
        "gpu/intr_2_mode0_scx4_timing_nops test failed"
    );
}

#[test]
fn test_gpu_intr_2_mode0_scx5_timing_nops() {
    let result = run_test(
        WILBERTPOL,
        "acceptance/gpu/intr_2_mode0_scx5_timing_nops.gb",
        Model::DmgB,
    );
    assert!(
        result.is_passed(),
        "gpu/intr_2_mode0_scx5_timing_nops test failed"
    );
}

#[test]
fn test_gpu_intr_2_mode0_scx6_timing_nops() {
    let result = run_test(
        WILBERTPOL,
        "acceptance/gpu/intr_2_mode0_scx6_timing_nops.gb",
        Model::DmgB,
    );
    assert!(
        result.is_passed(),
        "gpu/intr_2_mode0_scx6_timing_nops test failed"
    );
}

#[test]
fn test_gpu_intr_2_mode0_scx7_timing_nops() {
    let result = run_test(
        WILBERTPOL,
        "acceptance/gpu/intr_2_mode0_scx7_timing_nops.gb",
        Model::DmgB,
    );
    assert!(
        result.is_passed(),
        "gpu/intr_2_mode0_scx7_timing_nops test failed"
    );
}

#[test]
fn test_gpu_intr_2_mode0_scx8_timing_nops() {
    let result = run_test(
        WILBERTPOL,
        "acceptance/gpu/intr_2_mode0_scx8_timing_nops.gb",
        Model::DmgB,
    );
    assert!(
        result.is_passed(),
        "gpu/intr_2_mode0_scx8_timing_nops test failed"
    );
}

#[test]
fn test_gpu_intr_2_mode0_timing() {
    let result = run_test(
        WILBERTPOL,
        "acceptance/gpu/intr_2_mode0_timing.gb",
        Model::DmgB,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "gpu/intr_2_mode0_timing test failed"
    );
}

#[test]
fn test_gpu_intr_2_mode0_timing_sprites() {
    let result = run_test(
        WILBERTPOL,
        "acceptance/gpu/intr_2_mode0_timing_sprites.gb",
        Model::DmgB,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "gpu/intr_2_mode0_timing_sprites test failed"
    );
}

#[test]
fn test_gpu_intr_2_mode0_timing_sprites_nops() {
    let result = run_test(
        WILBERTPOL,
        "acceptance/gpu/intr_2_mode0_timing_sprites_nops.gb",
        Model::DmgB,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "gpu/intr_2_mode0_timing_sprites_nops test failed"
    );
}

#[test]
fn test_gpu_intr_2_mode0_timing_sprites_scx1_nops() {
    let result = run_test(
        WILBERTPOL,
        "acceptance/gpu/intr_2_mode0_timing_sprites_scx1_nops.gb",
        Model::DmgB,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "gpu/intr_2_mode0_timing_sprites_scx1_nops test failed"
    );
}

#[test]
fn test_gpu_intr_2_mode0_timing_sprites_scx2_nops() {
    let result = run_test(
        WILBERTPOL,
        "acceptance/gpu/intr_2_mode0_timing_sprites_scx2_nops.gb",
        Model::DmgB,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "gpu/intr_2_mode0_timing_sprites_scx2_nops test failed"
    );
}

#[test]
fn test_gpu_intr_2_mode0_timing_sprites_scx3_nops() {
    let result = run_test(
        WILBERTPOL,
        "acceptance/gpu/intr_2_mode0_timing_sprites_scx3_nops.gb",
        Model::DmgB,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "gpu/intr_2_mode0_timing_sprites_scx3_nops test failed"
    );
}

#[test]
fn test_gpu_intr_2_mode0_timing_sprites_scx4_nops() {
    let result = run_test(
        WILBERTPOL,
        "acceptance/gpu/intr_2_mode0_timing_sprites_scx4_nops.gb",
        Model::DmgB,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "gpu/intr_2_mode0_timing_sprites_scx4_nops test failed"
    );
}

#[test]
fn test_gpu_intr_2_mode3_timing() {
    let result = run_test(
        WILBERTPOL,
        "acceptance/gpu/intr_2_mode3_timing.gb",
        Model::DmgB,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "gpu/intr_2_mode3_timing test failed"
    );
}

#[test]
fn test_gpu_intr_2_oam_ok_timing() {
    let result = run_test(
        WILBERTPOL,
        "acceptance/gpu/intr_2_oam_ok_timing.gb",
        Model::DmgB,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "gpu/intr_2_oam_ok_timing test failed"
    );
}

#[test]
fn test_gpu_intr_2_timing() {
    let result = run_test(WILBERTPOL, "acceptance/gpu/intr_2_timing.gb", Model::DmgB);
    assert_eq!(result, TestResult::Passed, "gpu/intr_2_timing test failed");
}

#[test]
fn test_gpu_lcdon_mode_timing() {
    let result = run_test(
        WILBERTPOL,
        "acceptance/gpu/lcdon_mode_timing.gb",
        Model::DmgB,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "gpu/lcdon_mode_timing test failed"
    );
}

#[test]
fn test_gpu_lcdon_timing_gs() {
    let result = run_test(MOONEYE, "acceptance/ppu/lcdon_timing-GS.gb", Model::DmgB);
    assert_eq!(
        result,
        TestResult::Passed,
        "ppu/lcdon_timing-GS test failed"
    );
}

#[test]
fn test_gpu_lcdon_write_timing_gs() {
    let result = run_test(
        MOONEYE,
        "acceptance/ppu/lcdon_write_timing-GS.gb",
        Model::DmgB,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "ppu/lcdon_write_timing-GS test failed"
    );
}

#[test]
fn test_gpu_ly00_01_mode0_2() {
    let result = run_test(WILBERTPOL, "acceptance/gpu/ly00_01_mode0_2.gb", Model::DmgB);
    assert_eq!(
        result,
        TestResult::Passed,
        "gpu/ly00_01_mode0_2 test failed"
    );
}

#[test]
fn test_gpu_ly00_mode0_2_gs() {
    let result = run_test(WILBERTPOL, "acceptance/gpu/ly00_mode0_2-GS.gb", Model::DmgB);
    assert_eq!(
        result,
        TestResult::Passed,
        "gpu/ly00_mode0_2-GS test failed"
    );
}

#[test]
fn test_gpu_ly00_mode1_0_gs() {
    let result = run_test(WILBERTPOL, "acceptance/gpu/ly00_mode1_0-GS.gb", Model::DmgB);
    assert_eq!(
        result,
        TestResult::Passed,
        "gpu/ly00_mode1_0-GS test failed"
    );
}

#[test]
fn test_gpu_ly00_mode1_2_c() {
    // "-C" is any CGB: on the CGB-C (gambatte's enable_display/frame1_*
    // tests) mode 1 ends a dot before this test expects.
    let result = run_test(WILBERTPOL, "acceptance/gpu/ly00_mode1_2-C.gb", Model::CgbE);
    assert_eq!(result, TestResult::Passed, "gpu/ly00_mode1_2-C test failed");
}

#[test]
fn test_gpu_ly00_mode2_3() {
    let result = run_test(WILBERTPOL, "acceptance/gpu/ly00_mode2_3.gb", Model::DmgB);
    assert!(result.is_passed(), "gpu/ly00_mode2_3 test failed");
}

#[test]
fn test_gpu_ly00_mode3_0() {
    let result = run_test(WILBERTPOL, "acceptance/gpu/ly00_mode3_0.gb", Model::DmgB);
    assert!(result.is_passed(), "gpu/ly00_mode3_0 test failed");
}

#[test]
fn test_gpu_ly143_144_145() {
    let result = run_test(WILBERTPOL, "acceptance/gpu/ly143_144_145.gb", Model::Mgb);
    assert_eq!(result, TestResult::Passed, "gpu/ly143_144_145 test failed");
}

#[test]
fn test_gpu_ly143_144_152_153() {
    let result = run_test(
        WILBERTPOL,
        "acceptance/gpu/ly143_144_152_153.gb",
        Model::DmgB,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "gpu/ly143_144_152_153 test failed"
    );
}

#[test]
fn test_gpu_ly143_144_mode0_1() {
    let result = run_test(
        WILBERTPOL,
        "acceptance/gpu/ly143_144_mode0_1.gb",
        Model::DmgB,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "gpu/ly143_144_mode0_1 test failed"
    );
}

#[test]
fn test_gpu_ly143_144_mode3_0() {
    let result = run_test(
        WILBERTPOL,
        "acceptance/gpu/ly143_144_mode3_0.gb",
        Model::DmgB,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "gpu/ly143_144_mode3_0 test failed"
    );
}

// The `-C` ly_* ROMs were measured on a CGB-D/E class unit: on CGB-C and
// earlier LY reads 0 one M-cycle sooner on line 153.
#[test]
fn test_gpu_ly_lyc_0_c() {
    let result = run_test(WILBERTPOL, "acceptance/gpu/ly_lyc_0-C.gb", Model::CgbE);
    assert_eq!(result, TestResult::Passed, "gpu/ly_lyc_0-C test failed");
}

#[test]
fn test_gpu_ly_lyc_0_gs() {
    let result = run_test(WILBERTPOL, "acceptance/gpu/ly_lyc_0-GS.gb", Model::DmgB);
    assert_eq!(result, TestResult::Passed, "gpu/ly_lyc_0-GS test failed");
}

#[test]
fn test_gpu_ly_lyc_0_write_c() {
    let result = run_test(
        WILBERTPOL,
        "acceptance/gpu/ly_lyc_0_write-C.gb",
        Model::CgbE,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "gpu/ly_lyc_0_write-C test failed"
    );
}

#[test]
fn test_gpu_ly_lyc_0_write_gs() {
    let result = run_test(
        WILBERTPOL,
        "acceptance/gpu/ly_lyc_0_write-GS.gb",
        Model::DmgB,
    );
    assert!(result.is_passed(), "gpu/ly_lyc_0_write-GS test failed");
}

#[test]
fn test_gpu_ly_lyc_144_c() {
    let result = run_test(WILBERTPOL, "acceptance/gpu/ly_lyc_144-C.gb", Model::CgbE);
    assert_eq!(result, TestResult::Passed, "gpu/ly_lyc_144-C test failed");
}

#[test]
fn test_gpu_ly_lyc_144_gs() {
    let result = run_test(WILBERTPOL, "acceptance/gpu/ly_lyc_144-GS.gb", Model::DmgB);
    assert_eq!(result, TestResult::Passed, "gpu/ly_lyc_144-GS test failed");
}

#[test]
fn test_gpu_ly_lyc_153_c() {
    let result = run_test(WILBERTPOL, "acceptance/gpu/ly_lyc_153-C.gb", Model::CgbE);
    assert_eq!(result, TestResult::Passed, "gpu/ly_lyc_153-C test failed");
}

#[test]
fn test_gpu_ly_lyc_153_gs() {
    let result = run_test(WILBERTPOL, "acceptance/gpu/ly_lyc_153-GS.gb", Model::DmgB);
    assert_eq!(result, TestResult::Passed, "gpu/ly_lyc_153-GS test failed");
}

#[test]
fn test_gpu_ly_lyc_153_write_c() {
    let result = run_test(
        WILBERTPOL,
        "acceptance/gpu/ly_lyc_153_write-C.gb",
        Model::CgbE,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "gpu/ly_lyc_153_write-C test failed"
    );
}

#[test]
fn test_gpu_ly_lyc_153_write_gs() {
    let result = run_test(
        WILBERTPOL,
        "acceptance/gpu/ly_lyc_153_write-GS.gb",
        Model::DmgB,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "gpu/ly_lyc_153_write-GS test failed"
    );
}

#[test]
fn test_gpu_ly_lyc_c() {
    let result = run_test(WILBERTPOL, "acceptance/gpu/ly_lyc-C.gb", Model::CgbE);
    assert_eq!(result, TestResult::Passed, "gpu/ly_lyc-C test failed");
}

#[test]
fn test_gpu_ly_lyc_gs() {
    let result = run_test(WILBERTPOL, "acceptance/gpu/ly_lyc-GS.gb", Model::DmgB);
    assert_eq!(result, TestResult::Passed, "gpu/ly_lyc-GS test failed");
}

#[test]
fn test_gpu_ly_lyc_write_c() {
    let result = run_test(WILBERTPOL, "acceptance/gpu/ly_lyc_write-C.gb", Model::CgbE);
    assert_eq!(result, TestResult::Passed, "gpu/ly_lyc_write-C test failed");
}

#[test]
fn test_gpu_ly_lyc_write_gs() {
    let result = run_test(WILBERTPOL, "acceptance/gpu/ly_lyc_write-GS.gb", Model::DmgB);
    assert_eq!(
        result,
        TestResult::Passed,
        "gpu/ly_lyc_write-GS test failed"
    );
}

// See `test_gpu_ly_lyc_0_c` for the model choice.
#[test]
fn test_gpu_ly_new_frame_c() {
    let result = run_test(WILBERTPOL, "acceptance/gpu/ly_new_frame-C.gb", Model::CgbE);
    assert_eq!(result, TestResult::Passed, "gpu/ly_new_frame-C test failed");
}

#[test]
fn test_gpu_ly_new_frame_gs() {
    let result = run_test(WILBERTPOL, "acceptance/gpu/ly_new_frame-GS.gb", Model::DmgB);
    assert_eq!(
        result,
        TestResult::Passed,
        "gpu/ly_new_frame-GS test failed"
    );
}

#[test]
fn test_gpu_stat_irq_blocking() {
    let result = run_test(
        WILBERTPOL,
        "acceptance/gpu/stat_irq_blocking.gb",
        Model::DmgB,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "gpu/stat_irq_blocking test failed"
    );
}

#[test]
fn test_gpu_stat_write_if_c() {
    let result = run_test(WILBERTPOL, "acceptance/gpu/stat_write_if-C.gb", Model::CgbC);
    assert_eq!(
        result,
        TestResult::Passed,
        "gpu/stat_write_if-C test failed"
    );
}

#[test]
fn test_gpu_stat_write_if_gs() {
    let result = run_test(
        WILBERTPOL,
        "acceptance/gpu/stat_write_if-GS.gb",
        Model::DmgB,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "gpu/stat_write_if-GS test failed"
    );
}

#[test]
fn test_gpu_vblank_if_timing() {
    let result = run_test(
        WILBERTPOL,
        "acceptance/gpu/vblank_if_timing.gb",
        Model::CgbE,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "gpu/vblank_if_timing test failed"
    );
}

#[test]
fn test_gpu_vblank_stat_intr_c() {
    let result = run_test(WILBERTPOL, "misc/gpu/vblank_stat_intr-C.gb", Model::CgbE);
    assert_eq!(
        result,
        TestResult::Passed,
        "gpu/vblank_stat_intr-C test failed"
    );
}

#[test]
fn test_gpu_vblank_stat_intr_gs() {
    let result = run_test(
        WILBERTPOL,
        "acceptance/gpu/vblank_stat_intr-GS.gb",
        Model::Mgb,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "gpu/vblank_stat_intr-GS test failed"
    );
}

// =============================================================================
// serial/ tests
// =============================================================================

#[test]
fn test_serial_boot_sclk_align_dmgabcmgb() {
    let result = run_test(
        MOONEYE,
        "acceptance/serial/boot_sclk_align-dmgABCmgb.gb",
        Model::DmgB,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "serial/boot_sclk_align-dmgABCmgb test failed"
    );
}

// =============================================================================
// timer/ tests
// =============================================================================

#[test]
fn test_timer_div_write() {
    let result = run_test(WILBERTPOL, "acceptance/timer/div_write.gb", Model::CgbE);
    assert!(result.is_passed(), "timer/div_write test failed");
}

#[test]
fn test_timer_if() {
    let result = run_test(WILBERTPOL, "acceptance/timer/timer_if.gb", Model::CgbE);
    assert!(result.is_passed(), "timer/timer_if test failed");
}

#[test]
fn test_timer_rapid_toggle() {
    let result = run_test(WILBERTPOL, "acceptance/timer/rapid_toggle.gb", Model::CgbE);
    assert_eq!(result, TestResult::Passed, "timer/rapid_toggle test failed");
}

#[test]
fn test_timer_tim00() {
    let result = run_test(WILBERTPOL, "acceptance/timer/tim00.gb", Model::CgbE);
    assert!(result.is_passed(), "timer/tim00 test failed");
}

#[test]
fn test_timer_tim00_div_trigger() {
    let result = run_test(
        WILBERTPOL,
        "acceptance/timer/tim00_div_trigger.gb",
        Model::CgbE,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "timer/tim00_div_trigger test failed"
    );
}

#[test]
fn test_timer_tim01() {
    let result = run_test(WILBERTPOL, "acceptance/timer/tim01.gb", Model::CgbE);
    assert!(result.is_passed(), "timer/tim01 test failed");
}

#[test]
fn test_timer_tim01_div_trigger() {
    let result = run_test(
        WILBERTPOL,
        "acceptance/timer/tim01_div_trigger.gb",
        Model::CgbE,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "timer/tim01_div_trigger test failed"
    );
}

#[test]
fn test_timer_tim10() {
    let result = run_test(WILBERTPOL, "acceptance/timer/tim10.gb", Model::CgbE);
    assert!(result.is_passed(), "timer/tim10 test failed");
}

#[test]
fn test_timer_tim10_div_trigger() {
    let result = run_test(
        WILBERTPOL,
        "acceptance/timer/tim10_div_trigger.gb",
        Model::CgbE,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "timer/tim10_div_trigger test failed"
    );
}

#[test]
fn test_timer_tim11() {
    let result = run_test(WILBERTPOL, "acceptance/timer/tim11.gb", Model::CgbE);
    assert!(result.is_passed(), "timer/tim11 test failed");
}

#[test]
fn test_timer_tim11_div_trigger() {
    let result = run_test(
        WILBERTPOL,
        "acceptance/timer/tim11_div_trigger.gb",
        Model::CgbE,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "timer/tim11_div_trigger test failed"
    );
}

#[test]
fn test_timer_tima_reload() {
    let result = run_test(WILBERTPOL, "acceptance/timer/tima_reload.gb", Model::CgbE);
    assert_eq!(result, TestResult::Passed, "timer/tima_reload test failed");
}

#[test]
fn test_timer_tima_write_reloading() {
    let result = run_test(
        WILBERTPOL,
        "acceptance/timer/tima_write_reloading.gb",
        Model::CgbE,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "timer/tima_write_reloading test failed"
    );
}

#[test]
fn test_timer_tma_write_reloading() {
    let result = run_test(
        WILBERTPOL,
        "acceptance/timer/tma_write_reloading.gb",
        Model::CgbE,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "timer/tma_write_reloading test failed"
    );
}

// =============================================================================
// emulator-only/ tests
// =============================================================================
//
// MBC1 tests live under mooneye-test-suite/emulator-only/mbc1/, while
// Wilbertpol's only MBC test is mbc1_rom_4banks.gb at the suite root.

#[test]
fn test_mbc1_bits_bank1() {
    let result = run_test(MOONEYE, "emulator-only/mbc1/bits_bank1.gb", Model::CgbE);
    assert!(result.is_passed(), "mbc1/bits_bank1 test failed");
}

#[test]
fn test_mbc1_bits_bank2() {
    let result = run_test(MOONEYE, "emulator-only/mbc1/bits_bank2.gb", Model::CgbE);
    assert!(result.is_passed(), "mbc1/bits_bank2 test failed");
}

#[test]
fn test_mbc1_bits_mode() {
    let result = run_test(MOONEYE, "emulator-only/mbc1/bits_mode.gb", Model::CgbE);
    assert!(result.is_passed(), "mbc1/bits_mode test failed");
}

#[test]
fn test_mbc1_bits_ramg() {
    let result = run_test(MOONEYE, "emulator-only/mbc1/bits_ramg.gb", Model::CgbE);
    assert!(result.is_passed(), "mbc1/bits_ramg test failed");
}

#[test]
fn test_mbc1_multicart_rom_8mb() {
    let result = run_test(
        MOONEYE,
        "emulator-only/mbc1/multicart_rom_8Mb.gb",
        Model::CgbE,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "mbc1/multicart_rom_8Mb test failed"
    );
}

#[test]
fn test_mbc1_ram_64kb() {
    let result = run_test(MOONEYE, "emulator-only/mbc1/ram_64kb.gb", Model::CgbE);
    assert!(result.is_passed(), "mbc1/ram_64kb test failed");
}

#[test]
fn test_mbc1_ram_256kb() {
    let result = run_test(MOONEYE, "emulator-only/mbc1/ram_256kb.gb", Model::CgbE);
    assert!(result.is_passed(), "mbc1/ram_256kb test failed");
}

#[test]
fn test_mbc1_rom_4banks() {
    let result = run_test(WILBERTPOL, "emulator-only/mbc1_rom_4banks.gb", Model::CgbE);
    assert!(result.is_passed(), "mbc1_rom_4banks test failed");
}

#[test]
fn test_mbc1_rom_512kb() {
    let result = run_test(MOONEYE, "emulator-only/mbc1/rom_512kb.gb", Model::CgbE);
    assert!(result.is_passed(), "mbc1/rom_512kb test failed");
}

#[test]
fn test_mbc1_rom_1mb() {
    let result = run_test(MOONEYE, "emulator-only/mbc1/rom_1Mb.gb", Model::CgbE);
    assert!(result.is_passed(), "mbc1/rom_1Mb test failed");
}

#[test]
fn test_mbc1_rom_2mb() {
    let result = run_test(MOONEYE, "emulator-only/mbc1/rom_2Mb.gb", Model::CgbE);
    assert!(result.is_passed(), "mbc1/rom_2Mb test failed");
}

#[test]
fn test_mbc1_rom_4mb() {
    let result = run_test(MOONEYE, "emulator-only/mbc1/rom_4Mb.gb", Model::CgbE);
    assert!(result.is_passed(), "mbc1/rom_4Mb test failed");
}

#[test]
fn test_mbc1_rom_8mb() {
    let result = run_test(MOONEYE, "emulator-only/mbc1/rom_8Mb.gb", Model::CgbE);
    assert!(result.is_passed(), "mbc1/rom_8Mb test failed");
}

#[test]
fn test_mbc1_rom_16mb() {
    let result = run_test(MOONEYE, "emulator-only/mbc1/rom_16Mb.gb", Model::CgbE);
    assert!(result.is_passed(), "mbc1/rom_16Mb test failed");
}

#[test]
fn test_mbc2_bits_ramg() {
    let result = run_test(MOONEYE, "emulator-only/mbc2/bits_ramg.gb", Model::CgbE);
    assert!(result.is_passed(), "mbc2/bits_ramg test failed");
}

#[test]
fn test_mbc2_bits_romb() {
    let result = run_test(MOONEYE, "emulator-only/mbc2/bits_romb.gb", Model::CgbE);
    assert!(result.is_passed(), "mbc2/bits_romb test failed");
}

#[test]
fn test_mbc2_bits_unused() {
    let result = run_test(MOONEYE, "emulator-only/mbc2/bits_unused.gb", Model::CgbE);
    assert!(result.is_passed(), "mbc2/bits_unused test failed");
}

#[test]
fn test_mbc2_ram() {
    let result = run_test(MOONEYE, "emulator-only/mbc2/ram.gb", Model::CgbE);
    assert!(result.is_passed(), "mbc2/ram test failed");
}

#[test]
fn test_mbc2_rom_512kb() {
    let result = run_test(MOONEYE, "emulator-only/mbc2/rom_512kb.gb", Model::CgbE);
    assert!(result.is_passed(), "mbc2/rom_512kb test failed");
}

#[test]
fn test_mbc2_rom_1mb() {
    let result = run_test(MOONEYE, "emulator-only/mbc2/rom_1Mb.gb", Model::CgbE);
    assert!(result.is_passed(), "mbc2/rom_1Mb test failed");
}

#[test]
fn test_mbc2_rom_2mb() {
    let result = run_test(MOONEYE, "emulator-only/mbc2/rom_2Mb.gb", Model::CgbE);
    assert!(result.is_passed(), "mbc2/rom_2Mb test failed");
}

#[test]
fn test_mbc5_rom_512kb() {
    let result = run_test(MOONEYE, "emulator-only/mbc5/rom_512kb.gb", Model::CgbE);
    assert!(result.is_passed(), "mbc5/rom_512kb test failed");
}

#[test]
fn test_mbc5_rom_1mb() {
    let result = run_test(MOONEYE, "emulator-only/mbc5/rom_1Mb.gb", Model::CgbE);
    assert!(result.is_passed(), "mbc5/rom_1Mb test failed");
}

#[test]
fn test_mbc5_rom_2mb() {
    let result = run_test(MOONEYE, "emulator-only/mbc5/rom_2Mb.gb", Model::CgbE);
    assert!(result.is_passed(), "mbc5/rom_2Mb test failed");
}

#[test]
fn test_mbc5_rom_4mb() {
    let result = run_test(MOONEYE, "emulator-only/mbc5/rom_4Mb.gb", Model::CgbE);
    assert!(result.is_passed(), "mbc5/rom_4Mb test failed");
}

#[test]
fn test_mbc5_rom_8mb() {
    let result = run_test(MOONEYE, "emulator-only/mbc5/rom_8Mb.gb", Model::CgbE);
    assert!(result.is_passed(), "mbc5/rom_8Mb test failed");
}

#[test]
fn test_mbc5_rom_16mb() {
    let result = run_test(MOONEYE, "emulator-only/mbc5/rom_16Mb.gb", Model::CgbE);
    assert!(result.is_passed(), "mbc5/rom_16Mb test failed");
}

#[test]
fn test_mbc5_rom_32mb() {
    let result = run_test(MOONEYE, "emulator-only/mbc5/rom_32Mb.gb", Model::CgbE);
    assert!(result.is_passed(), "mbc5/rom_32Mb test failed");
}

#[test]
fn test_mbc5_rom_64mb() {
    let result = run_test(MOONEYE, "emulator-only/mbc5/rom_64Mb.gb", Model::CgbE);
    assert!(result.is_passed(), "mbc5/rom_64Mb test failed");
}

// =============================================================================
// manual-only/ tests
// =============================================================================

#[test]
fn test_manual_sprite_priority_dmg() {
    let result = run_screenshot_test(WILBERTPOL, "manual-only/sprite_priority.gb", Model::DmgB);
    assert!(
        result.is_passed(),
        "manual-only/sprite_priority DMG test failed"
    );
}

#[test]
fn test_manual_sprite_priority_cgb() {
    let result = run_screenshot_test(WILBERTPOL, "manual-only/sprite_priority.gb", Model::CgbE);
    assert!(
        result.is_passed(),
        "manual-only/sprite_priority CGB test failed"
    );
}
