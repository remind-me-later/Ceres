//! Integration tests using Blargg test ROMs
//!
//! These tests validate the CPU instruction implementation against
//! Blargg's comprehensive test suite.

use ceres_tests::{
    load_test_rom,
    test_runner::{timeouts, TestConfig, TestResult, TestRunner},
};

/// Helper to run a test ROM with a specific timeout
fn run_test_rom_with_timeout(path: &str, timeout_frames: u32) -> TestResult {
    let rom = match load_test_rom(path) {
        Ok(rom) => rom,
        Err(e) => return TestResult::Failed(format!("Failed to load test ROM: {e}")),
    };

    let config = TestConfig {
        timeout_frames,
        ..TestConfig::default()
    };

    let mut runner = match TestRunner::new(rom, config) {
        Ok(runner) => runner,
        Err(e) => return TestResult::Failed(format!("Failed to create test runner: {e}")),
    };

    runner.run()
}

// ============================================================================
// CPU Instructions Tests
// ============================================================================

#[test]
#[ignore = "Requires test ROMs to be downloaded"]
fn test_blargg_cpu_instrs_01_special() {
    let result = run_test_rom_with_timeout(
        "blargg/cpu_instrs/individual/01-special.gb",
        timeouts::CPU_INSTRS,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "CPU special instructions test failed"
    );
}

#[test]
#[ignore = "Requires test ROMs to be downloaded"]
fn test_blargg_cpu_instrs_02_interrupts() {
    let result = run_test_rom_with_timeout(
        "blargg/cpu_instrs/individual/02-interrupts.gb",
        timeouts::CPU_INSTRS,
    );
    assert_eq!(result, TestResult::Passed, "CPU interrupts test failed");
}

#[test]
#[ignore = "Requires test ROMs to be downloaded"]
fn test_blargg_cpu_instrs_03_op_sp_hl() {
    let result = run_test_rom_with_timeout(
        "blargg/cpu_instrs/individual/03-op sp,hl.gb",
        timeouts::CPU_INSTRS,
    );
    assert_eq!(result, TestResult::Passed, "CPU OP SP,HL test failed");
}

#[test]
#[ignore = "Requires test ROMs to be downloaded"]
fn test_blargg_cpu_instrs_04_op_r_imm() {
    let result = run_test_rom_with_timeout(
        "blargg/cpu_instrs/individual/04-op r,imm.gb",
        timeouts::CPU_INSTRS,
    );
    assert_eq!(result, TestResult::Passed, "CPU OP R,IMM test failed");
}

#[test]
#[ignore = "Requires test ROMs to be downloaded"]
fn test_blargg_cpu_instrs_05_op_rp() {
    let result = run_test_rom_with_timeout(
        "blargg/cpu_instrs/individual/05-op rp.gb",
        timeouts::CPU_INSTRS,
    );
    assert_eq!(result, TestResult::Passed, "CPU OP RP test failed");
}

#[test]
#[ignore = "Requires test ROMs to be downloaded"]
fn test_blargg_cpu_instrs_06_ld_r_r() {
    let result = run_test_rom_with_timeout(
        "blargg/cpu_instrs/individual/06-ld r,r.gb",
        timeouts::CPU_INSTRS,
    );
    assert_eq!(result, TestResult::Passed, "CPU LD R,R test failed");
}

#[test]
#[ignore = "Requires test ROMs to be downloaded"]
fn test_blargg_cpu_instrs_07_jr_jp_call_ret_rst() {
    let result = run_test_rom_with_timeout(
        "blargg/cpu_instrs/individual/07-jr,jp,call,ret,rst.gb",
        timeouts::CPU_INSTRS,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "CPU JR,JP,CALL,RET,RST test failed"
    );
}

#[test]
#[ignore = "Requires test ROMs to be downloaded"]
fn test_blargg_cpu_instrs_08_misc_instrs() {
    let result = run_test_rom_with_timeout(
        "blargg/cpu_instrs/individual/08-misc instrs.gb",
        timeouts::CPU_INSTRS,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "CPU misc instructions test failed"
    );
}

#[test]
#[ignore = "Requires test ROMs to be downloaded"]
fn test_blargg_cpu_instrs_09_op_r_r() {
    let result = run_test_rom_with_timeout(
        "blargg/cpu_instrs/individual/09-op r,r.gb",
        timeouts::CPU_INSTRS,
    );
    assert_eq!(result, TestResult::Passed, "CPU OP R,R test failed");
}

#[test]
#[ignore = "Requires test ROMs to be downloaded"]
fn test_blargg_cpu_instrs_10_bit_ops() {
    let result = run_test_rom_with_timeout(
        "blargg/cpu_instrs/individual/10-bit ops.gb",
        timeouts::CPU_INSTRS,
    );
    assert_eq!(result, TestResult::Passed, "CPU bit ops test failed");
}

#[test]
#[ignore = "Requires test ROMs to be downloaded"]
fn test_blargg_cpu_instrs_11_op_a_hl() {
    let result = run_test_rom_with_timeout(
        "blargg/cpu_instrs/individual/11-op a,(hl).gb",
        timeouts::CPU_INSTRS,
    );
    assert_eq!(result, TestResult::Passed, "CPU OP A,(HL) test failed");
}

// ============================================================================
// Timing Tests
// ============================================================================

#[test]
#[ignore = "Requires test ROMs to be downloaded"]
fn test_blargg_instr_timing() {
    let result = run_test_rom_with_timeout(
        "blargg/instr_timing/instr_timing.gb",
        timeouts::INSTR_TIMING,
    );
    assert_eq!(result, TestResult::Passed, "Instruction timing test failed");
}

#[test]
#[ignore = "Requires test ROMs to be downloaded"]
fn test_blargg_mem_timing_01_read_timing() {
    let result = run_test_rom_with_timeout(
        "blargg/mem_timing/individual/01-read_timing.gb",
        timeouts::MEM_TIMING,
    );
    assert_eq!(result, TestResult::Passed, "Memory read timing test failed");
}

#[test]
#[ignore = "Requires test ROMs to be downloaded"]
fn test_blargg_mem_timing_02_write_timing() {
    let result = run_test_rom_with_timeout(
        "blargg/mem_timing/individual/02-write_timing.gb",
        timeouts::MEM_TIMING,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "Memory write timing test failed"
    );
}

#[test]
#[ignore = "Requires test ROMs to be downloaded"]
fn test_blargg_mem_timing_03_modify_timing() {
    let result = run_test_rom_with_timeout(
        "blargg/mem_timing/individual/03-modify_timing.gb",
        timeouts::MEM_TIMING,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "Memory modify timing test failed"
    );
}

// ============================================================================
// DMG Sound Tests
// ============================================================================

#[test]
#[ignore = "Requires test ROMs to be downloaded"]
fn test_blargg_dmg_sound_01_registers() {
    let result = run_test_rom_with_timeout(
        "blargg/dmg_sound/rom_singles/01-registers.gb",
        timeouts::DMG_SOUND,
    );
    assert_eq!(result, TestResult::Passed, "DMG sound registers test failed");
}

#[test]
#[ignore = "Requires test ROMs to be downloaded"]
fn test_blargg_dmg_sound_02_len_ctr() {
    let result = run_test_rom_with_timeout(
        "blargg/dmg_sound/rom_singles/02-len ctr.gb",
        timeouts::DMG_SOUND,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "DMG sound length counter test failed"
    );
}

#[test]
#[ignore = "Requires test ROMs to be downloaded"]
fn test_blargg_dmg_sound_03_trigger() {
    let result = run_test_rom_with_timeout(
        "blargg/dmg_sound/rom_singles/03-trigger.gb",
        timeouts::DMG_SOUND,
    );
    assert_eq!(result, TestResult::Passed, "DMG sound trigger test failed");
}

#[test]
#[ignore = "Requires test ROMs to be downloaded"]
fn test_blargg_dmg_sound_04_sweep() {
    let result = run_test_rom_with_timeout(
        "blargg/dmg_sound/rom_singles/04-sweep.gb",
        timeouts::DMG_SOUND,
    );
    assert_eq!(result, TestResult::Passed, "DMG sound sweep test failed");
}

#[test]
#[ignore = "Requires test ROMs to be downloaded"]
fn test_blargg_dmg_sound_05_sweep_details() {
    let result = run_test_rom_with_timeout(
        "blargg/dmg_sound/rom_singles/05-sweep details.gb",
        timeouts::DMG_SOUND,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "DMG sound sweep details test failed"
    );
}

#[test]
#[ignore = "Requires test ROMs to be downloaded"]
fn test_blargg_dmg_sound_06_overflow_on_trigger() {
    let result = run_test_rom_with_timeout(
        "blargg/dmg_sound/rom_singles/06-overflow on trigger.gb",
        timeouts::DMG_SOUND,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "DMG sound overflow on trigger test failed"
    );
}

#[test]
#[ignore = "Requires test ROMs to be downloaded"]
fn test_blargg_dmg_sound_07_len_sweep_period_sync() {
    let result = run_test_rom_with_timeout(
        "blargg/dmg_sound/rom_singles/07-len sweep period sync.gb",
        timeouts::DMG_SOUND,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "DMG sound len sweep period sync test failed"
    );
}

#[test]
#[ignore = "Requires test ROMs to be downloaded"]
fn test_blargg_dmg_sound_08_len_ctr_during_power() {
    let result = run_test_rom_with_timeout(
        "blargg/dmg_sound/rom_singles/08-len ctr during power.gb",
        timeouts::DMG_SOUND,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "DMG sound len ctr during power test failed"
    );
}

#[test]
#[ignore = "Requires test ROMs to be downloaded"]
fn test_blargg_dmg_sound_09_wave_read_while_on() {
    let result = run_test_rom_with_timeout(
        "blargg/dmg_sound/rom_singles/09-wave read while on.gb",
        timeouts::DMG_SOUND,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "DMG sound wave read while on test failed"
    );
}

#[test]
#[ignore = "Requires test ROMs to be downloaded"]
fn test_blargg_dmg_sound_10_wave_trigger_while_on() {
    let result = run_test_rom_with_timeout(
        "blargg/dmg_sound/rom_singles/10-wave trigger while on.gb",
        timeouts::DMG_SOUND,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "DMG sound wave trigger while on test failed"
    );
}

#[test]
#[ignore = "Requires test ROMs to be downloaded"]
fn test_blargg_dmg_sound_11_regs_after_power() {
    let result = run_test_rom_with_timeout(
        "blargg/dmg_sound/rom_singles/11-regs after power.gb",
        timeouts::DMG_SOUND,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "DMG sound regs after power test failed"
    );
}

#[test]
#[ignore = "Requires test ROMs to be downloaded"]
fn test_blargg_dmg_sound_12_wave_write_while_on() {
    let result = run_test_rom_with_timeout(
        "blargg/dmg_sound/rom_singles/12-wave write while on.gb",
        timeouts::DMG_SOUND,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "DMG sound wave write while on test failed"
    );
}

// ============================================================================
// CGB Sound Tests
// ============================================================================

#[test]
#[ignore = "Requires test ROMs to be downloaded"]
fn test_blargg_cgb_sound_01_registers() {
    let result = run_test_rom_with_timeout(
        "blargg/cgb_sound/rom_singles/01-registers.gb",
        timeouts::CGB_SOUND,
    );
    assert_eq!(result, TestResult::Passed, "CGB sound registers test failed");
}

#[test]
#[ignore = "Requires test ROMs to be downloaded"]
fn test_blargg_cgb_sound_02_len_ctr() {
    let result = run_test_rom_with_timeout(
        "blargg/cgb_sound/rom_singles/02-len ctr.gb",
        timeouts::CGB_SOUND,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "CGB sound length counter test failed"
    );
}

#[test]
#[ignore = "Requires test ROMs to be downloaded"]
fn test_blargg_cgb_sound_03_trigger() {
    let result = run_test_rom_with_timeout(
        "blargg/cgb_sound/rom_singles/03-trigger.gb",
        timeouts::CGB_SOUND,
    );
    assert_eq!(result, TestResult::Passed, "CGB sound trigger test failed");
}

#[test]
#[ignore = "Requires test ROMs to be downloaded"]
fn test_blargg_cgb_sound_04_sweep() {
    let result = run_test_rom_with_timeout(
        "blargg/cgb_sound/rom_singles/04-sweep.gb",
        timeouts::CGB_SOUND,
    );
    assert_eq!(result, TestResult::Passed, "CGB sound sweep test failed");
}

#[test]
#[ignore = "Requires test ROMs to be downloaded"]
fn test_blargg_cgb_sound_05_sweep_details() {
    let result = run_test_rom_with_timeout(
        "blargg/cgb_sound/rom_singles/05-sweep details.gb",
        timeouts::CGB_SOUND,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "CGB sound sweep details test failed"
    );
}

#[test]
#[ignore = "Requires test ROMs to be downloaded"]
fn test_blargg_cgb_sound_06_overflow_on_trigger() {
    let result = run_test_rom_with_timeout(
        "blargg/cgb_sound/rom_singles/06-overflow on trigger.gb",
        timeouts::CGB_SOUND,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "CGB sound overflow on trigger test failed"
    );
}

#[test]
#[ignore = "Requires test ROMs to be downloaded"]
fn test_blargg_cgb_sound_07_len_sweep_period_sync() {
    let result = run_test_rom_with_timeout(
        "blargg/cgb_sound/rom_singles/07-len sweep period sync.gb",
        timeouts::CGB_SOUND,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "CGB sound len sweep period sync test failed"
    );
}

#[test]
#[ignore = "Requires test ROMs to be downloaded"]
fn test_blargg_cgb_sound_08_len_ctr_during_power() {
    let result = run_test_rom_with_timeout(
        "blargg/cgb_sound/rom_singles/08-len ctr during power.gb",
        timeouts::CGB_SOUND,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "CGB sound len ctr during power test failed"
    );
}

#[test]
#[ignore = "Requires test ROMs to be downloaded"]
fn test_blargg_cgb_sound_09_wave_read_while_on() {
    let result = run_test_rom_with_timeout(
        "blargg/cgb_sound/rom_singles/09-wave read while on.gb",
        timeouts::CGB_SOUND,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "CGB sound wave read while on test failed"
    );
}

#[test]
#[ignore = "Requires test ROMs to be downloaded"]
fn test_blargg_cgb_sound_10_wave_trigger_while_on() {
    let result = run_test_rom_with_timeout(
        "blargg/cgb_sound/rom_singles/10-wave trigger while on.gb",
        timeouts::CGB_SOUND,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "CGB sound wave trigger while on test failed"
    );
}

#[test]
#[ignore = "Requires test ROMs to be downloaded"]
fn test_blargg_cgb_sound_11_regs_after_power() {
    let result = run_test_rom_with_timeout(
        "blargg/cgb_sound/rom_singles/11-regs after power.gb",
        timeouts::CGB_SOUND,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "CGB sound regs after power test failed"
    );
}

#[test]
#[ignore = "Requires test ROMs to be downloaded"]
fn test_blargg_cgb_sound_12_wave() {
    let result = run_test_rom_with_timeout(
        "blargg/cgb_sound/rom_singles/12-wave.gb",
        timeouts::CGB_SOUND,
    );
    assert_eq!(result, TestResult::Passed, "CGB sound wave test failed");
}

// ============================================================================
// OAM Bug Tests
// ============================================================================

#[test]
#[ignore = "Requires test ROMs to be downloaded"]
fn test_blargg_oam_bug_1_lcd_sync() {
    let result =
        run_test_rom_with_timeout("blargg/oam_bug/rom_singles/1-lcd_sync.gb", timeouts::OAM_BUG);
    assert_eq!(result, TestResult::Passed, "OAM bug LCD sync test failed");
}

#[test]
#[ignore = "Requires test ROMs to be downloaded"]
fn test_blargg_oam_bug_2_causes() {
    let result =
        run_test_rom_with_timeout("blargg/oam_bug/rom_singles/2-causes.gb", timeouts::OAM_BUG);
    assert_eq!(result, TestResult::Passed, "OAM bug causes test failed");
}

#[test]
#[ignore = "Requires test ROMs to be downloaded"]
fn test_blargg_oam_bug_3_non_causes() {
    let result = run_test_rom_with_timeout(
        "blargg/oam_bug/rom_singles/3-non_causes.gb",
        timeouts::OAM_BUG,
    );
    assert_eq!(result, TestResult::Passed, "OAM bug non-causes test failed");
}

#[test]
#[ignore = "Requires test ROMs to be downloaded"]
fn test_blargg_oam_bug_4_scanline_timing() {
    let result = run_test_rom_with_timeout(
        "blargg/oam_bug/rom_singles/4-scanline_timing.gb",
        timeouts::OAM_BUG,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "OAM bug scanline timing test failed"
    );
}

#[test]
#[ignore = "Requires test ROMs to be downloaded"]
fn test_blargg_oam_bug_5_timing_bug() {
    let result = run_test_rom_with_timeout(
        "blargg/oam_bug/rom_singles/5-timing_bug.gb",
        timeouts::OAM_BUG,
    );
    assert_eq!(result, TestResult::Passed, "OAM bug timing bug test failed");
}

#[test]
#[ignore = "Requires test ROMs to be downloaded"]
fn test_blargg_oam_bug_6_timing_no_bug() {
    let result = run_test_rom_with_timeout(
        "blargg/oam_bug/rom_singles/6-timing_no_bug.gb",
        timeouts::OAM_BUG,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "OAM bug timing no bug test failed"
    );
}

#[test]
#[ignore = "Requires test ROMs to be downloaded"]
fn test_blargg_oam_bug_7_timing_effect() {
    let result = run_test_rom_with_timeout(
        "blargg/oam_bug/rom_singles/7-timing_effect.gb",
        timeouts::OAM_BUG,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "OAM bug timing effect test failed"
    );
}

#[test]
#[ignore = "Requires test ROMs to be downloaded"]
fn test_blargg_oam_bug_8_instr_effect() {
    let result = run_test_rom_with_timeout(
        "blargg/oam_bug/rom_singles/8-instr_effect.gb",
        timeouts::OAM_BUG,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "OAM bug instr effect test failed"
    );
}

// ============================================================================
// Misc Tests
// ============================================================================

#[test]
#[ignore = "Requires test ROMs to be downloaded"]
fn test_blargg_halt_bug() {
    let result = run_test_rom_with_timeout("blargg/halt_bug.gb", timeouts::HALT_BUG);
    assert_eq!(result, TestResult::Passed, "Halt bug test failed");
}

#[test]
#[ignore = "Requires test ROMs to be downloaded"]
fn test_blargg_interrupt_time() {
    let result = run_test_rom_with_timeout(
        "blargg/interrupt_time/interrupt_time.gb",
        timeouts::INTERRUPT_TIME,
    );
    assert_eq!(result, TestResult::Passed, "Interrupt time test failed");
}
