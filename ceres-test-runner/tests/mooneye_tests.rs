//! Integration tests using the Mooneye Test Suite
//!
//! The Mooneye Test Suite is a comprehensive collection of hardware-validated Game Boy test ROMs
//! that test various low-level behaviors including CPU instructions, timing, interrupts, PPU
//! rendering, timer operations, serial communication, and OAM DMA.
//!
//! ## Test Protocol
//!
//! Mooneye tests use a specific protocol to signal pass/fail:
//! - **Pass**: CPU registers contain Fibonacci numbers (B=3, C=5, D=8, E=13, H=21, L=34)
//! - **Fail**: All CPU registers contain 0x42
//! - **Exit**: Tests execute the `ld b, b` instruction (opcode 0x40) when finished
//!
//! ## Test Organization
//!
//! Tests are organized by category matching the test ROM directory structure:
//! - Root level: Timing, interrupt, and instruction tests
//! - `bits/`: Register and memory tests
//! - `instr/`: Instruction behavior tests
//! - `interrupts/`: Interrupt handling tests
//! - `oam_dma/`: OAM DMA transfer tests
//! - `ppu/`: PPU timing and behavior tests
//! - `serial/`: Serial communication tests
//! - `timer/`: Timer and DIV register tests
//!
//! ## Model Selection
//!
//! Tests with model hints in their names run on specific hardware:
//! - `-dmg0`, `-dmgABC`, `-dmgABCmgb`: DMG (original Game Boy)
//! - `-mgb`: MGB (Game Boy Pocket)
//! - `-sgb`, `-sgb2`, `-GS`: SGB/SGB2 (Super Game Boy)
//! - `-S`: SGB and SGB2
//! - Tests without hints default to CGB (Game Boy Color)
//!
//! ## Ignored Tests
//!
//! Tests marked with `#[ignore]` currently fail and need emulation improvements.
//! These can be run individually with: `cargo test -- --ignored <test_name>`
//!
//! ## Current Status
//!
//! Out of 75 acceptance tests:
//! - **42 tests pass** (56% pass rate)
//! - **33 tests fail** and are marked with `#[ignore]`
//!
//! Failing tests need improvements in:
//! - Boot ROM behavior and register initialization
//! - PPU timing edge cases
//! - Timer/interrupt edge cases
//! - OAM DMA sources and timing
//! - Serial communication timing

use ceres_core::Model;
use ceres_test_runner::{
    load_test_rom,
    test_runner::{TestConfig, TestResult, TestRunner, timeouts},
};

/// Helper function to run a Mooneye acceptance test
fn run_mooneye_test(path: &str, model: Model) -> TestResult {
    let rom = match load_test_rom(path) {
        Ok(rom) => rom,
        Err(e) => return TestResult::Failed(format!("Failed to load test ROM: {e}")),
    };

    let config = TestConfig {
        model,
        timeout_frames: timeouts::MOONEYE_ACCEPTANCE,
        use_mooneye_validation: true,
        capture_serial: false, // Mooneye tests don't use serial output
        ..TestConfig::default()
    };

    let mut runner = match TestRunner::new(rom, config) {
        Ok(runner) => runner,
        Err(e) => return TestResult::Failed(format!("Failed to create test runner: {e}")),
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
        Model::Cgb,
    );
    assert_eq!(result, TestResult::Passed, "add_sp_e_timing test failed");
}

#[test]
fn test_mooneye_call_cc_timing() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/call_cc_timing.gb",
        Model::Cgb,
    );
    assert_eq!(result, TestResult::Passed, "call_cc_timing test failed");
}

#[test]
#[ignore] // TODO: Enable when passing
fn test_mooneye_call_cc_timing2() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/call_cc_timing2.gb",
        Model::Cgb,
    );
    assert_eq!(result, TestResult::Passed, "call_cc_timing2 test failed");
}

#[test]
fn test_mooneye_call_timing() {
    let result = run_mooneye_test("mooneye-test-suite/acceptance/call_timing.gb", Model::Cgb);
    assert_eq!(result, TestResult::Passed, "call_timing test failed");
}

#[test]
#[ignore] // TODO: Enable when passing
fn test_mooneye_call_timing2() {
    let result = run_mooneye_test("mooneye-test-suite/acceptance/call_timing2.gb", Model::Cgb);
    assert_eq!(result, TestResult::Passed, "call_timing2 test failed");
}

#[test]
fn test_mooneye_di_timing_gs() {
    let result = run_mooneye_test("mooneye-test-suite/acceptance/di_timing-GS.gb", Model::Dmg);
    assert_eq!(result, TestResult::Passed, "di_timing-GS test failed");
}

#[test]
fn test_mooneye_div_timing() {
    let result = run_mooneye_test("mooneye-test-suite/acceptance/div_timing.gb", Model::Cgb);
    assert_eq!(result, TestResult::Passed, "div_timing test failed");
}

#[test]
fn test_mooneye_ei_sequence() {
    let result = run_mooneye_test("mooneye-test-suite/acceptance/ei_sequence.gb", Model::Cgb);
    assert_eq!(result, TestResult::Passed, "ei_sequence test failed");
}

#[test]
fn test_mooneye_ei_timing() {
    let result = run_mooneye_test("mooneye-test-suite/acceptance/ei_timing.gb", Model::Cgb);
    assert_eq!(result, TestResult::Passed, "ei_timing test failed");
}

#[test]
fn test_mooneye_halt_ime0_ei() {
    let result = run_mooneye_test("mooneye-test-suite/acceptance/halt_ime0_ei.gb", Model::Cgb);
    assert_eq!(result, TestResult::Passed, "halt_ime0_ei test failed");
}

#[test]
fn test_mooneye_halt_ime0_nointr_timing() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/halt_ime0_nointr_timing.gb",
        Model::Cgb,
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
        Model::Cgb,
    );
    assert_eq!(result, TestResult::Passed, "halt_ime1_timing test failed");
}

#[test]
fn test_mooneye_halt_ime1_timing2_gs() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/halt_ime1_timing2-GS.gb",
        Model::Dmg,
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
        Model::Cgb,
    );
    assert_eq!(result, TestResult::Passed, "if_ie_registers test failed");
}

#[test]
fn test_mooneye_intr_timing() {
    let result = run_mooneye_test("mooneye-test-suite/acceptance/intr_timing.gb", Model::Cgb);
    assert_eq!(result, TestResult::Passed, "intr_timing test failed");
}

#[test]
fn test_mooneye_jp_cc_timing() {
    let result = run_mooneye_test("mooneye-test-suite/acceptance/jp_cc_timing.gb", Model::Cgb);
    assert_eq!(result, TestResult::Passed, "jp_cc_timing test failed");
}

#[test]
fn test_mooneye_jp_timing() {
    let result = run_mooneye_test("mooneye-test-suite/acceptance/jp_timing.gb", Model::Cgb);
    assert_eq!(result, TestResult::Passed, "jp_timing test failed");
}

#[test]
fn test_mooneye_ld_hl_sp_e_timing() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/ld_hl_sp_e_timing.gb",
        Model::Cgb,
    );
    assert_eq!(result, TestResult::Passed, "ld_hl_sp_e_timing test failed");
}

#[test]
fn test_mooneye_oam_dma_restart() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/oam_dma_restart.gb",
        Model::Cgb,
    );
    assert_eq!(result, TestResult::Passed, "oam_dma_restart test failed");
}

#[test]
#[ignore] // TODO: Enable when passing
fn test_mooneye_oam_dma_start() {
    let result = run_mooneye_test("mooneye-test-suite/acceptance/oam_dma_start.gb", Model::Cgb);
    assert_eq!(result, TestResult::Passed, "oam_dma_start test failed");
}

#[test]
fn test_mooneye_oam_dma_timing() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/oam_dma_timing.gb",
        Model::Cgb,
    );
    assert_eq!(result, TestResult::Passed, "oam_dma_timing test failed");
}

#[test]
fn test_mooneye_pop_timing() {
    let result = run_mooneye_test("mooneye-test-suite/acceptance/pop_timing.gb", Model::Cgb);
    assert_eq!(result, TestResult::Passed, "pop_timing test failed");
}

#[test]
#[ignore] // TODO: Enable when passing
fn test_mooneye_push_timing() {
    let result = run_mooneye_test("mooneye-test-suite/acceptance/push_timing.gb", Model::Cgb);
    assert_eq!(result, TestResult::Passed, "push_timing test failed");
}

#[test]
fn test_mooneye_rapid_di_ei() {
    let result = run_mooneye_test("mooneye-test-suite/acceptance/rapid_di_ei.gb", Model::Cgb);
    assert_eq!(result, TestResult::Passed, "rapid_di_ei test failed");
}

#[test]
fn test_mooneye_ret_cc_timing() {
    let result = run_mooneye_test("mooneye-test-suite/acceptance/ret_cc_timing.gb", Model::Cgb);
    assert_eq!(result, TestResult::Passed, "ret_cc_timing test failed");
}

#[test]
fn test_mooneye_ret_timing() {
    let result = run_mooneye_test("mooneye-test-suite/acceptance/ret_timing.gb", Model::Cgb);
    assert_eq!(result, TestResult::Passed, "ret_timing test failed");
}

#[test]
fn test_mooneye_reti_intr_timing() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/reti_intr_timing.gb",
        Model::Cgb,
    );
    assert_eq!(result, TestResult::Passed, "reti_intr_timing test failed");
}

#[test]
fn test_mooneye_reti_timing() {
    let result = run_mooneye_test("mooneye-test-suite/acceptance/reti_timing.gb", Model::Cgb);
    assert_eq!(result, TestResult::Passed, "reti_timing test failed");
}

#[test]
#[ignore] // TODO: Enable when passing
fn test_mooneye_rst_timing() {
    let result = run_mooneye_test("mooneye-test-suite/acceptance/rst_timing.gb", Model::Cgb);
    assert_eq!(result, TestResult::Passed, "rst_timing test failed");
}

// Boot register tests - model-specific
#[test]
#[ignore] // TODO: Enable when passing - DMG CPU revision 0
fn test_mooneye_boot_div_dmg0() {
    let result = run_mooneye_test("mooneye-test-suite/acceptance/boot_div-dmg0.gb", Model::Dmg);
    assert_eq!(result, TestResult::Passed, "boot_div-dmg0 test failed");
}

#[test]
#[ignore] // TODO: Enable when passing - DMG CPU revisions A/B/C + MGB
fn test_mooneye_boot_div_dmgabcmgb() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/boot_div-dmgABCmgb.gb",
        Model::Dmg,
    );
    assert_eq!(result, TestResult::Passed, "boot_div-dmgABCmgb test failed");
}

#[test]
#[ignore] // TODO: Enable when passing - SGB hint
fn test_mooneye_boot_div_s() {
    let result = run_mooneye_test("mooneye-test-suite/acceptance/boot_div-S.gb", Model::Dmg);
    assert_eq!(result, TestResult::Passed, "boot_div-S test failed");
}

#[test]
#[ignore] // TODO: Enable when passing - SGB hint
fn test_mooneye_boot_div2_s() {
    let result = run_mooneye_test("mooneye-test-suite/acceptance/boot_div2-S.gb", Model::Dmg);
    assert_eq!(result, TestResult::Passed, "boot_div2-S test failed");
}

#[test]
#[ignore] // TODO: Enable when passing - DMG CPU revision 0
fn test_mooneye_boot_hwio_dmg0() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/boot_hwio-dmg0.gb",
        Model::Dmg,
    );
    assert_eq!(result, TestResult::Passed, "boot_hwio-dmg0 test failed");
}

#[test]
#[ignore] // TODO: Enable when passing - DMG CPU revisions A/B/C + MGB
fn test_mooneye_boot_hwio_dmgabcmgb() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/boot_hwio-dmgABCmgb.gb",
        Model::Dmg,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "boot_hwio-dmgABCmgb test failed"
    );
}

#[test]
#[ignore] // TODO: Enable when passing - SGB hint
fn test_mooneye_boot_hwio_s() {
    let result = run_mooneye_test("mooneye-test-suite/acceptance/boot_hwio-S.gb", Model::Dmg);
    assert_eq!(result, TestResult::Passed, "boot_hwio-S test failed");
}

#[test]
#[ignore] // TODO: Enable when passing - DMG CPU revision 0
fn test_mooneye_boot_regs_dmg0() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/boot_regs-dmg0.gb",
        Model::Dmg,
    );
    assert_eq!(result, TestResult::Passed, "boot_regs-dmg0 test failed");
}

#[test]
fn test_mooneye_boot_regs_dmgabc() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/boot_regs-dmgABC.gb",
        Model::Dmg,
    );
    assert_eq!(result, TestResult::Passed, "boot_regs-dmgABC test failed");
}

#[test]
fn test_mooneye_boot_regs_mgb() {
    let result = run_mooneye_test("mooneye-test-suite/acceptance/boot_regs-mgb.gb", Model::Mgb);
    assert_eq!(result, TestResult::Passed, "boot_regs-mgb test failed");
}

#[test]
#[ignore] // TODO: Enable when passing - SGB hint
fn test_mooneye_boot_regs_sgb() {
    let result = run_mooneye_test("mooneye-test-suite/acceptance/boot_regs-sgb.gb", Model::Dmg);
    assert_eq!(result, TestResult::Passed, "boot_regs-sgb test failed");
}

#[test]
#[ignore] // TODO: Enable when passing - SGB2 hint
fn test_mooneye_boot_regs_sgb2() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/boot_regs-sgb2.gb",
        Model::Dmg,
    );
    assert_eq!(result, TestResult::Passed, "boot_regs-sgb2 test failed");
}

// =============================================================================
// bits/ Tests
// =============================================================================

#[test]
fn test_mooneye_bits_mem_oam() {
    let result = run_mooneye_test("mooneye-test-suite/acceptance/bits/mem_oam.gb", Model::Cgb);
    assert_eq!(result, TestResult::Passed, "bits/mem_oam test failed");
}

#[test]
fn test_mooneye_bits_reg_f() {
    let result = run_mooneye_test("mooneye-test-suite/acceptance/bits/reg_f.gb", Model::Cgb);
    assert_eq!(result, TestResult::Passed, "bits/reg_f test failed");
}

#[test]
#[ignore] // TODO: Enable when passing - GS hint
fn test_mooneye_bits_unused_hwio_gs() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/bits/unused_hwio-GS.gb",
        Model::Dmg,
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
    let result = run_mooneye_test("mooneye-test-suite/acceptance/instr/daa.gb", Model::Cgb);
    assert_eq!(result, TestResult::Passed, "instr/daa test failed");
}

// =============================================================================
// interrupts/ Tests
// =============================================================================

#[test]
#[ignore] // TODO: Enable when passing
fn test_mooneye_interrupts_ie_push() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/interrupts/ie_push.gb",
        Model::Cgb,
    );
    assert_eq!(result, TestResult::Passed, "interrupts/ie_push test failed");
}

// =============================================================================
// oam_dma/ Tests
// =============================================================================

#[test]
fn test_mooneye_oam_dma_basic() {
    let result = run_mooneye_test("mooneye-test-suite/acceptance/oam_dma/basic.gb", Model::Cgb);
    assert_eq!(result, TestResult::Passed, "oam_dma/basic test failed");
}

#[test]
fn test_mooneye_oam_dma_reg_read() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/oam_dma/reg_read.gb",
        Model::Cgb,
    );
    assert_eq!(result, TestResult::Passed, "oam_dma/reg_read test failed");
}

#[test]
#[ignore] // TODO: Enable when passing - GS hint
fn test_mooneye_oam_dma_sources_gs() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/oam_dma/sources-GS.gb",
        Model::Dmg,
    );
    assert_eq!(result, TestResult::Passed, "oam_dma/sources-GS test failed");
}

// =============================================================================
// ppu/ Tests
// =============================================================================

#[test]
#[ignore] // TODO: Enable when passing - GS hint
fn test_mooneye_ppu_hblank_ly_scx_timing_gs() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/ppu/hblank_ly_scx_timing-GS.gb",
        Model::Dmg,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "ppu/hblank_ly_scx_timing-GS test failed"
    );
}

#[test]
fn test_mooneye_ppu_intr_1_2_timing_gs() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/ppu/intr_1_2_timing-GS.gb",
        Model::Dmg,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "ppu/intr_1_2_timing-GS test failed"
    );
}

#[test]
fn test_mooneye_ppu_intr_2_0_timing() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/ppu/intr_2_0_timing.gb",
        Model::Cgb,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "ppu/intr_2_0_timing test failed"
    );
}

#[test]
#[ignore] // TODO: Enable when passing
fn test_mooneye_ppu_intr_2_mode0_timing() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/ppu/intr_2_mode0_timing.gb",
        Model::Cgb,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "ppu/intr_2_mode0_timing test failed"
    );
}

#[test]
#[ignore] // TODO: Enable when passing
fn test_mooneye_ppu_intr_2_mode0_timing_sprites() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/ppu/intr_2_mode0_timing_sprites.gb",
        Model::Cgb,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "ppu/intr_2_mode0_timing_sprites test failed"
    );
}

#[test]
#[ignore] // TODO: Enable when passing
fn test_mooneye_ppu_intr_2_mode3_timing() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/ppu/intr_2_mode3_timing.gb",
        Model::Cgb,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "ppu/intr_2_mode3_timing test failed"
    );
}

#[test]
#[ignore] // TODO: Enable when passing
fn test_mooneye_ppu_intr_2_oam_ok_timing() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/ppu/intr_2_oam_ok_timing.gb",
        Model::Cgb,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "ppu/intr_2_oam_ok_timing test failed"
    );
}

#[test]
#[ignore] // TODO: Enable when passing - GS hint
fn test_mooneye_ppu_lcdon_timing_gs() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/ppu/lcdon_timing-GS.gb",
        Model::Dmg,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "ppu/lcdon_timing-GS test failed"
    );
}

#[test]
#[ignore] // TODO: Enable when passing - GS hint
fn test_mooneye_ppu_lcdon_write_timing_gs() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/ppu/lcdon_write_timing-GS.gb",
        Model::Dmg,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "ppu/lcdon_write_timing-GS test failed"
    );
}

#[test]
#[ignore] // TODO: Enable when passing
fn test_mooneye_ppu_stat_irq_blocking() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/ppu/stat_irq_blocking.gb",
        Model::Cgb,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "ppu/stat_irq_blocking test failed"
    );
}

#[test]
#[ignore] // TODO: Enable when passing
fn test_mooneye_ppu_stat_lyc_onoff() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/ppu/stat_lyc_onoff.gb",
        Model::Cgb,
    );
    assert_eq!(result, TestResult::Passed, "ppu/stat_lyc_onoff test failed");
}

#[test]
#[ignore] // TODO: Enable when passing - GS hint
fn test_mooneye_ppu_vblank_stat_intr_gs() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/ppu/vblank_stat_intr-GS.gb",
        Model::Dmg,
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
#[ignore] // TODO: Enable when passing - dmgABCmgb hint
fn test_mooneye_serial_boot_sclk_align_dmgabcmgb() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/serial/boot_sclk_align-dmgABCmgb.gb",
        Model::Dmg,
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
        Model::Cgb,
    );
    assert_eq!(result, TestResult::Passed, "timer/div_write test failed");
}

#[test]
#[ignore] // TODO: Enable when passing
fn test_mooneye_timer_rapid_toggle() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/timer/rapid_toggle.gb",
        Model::Cgb,
    );
    assert_eq!(result, TestResult::Passed, "timer/rapid_toggle test failed");
}

#[test]
fn test_mooneye_timer_tim00() {
    let result = run_mooneye_test("mooneye-test-suite/acceptance/timer/tim00.gb", Model::Cgb);
    assert_eq!(result, TestResult::Passed, "timer/tim00 test failed");
}

#[test]
fn test_mooneye_timer_tim00_div_trigger() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/timer/tim00_div_trigger.gb",
        Model::Cgb,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "timer/tim00_div_trigger test failed"
    );
}

#[test]
fn test_mooneye_timer_tim01() {
    let result = run_mooneye_test("mooneye-test-suite/acceptance/timer/tim01.gb", Model::Cgb);
    assert_eq!(result, TestResult::Passed, "timer/tim01 test failed");
}

#[test]
fn test_mooneye_timer_tim01_div_trigger() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/timer/tim01_div_trigger.gb",
        Model::Cgb,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "timer/tim01_div_trigger test failed"
    );
}

#[test]
fn test_mooneye_timer_tim10() {
    let result = run_mooneye_test("mooneye-test-suite/acceptance/timer/tim10.gb", Model::Cgb);
    assert_eq!(result, TestResult::Passed, "timer/tim10 test failed");
}

#[test]
fn test_mooneye_timer_tim10_div_trigger() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/timer/tim10_div_trigger.gb",
        Model::Cgb,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "timer/tim10_div_trigger test failed"
    );
}

#[test]
fn test_mooneye_timer_tim11() {
    let result = run_mooneye_test("mooneye-test-suite/acceptance/timer/tim11.gb", Model::Cgb);
    assert_eq!(result, TestResult::Passed, "timer/tim11 test failed");
}

#[test]
fn test_mooneye_timer_tim11_div_trigger() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/timer/tim11_div_trigger.gb",
        Model::Cgb,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "timer/tim11_div_trigger test failed"
    );
}

#[test]
#[ignore] // TODO: Enable when passing
fn test_mooneye_timer_tima_reload() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/timer/tima_reload.gb",
        Model::Cgb,
    );
    assert_eq!(result, TestResult::Passed, "timer/tima_reload test failed");
}

#[test]
#[ignore] // TODO: Enable when passing
fn test_mooneye_timer_tima_write_reloading() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/timer/tima_write_reloading.gb",
        Model::Cgb,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "timer/tima_write_reloading test failed"
    );
}

#[test]
#[ignore] // TODO: Enable when passing
fn test_mooneye_timer_tma_write_reloading() {
    let result = run_mooneye_test(
        "mooneye-test-suite/acceptance/timer/tma_write_reloading.gb",
        Model::Cgb,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "timer/tma_write_reloading test failed"
    );
}
