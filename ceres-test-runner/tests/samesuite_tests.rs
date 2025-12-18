use ceres_core::Model;
use ceres_test_runner::{
    load_test_rom,
    test_runner::{TestConfig, TestResult, TestRunner, timeouts},
};

/// Helper function to run a SameSuite test
fn run_samesuite_test(path: &str, model: Model) -> TestResult {
    let rom = match load_test_rom(path) {
        Ok(rom) => rom,
        Err(e) => return TestResult::Failed(format!("Failed to load test ROM: {e}")),
    };

    let config = TestConfig {
        model,
        timeout_frames: timeouts::MOONEYE_ACCEPTANCE, // Use same timeout as Mooneye
        use_mooneye_validation: true,                 // SameSuite uses same Fibonacci validation
        capture_serial: false,
        ..TestConfig::default()
    };

    let mut runner = match TestRunner::new(rom, config) {
        Ok(runner) => runner,
        Err(e) => return TestResult::Failed(format!("Failed to create test runner: {e}")),
    };

    runner.run()
}

// =============================================================================
// DMA Tests
// =============================================================================

#[test]
fn test_samesuite_gbc_dma_cont() {
    let result = run_samesuite_test("same-suite/dma/gbc_dma_cont.gb", Model::CgbE);
    assert_eq!(result, TestResult::Passed, "dma/gbc_dma_cont test failed");
}

#[test]
fn test_samesuite_gdma_addr_mask() {
    let result = run_samesuite_test("same-suite/dma/gdma_addr_mask.gb", Model::CgbE);
    assert_eq!(result, TestResult::Passed, "dma/gdma_addr_mask test failed");
}

#[test]
#[ignore] // TODO: Enable when passing - HDMA with LCD off behavior needs fixing
fn test_samesuite_hdma_lcd_off() {
    let result = run_samesuite_test("same-suite/dma/hdma_lcd_off.gb", Model::CgbE);
    assert_eq!(result, TestResult::Passed, "dma/hdma_lcd_off test failed");
}

#[test]
#[ignore] // TODO: Enable when passing - HDMA mode 0 (General Purpose DMA) needs fixing
fn test_samesuite_hdma_mode0() {
    let result = run_samesuite_test("same-suite/dma/hdma_mode0.gb", Model::CgbE);
    assert_eq!(result, TestResult::Passed, "dma/hdma_mode0 test failed");
}

// =============================================================================
// Interrupt Tests
// =============================================================================

#[test]
fn test_samesuite_ei_delay_halt() {
    let result = run_samesuite_test("same-suite/interrupt/ei_delay_halt.gb", Model::CgbE);
    assert_eq!(
        result,
        TestResult::Passed,
        "interrupt/ei_delay_halt test failed"
    );
}

// =============================================================================
// APU Tests
// =============================================================================

// Misc

#[test]
fn test_samesuite_apu_div_trigger_volume_10() {
    let result = run_samesuite_test("same-suite/apu/div_trigger_volume_10.gb", Model::CgbE);
    assert_eq!(
        result,
        TestResult::Passed,
        "apu/div_trigger_volume_10 test failed"
    );
}

#[test]
fn test_samesuite_apu_div_write_trigger_10() {
    let result = run_samesuite_test("same-suite/apu/div_write_trigger_10.gb", Model::CgbE);
    assert_eq!(
        result,
        TestResult::Passed,
        "apu/div_write_trigger_10 test failed"
    );
}

#[test]
fn test_samesuite_apu_div_write_trigger_volume_10() {
    let result = run_samesuite_test("same-suite/apu/div_write_trigger_volume_10.gb", Model::CgbE);
    assert_eq!(
        result,
        TestResult::Passed,
        "apu/div_write_trigger_volume_10 test failed"
    );
}

#[test]
fn test_samesuite_apu_div_write_trigger_volume() {
    let result = run_samesuite_test("same-suite/apu/div_write_trigger_volume.gb", Model::CgbE);
    assert_eq!(
        result,
        TestResult::Passed,
        "apu/div_write_trigger_volume test failed"
    );
}

#[test]
fn test_samesuite_apu_div_write_trigger() {
    let result = run_samesuite_test("same-suite/apu/div_write_trigger.gb", Model::CgbE);
    assert_eq!(
        result,
        TestResult::Passed,
        "apu/div_write_trigger test failed"
    );
}

// Channel 1

#[test]
fn test_samesuite_apu_channel_1_align_cpu() {
    let result = run_samesuite_test(
        "same-suite/apu/channel_1/channel_1_align_cpu.gb",
        Model::CgbE,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "apu/channel_1/channel_1_align_cpu test failed"
    );
}

#[test]
fn test_samesuite_apu_channel_1_align() {
    let result = run_samesuite_test("same-suite/apu/channel_1/channel_1_align.gb", Model::CgbE);
    assert_eq!(
        result,
        TestResult::Passed,
        "apu/channel_1/channel_1_align test failed"
    );
}

#[test]
fn test_samesuite_apu_channel_1_delay() {
    let result = run_samesuite_test("same-suite/apu/channel_1/channel_1_delay.gb", Model::CgbE);
    assert_eq!(
        result,
        TestResult::Passed,
        "apu/channel_1/channel_1_delay test failed"
    );
}

#[test]
fn test_samesuite_apu_channel_1_duty_delay() {
    let result = run_samesuite_test(
        "same-suite/apu/channel_1/channel_1_duty_delay.gb",
        Model::CgbE,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "apu/channel_1/channel_1_duty_delay test failed"
    );
}

#[test]
fn test_samesuite_apu_channel_1_duty() {
    let result = run_samesuite_test("same-suite/apu/channel_1/channel_1_duty.gb", Model::CgbE);
    assert_eq!(
        result,
        TestResult::Passed,
        "apu/channel_1/channel_1_duty test failed"
    );
}

#[test]
fn test_samesuite_apu_channel_1_freq_change() {
    let result = run_samesuite_test(
        "same-suite/apu/channel_1/channel_1_freq_change.gb",
        Model::CgbE,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "apu/channel_1/channel_1_freq_change test failed"
    );
}

#[test]
fn test_samesuite_apu_channel_1_freq_change_timing_cgb_de() {
    let result = run_samesuite_test(
        "same-suite/apu/channel_1/channel_1_freq_change_timing-cgbDE.gb",
        Model::CgbE,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "apu/channel_1/channel_1_freq_change_timing-cgbDE test failed"
    );
}

#[test]
fn test_samesuite_apu_channel_1_nrx2_glitch() {
    let result = run_samesuite_test(
        "same-suite/apu/channel_1/channel_1_nrx2_glitch.gb",
        Model::CgbE,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "apu/channel_1/channel_1_nrx2_glitch test failed"
    );
}

#[test]
fn test_samesuite_apu_channel_1_nrx2_speed_change() {
    let result = run_samesuite_test(
        "same-suite/apu/channel_1/channel_1_nrx2_speed_change.gb",
        Model::CgbE,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "apu/channel_1/channel_1_nrx2_speed_change test failed"
    );
}

#[test]
fn test_samesuite_apu_channel_1_restart() {
    let result = run_samesuite_test("same-suite/apu/channel_1/channel_1_restart.gb", Model::CgbE);
    assert_eq!(
        result,
        TestResult::Passed,
        "apu/channel_1/channel_1_restart test failed"
    );
}

#[test]
fn test_samesuite_apu_channel_1_restart_nrx2_glitch() {
    let result = run_samesuite_test(
        "same-suite/apu/channel_1/channel_1_restart_nrx2_glitch.gb",
        Model::CgbE,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "apu/channel_1/channel_1_restart_nrx2_glitch test failed"
    );
}

#[test]
fn test_samesuite_apu_channel_1_stop_div() {
    let result = run_samesuite_test(
        "same-suite/apu/channel_1/channel_1_stop_div.gb",
        Model::CgbE,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "apu/channel_1/channel_1_stop_div test failed"
    );
}

#[test]
fn test_samesuite_apu_channel_1_stop_restart() {
    let result = run_samesuite_test(
        "same-suite/apu/channel_1/channel_1_stop_restart.gb",
        Model::CgbE,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "apu/channel_1/channel_1_stop_restart test failed"
    );
}

#[test]
fn test_samesuite_apu_channel_1_sweep() {
    let result = run_samesuite_test("same-suite/apu/channel_1/channel_1_sweep.gb", Model::CgbE);
    assert_eq!(
        result,
        TestResult::Passed,
        "apu/channel_1/channel_1_sweep test failed"
    );
}

#[test]
fn test_samesuite_apu_channel_1_sweep_restart_2() {
    let result = run_samesuite_test(
        "same-suite/apu/channel_1/channel_1_sweep_restart_2.gb",
        Model::CgbE,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "apu/channel_1/channel_1_sweep_restart_2 test failed"
    );
}

#[test]
fn test_samesuite_apu_channel_1_sweep_restart() {
    let result = run_samesuite_test(
        "same-suite/apu/channel_1/channel_1_sweep_restart.gb",
        Model::CgbE,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "apu/channel_1/channel_1_sweep_restart test failed"
    );
}

#[test]
fn test_samesuite_apu_channel_1_volume_div() {
    let result = run_samesuite_test(
        "same-suite/apu/channel_1/channel_1_volume_div.gb",
        Model::CgbE,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "apu/channel_1/channel_1_volume_div test failed"
    );
}

#[test]
fn test_samesuite_apu_channel_1_volume() {
    let result = run_samesuite_test("same-suite/apu/channel_1/channel_1_volume.gb", Model::CgbE);
    assert_eq!(
        result,
        TestResult::Passed,
        "apu/channel_1/channel_1_volume test failed"
    );
}

// Channel 2

#[test]
fn test_samesuite_apu_channel_2_align_cpu() {
    let result = run_samesuite_test(
        "same-suite/apu/channel_2/channel_2_align_cpu.gb",
        Model::CgbE,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "apu/channel_2/channel_2_align_cpu test failed"
    );
}

#[test]
fn test_samesuite_apu_channel_2_align() {
    let result = run_samesuite_test("same-suite/apu/channel_2/channel_2_align.gb", Model::CgbE);
    assert_eq!(
        result,
        TestResult::Passed,
        "apu/channel_2/channel_2_align test failed"
    );
}

#[test]
fn test_samesuite_apu_channel_2_delay() {
    let result = run_samesuite_test("same-suite/apu/channel_2/channel_2_delay.gb", Model::CgbE);
    assert_eq!(
        result,
        TestResult::Passed,
        "apu/channel_2/channel_2_delay test failed"
    );
}

#[test]
fn test_samesuite_apu_channel_2_duty_delay() {
    let result = run_samesuite_test(
        "same-suite/apu/channel_2/channel_2_duty_delay.gb",
        Model::CgbE,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "apu/channel_2/channel_2_duty_delay test failed"
    );
}

#[test]
fn test_samesuite_apu_channel_2_duty() {
    let result = run_samesuite_test("same-suite/apu/channel_2/channel_2_duty.gb", Model::CgbE);
    assert_eq!(
        result,
        TestResult::Passed,
        "apu/channel_2/channel_2_duty test failed"
    );
}

#[test]
fn test_samesuite_apu_channel_2_freq_change() {
    let result = run_samesuite_test(
        "same-suite/apu/channel_2/channel_2_freq_change.gb",
        Model::CgbE,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "apu/channel_2/channel_2_freq_change test failed"
    );
}

#[test]
fn test_samesuite_apu_channel_2_nrx2_glitch() {
    let result = run_samesuite_test(
        "same-suite/apu/channel_2/channel_2_nrx2_glitch.gb",
        Model::CgbE,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "apu/channel_2/channel_2_nrx2_glitch test failed"
    );
}

#[test]
fn test_samesuite_apu_channel_2_nrx2_speed_change() {
    let result = run_samesuite_test(
        "same-suite/apu/channel_2/channel_2_nrx2_speed_change.gb",
        Model::CgbE,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "apu/channel_2/channel_2_nrx2_speed_change test failed"
    );
}

#[test]
fn test_samesuite_apu_channel_2_restart() {
    let result = run_samesuite_test("same-suite/apu/channel_2/channel_2_restart.gb", Model::CgbE);
    assert_eq!(
        result,
        TestResult::Passed,
        "apu/channel_2/channel_2_restart test failed"
    );
}

#[test]
fn test_samesuite_apu_channel_2_restart_nrx2_glitch() {
    let result = run_samesuite_test(
        "same-suite/apu/channel_2/channel_2_restart_nrx2_glitch.gb",
        Model::CgbE,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "apu/channel_2/channel_2_restart_nrx2_glitch test failed"
    );
}

#[test]
fn test_samesuite_apu_channel_2_stop_div() {
    let result = run_samesuite_test(
        "same-suite/apu/channel_2/channel_2_stop_div.gb",
        Model::CgbE,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "apu/channel_2/channel_2_stop_div test failed"
    );
}

#[test]
fn test_samesuite_apu_channel_2_stop_restart() {
    let result = run_samesuite_test(
        "same-suite/apu/channel_2/channel_2_stop_restart.gb",
        Model::CgbE,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "apu/channel_2/channel_2_stop_restart test failed"
    );
}

#[test]
fn test_samesuite_apu_channel_2_volume_div() {
    let result = run_samesuite_test(
        "same-suite/apu/channel_2/channel_2_volume_div.gb",
        Model::CgbE,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "apu/channel_2/channel_2_volume_div test failed"
    );
}

#[test]
fn test_samesuite_apu_channel_2_volume() {
    let result = run_samesuite_test("same-suite/apu/channel_2/channel_2_volume.gb", Model::CgbE);
    assert_eq!(
        result,
        TestResult::Passed,
        "apu/channel_2/channel_2_volume test failed"
    );
}

// Channel 3

#[test]
fn test_samesuite_apu_channel_3_and_glitch() {
    let result = run_samesuite_test(
        "same-suite/apu/channel_3/channel_3_and_glitch.gb",
        Model::CgbE,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "apu/channel_3/channel_3_and_glitch test failed"
    );
}

#[test]
fn test_samesuite_apu_channel_3_delay() {
    let result = run_samesuite_test("same-suite/apu/channel_3/channel_3_delay.gb", Model::CgbE);
    assert_eq!(
        result,
        TestResult::Passed,
        "apu/channel_3/channel_3_delay test failed"
    );
}

#[test]
fn test_samesuite_apu_channel_3_first_sample() {
    let result = run_samesuite_test(
        "same-suite/apu/channel_3/channel_3_first_sample.gb",
        Model::CgbE,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "apu/channel_3/channel_3_first_sample test failed"
    );
}

#[test]
fn test_samesuite_apu_channel_3_freq_change_delay() {
    let result = run_samesuite_test(
        "same-suite/apu/channel_3/channel_3_freq_change_delay.gb",
        Model::CgbE,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "apu/channel_3/channel_3_freq_change_delay test failed"
    );
}

#[test]
fn test_samesuite_apu_channel_3_restart_delay() {
    let result = run_samesuite_test(
        "same-suite/apu/channel_3/channel_3_restart_delay.gb",
        Model::CgbE,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "apu/channel_3/channel_3_restart_delay test failed"
    );
}

#[test]
fn test_samesuite_apu_channel_3_restart_during_delay() {
    let result = run_samesuite_test(
        "same-suite/apu/channel_3/channel_3_restart_during_delay.gb",
        Model::CgbE,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "apu/channel_3/channel_3_restart_during_delay test failed"
    );
}

#[test]
fn test_samesuite_apu_channel_3_restart_stop_delay() {
    let result = run_samesuite_test(
        "same-suite/apu/channel_3/channel_3_restart_stop_delay.gb",
        Model::CgbE,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "apu/channel_3/channel_3_restart_stop_delay test failed"
    );
}

#[test]
fn test_samesuite_apu_channel_3_shift_delay() {
    let result = run_samesuite_test(
        "same-suite/apu/channel_3/channel_3_shift_delay.gb",
        Model::CgbE,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "apu/channel_3/channel_3_shift_delay test failed"
    );
}

#[test]
fn test_samesuite_apu_channel_3_shift_skip_delay() {
    let result = run_samesuite_test(
        "same-suite/apu/channel_3/channel_3_shift_skip_delay.gb",
        Model::CgbE,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "apu/channel_3/channel_3_shift_skip_delay test failed"
    );
}

#[test]
fn test_samesuite_apu_channel_3_stop_delay() {
    let result = run_samesuite_test(
        "same-suite/apu/channel_3/channel_3_stop_delay.gb",
        Model::CgbE,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "apu/channel_3/channel_3_stop_delay test failed"
    );
}

#[test]
fn test_samesuite_apu_channel_3_stop_div() {
    let result = run_samesuite_test(
        "same-suite/apu/channel_3/channel_3_stop_div.gb",
        Model::CgbE,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "apu/channel_3/channel_3_stop_div test failed"
    );
}

#[test]
fn test_samesuite_apu_channel_3_wave_ram_dac_on_rw() {
    let result = run_samesuite_test(
        "same-suite/apu/channel_3/channel_3_wave_ram_dac_on_rw.gb",
        Model::CgbE,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "apu/channel_3/channel_3_wave_ram_dac_on_rw test failed"
    );
}

#[test]
fn test_samesuite_apu_channel_3_wave_ram_locked_write() {
    let result = run_samesuite_test(
        "same-suite/apu/channel_3/channel_3_wave_ram_locked_write.gb",
        Model::CgbE,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "apu/channel_3/channel_3_wave_ram_locked_write test failed"
    );
}

#[test]
fn test_samesuite_apu_channel_3_wave_ram_sync() {
    let result = run_samesuite_test(
        "same-suite/apu/channel_3/channel_3_wave_ram_sync.gb",
        Model::CgbE,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "apu/channel_3/channel_3_wave_ram_sync test failed"
    );
}

// Channel 4

#[test]
fn test_samesuite_apu_channel_4_align() {
    let result = run_samesuite_test("same-suite/apu/channel_4/channel_4_align.gb", Model::CgbE);
    assert_eq!(
        result,
        TestResult::Passed,
        "apu/channel_4/channel_4_align test failed"
    );
}

#[test]
fn test_samesuite_apu_channel_4_delay() {
    let result = run_samesuite_test("same-suite/apu/channel_4/channel_4_delay.gb", Model::CgbE);
    assert_eq!(
        result,
        TestResult::Passed,
        "apu/channel_4/channel_4_delay test failed"
    );
}

#[test]
fn test_samesuite_apu_channel_4_equivalent_frequencies() {
    let result = run_samesuite_test(
        "same-suite/apu/channel_4/channel_4_equivalent_frequencies.gb",
        Model::CgbE,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "apu/channel_4/channel_4_equivalent_frequencies test failed"
    );
}

#[test]
fn test_samesuite_apu_channel_4_freq_change() {
    let result = run_samesuite_test(
        "same-suite/apu/channel_4/channel_4_freq_change.gb",
        Model::CgbE,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "apu/channel_4/channel_4_freq_change test failed"
    );
}

#[test]
fn test_samesuite_apu_channel_4_frequency_alignment() {
    let result = run_samesuite_test(
        "same-suite/apu/channel_4/channel_4_frequency_alignment.gb",
        Model::CgbE,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "apu/channel_4/channel_4_frequency_alignment test failed"
    );
}

#[test]
fn test_samesuite_apu_channel_4_lfsr15() {
    let result = run_samesuite_test("same-suite/apu/channel_4/channel_4_lfsr15.gb", Model::CgbE);
    assert_eq!(
        result,
        TestResult::Passed,
        "apu/channel_4/channel_4_lfsr15 test failed"
    );
}

#[test]
fn test_samesuite_apu_channel_4_lfsr_7_15() {
    let result = run_samesuite_test(
        "same-suite/apu/channel_4/channel_4_lfsr_7_15.gb",
        Model::CgbE,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "apu/channel_4/channel_4_lfsr_7_15 test failed"
    );
}

#[test]
fn test_samesuite_apu_channel_4_lfsr() {
    let result = run_samesuite_test("same-suite/apu/channel_4/channel_4_lfsr.gb", Model::CgbE);
    assert_eq!(
        result,
        TestResult::Passed,
        "apu/channel_4/channel_4_lfsr test failed"
    );
}

#[test]
fn test_samesuite_apu_channel_4_lfsr_restart_fast() {
    let result = run_samesuite_test(
        "same-suite/apu/channel_4/channel_4_lfsr_restart_fast.gb",
        Model::CgbE,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "apu/channel_4/channel_4_lfsr_restart_fast test failed"
    );
}

#[test]
fn test_samesuite_apu_channel_4_lfsr_restart() {
    let result = run_samesuite_test(
        "same-suite/apu/channel_4/channel_4_lfsr_restart.gb",
        Model::CgbE,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "apu/channel_4/channel_4_lfsr_restart test failed"
    );
}

#[test]
fn test_samesuite_apu_channel_4_volume_div() {
    let result = run_samesuite_test(
        "same-suite/apu/channel_4/channel_4_volume_div.gb",
        Model::CgbE,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "apu/channel_4/channel_4_volume_div test failed"
    );
}

#[test]
fn test_samesuite_apu_channel_4_lfsr_15_7() {
    let result = run_samesuite_test(
        "same-suite/apu/channel_4/channel_4_lfsr_15_7.gb",
        Model::CgbE,
    );
    assert_eq!(
        result,
        TestResult::Passed,
        "apu/channel_4/channel_4_lfsr_15_7 test failed"
    );
}
