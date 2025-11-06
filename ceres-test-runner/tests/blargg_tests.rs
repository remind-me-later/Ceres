//! Integration tests using Blargg test ROMs
//!
//! These tests validate the CPU instruction implementation against
//! Blargg's comprehensive test suite.

use ceres_test_runner::{
    load_test_rom,
    test_runner::{TestConfig, TestResult, TestRunner, timeouts},
};

/// Helper to run a test ROM with a specific timeout
fn run_test_rom(path: &str) -> TestResult {
    let rom = match load_test_rom(path) {
        Ok(rom) => rom,
        Err(e) => return TestResult::Failed(format!("Failed to load test ROM: {e}")),
    };

    let config = TestConfig {
        timeout_frames: timeouts::CPU_INSTRS,
        ..TestConfig::default()
    };

    let mut runner = match TestRunner::new(rom, config) {
        Ok(runner) => runner,
        Err(e) => return TestResult::Failed(format!("Failed to create test runner: {e}")),
    };

    runner.run()
}

/// Macro to generate CPU instruction tests
macro_rules! cpu_test {
    ($name:ident, $path:expr, $desc:expr) => {
        #[test]
        fn $name() {
            let result = run_test_rom($path);
            assert_eq!(result, TestResult::Passed, $desc);
        }
    };
}

// ============================================================================
// CPU Instructions Tests
// ============================================================================

/// Run the complete CPU instructions test suite.
/// This runs all 11 CPU tests in one ROM.
#[test]
fn test_blargg_cpu_instrs_all() {
    let result = run_test_rom("blargg/cpu_instrs/cpu_instrs.gb");
    assert_eq!(
        result,
        TestResult::Passed,
        "CPU instructions test suite failed"
    );
}

// Individual CPU instruction tests - only run if the full suite fails.
// These help identify which specific instruction category is failing.

cpu_test!(
    test_blargg_cpu_instrs_01_special,
    "blargg/cpu_instrs/individual/01-special.gb",
    "CPU special instructions test failed"
);

cpu_test!(
    test_blargg_cpu_instrs_02_interrupts,
    "blargg/cpu_instrs/individual/02-interrupts.gb",
    "CPU interrupts test failed"
);

cpu_test!(
    test_blargg_cpu_instrs_03_op_sp_hl,
    "blargg/cpu_instrs/individual/03-op sp,hl.gb",
    "CPU OP SP,HL test failed"
);

cpu_test!(
    test_blargg_cpu_instrs_04_op_r_imm,
    "blargg/cpu_instrs/individual/04-op r,imm.gb",
    "CPU OP R,IMM test failed"
);

cpu_test!(
    test_blargg_cpu_instrs_05_op_rp,
    "blargg/cpu_instrs/individual/05-op rp.gb",
    "CPU OP RP test failed"
);

cpu_test!(
    test_blargg_cpu_instrs_06_ld_r_r,
    "blargg/cpu_instrs/individual/06-ld r,r.gb",
    "CPU LD R,R test failed"
);

cpu_test!(
    test_blargg_cpu_instrs_07_jr_jp_call_ret_rst,
    "blargg/cpu_instrs/individual/07-jr,jp,call,ret,rst.gb",
    "CPU JR,JP,CALL,RET,RST test failed"
);

cpu_test!(
    test_blargg_cpu_instrs_08_misc_instrs,
    "blargg/cpu_instrs/individual/08-misc instrs.gb",
    "CPU misc instructions test failed"
);

cpu_test!(
    test_blargg_cpu_instrs_09_op_r_r,
    "blargg/cpu_instrs/individual/09-op r,r.gb",
    "CPU OP R,R test failed"
);

cpu_test!(
    test_blargg_cpu_instrs_10_bit_ops,
    "blargg/cpu_instrs/individual/10-bit ops.gb",
    "CPU bit ops test failed"
);

cpu_test!(
    test_blargg_cpu_instrs_11_op_a_hl,
    "blargg/cpu_instrs/individual/11-op a,(hl).gb",
    "CPU OP A,(HL) test failed"
);

// ============================================================================
// Instruction Timing Tests
// ============================================================================

/// Run the instruction timing test.
/// This validates that instructions take the correct number of cycles.
#[test]
fn test_blargg_instr_timing() {
    let rom = match load_test_rom("blargg/instr_timing/instr_timing.gb") {
        Ok(rom) => rom,
        Err(e) => panic!("Failed to load test ROM: {e}"),
    };

    let config = TestConfig {
        timeout_frames: timeouts::INSTR_TIMING,
        ..TestConfig::default()
    };

    let mut runner = match TestRunner::new(rom, config) {
        Ok(runner) => runner,
        Err(e) => panic!("Failed to create test runner: {e}"),
    };

    let result = runner.run();
    assert_eq!(result, TestResult::Passed, "Instruction timing test failed");
}

// ============================================================================
// Memory Timing Tests
// ============================================================================

/// Run the complete memory timing test suite.
#[test]
fn test_blargg_mem_timing_all() {
    let rom = match load_test_rom("blargg/mem_timing/mem_timing.gb") {
        Ok(rom) => rom,
        Err(e) => panic!("Failed to load test ROM: {e}"),
    };

    let config = TestConfig {
        timeout_frames: timeouts::MEM_TIMING,
        ..TestConfig::default()
    };

    let mut runner = match TestRunner::new(rom, config) {
        Ok(runner) => runner,
        Err(e) => panic!("Failed to create test runner: {e}"),
    };

    let result = runner.run();
    assert_eq!(
        result,
        TestResult::Passed,
        "Memory timing test suite failed"
    );
}

/// Helper to run `mem_timing` tests with the correct timeout
fn run_mem_timing_test(path: &str) -> TestResult {
    let rom = match load_test_rom(path) {
        Ok(rom) => rom,
        Err(e) => return TestResult::Failed(format!("Failed to load test ROM: {e}")),
    };

    let config = TestConfig {
        timeout_frames: timeouts::MEM_TIMING,
        ..TestConfig::default()
    };

    let mut runner = match TestRunner::new(rom, config) {
        Ok(runner) => runner,
        Err(e) => return TestResult::Failed(format!("Failed to create test runner: {e}")),
    };

    runner.run()
}

/// Macro to generate memory timing tests
macro_rules! mem_timing_test {
    ($name:ident, $path:expr, $desc:expr) => {
        #[test]
        fn $name() {
            let result = run_mem_timing_test($path);
            assert_eq!(result, TestResult::Passed, $desc);
        }
    };
}

mem_timing_test!(
    test_blargg_mem_timing_01_read_timing,
    "blargg/mem_timing/individual/01-read_timing.gb",
    "Memory read timing test failed"
);

mem_timing_test!(
    test_blargg_mem_timing_02_write_timing,
    "blargg/mem_timing/individual/02-write_timing.gb",
    "Memory write timing test failed"
);

mem_timing_test!(
    test_blargg_mem_timing_03_modify_timing,
    "blargg/mem_timing/individual/03-modify_timing.gb",
    "Memory modify timing test failed"
);

// ============================================================================
// Memory Timing 2 Tests
// ============================================================================
// Note: These tests currently timeout - they expose emulation bugs
// that need to be fixed. Run with --ignored to test them.

/// Run the complete memory timing 2 test suite.
#[test]
#[ignore = "Currently times out - emulation bug to fix"]
fn test_blargg_mem_timing_2_all() {
    let rom = match load_test_rom("blargg/mem_timing-2/mem_timing.gb") {
        Ok(rom) => rom,
        Err(e) => panic!("Failed to load test ROM: {e}"),
    };

    let config = TestConfig {
        timeout_frames: timeouts::MEM_TIMING_2,
        ..TestConfig::default()
    };

    let mut runner = match TestRunner::new(rom, config) {
        Ok(runner) => runner,
        Err(e) => panic!("Failed to create test runner: {e}"),
    };

    let result = runner.run();
    assert_eq!(
        result,
        TestResult::Passed,
        "Memory timing 2 test suite failed"
    );
}

/// Helper to run `mem_timing-2` tests with the correct timeout
fn run_mem_timing_2_test(path: &str) -> TestResult {
    let rom = match load_test_rom(path) {
        Ok(rom) => rom,
        Err(e) => return TestResult::Failed(format!("Failed to load test ROM: {e}")),
    };

    let config = TestConfig {
        timeout_frames: timeouts::MEM_TIMING_2,
        ..TestConfig::default()
    };

    let mut runner = match TestRunner::new(rom, config) {
        Ok(runner) => runner,
        Err(e) => return TestResult::Failed(format!("Failed to create test runner: {e}")),
    };

    runner.run()
}

// Mark individual mem_timing-2 tests as ignored
// These will help debug which specific test is failing
#[ignore = "Currently times out - emulation bug to fix"]
#[test]
fn test_blargg_mem_timing_2_01_read_timing() {
    let result = run_mem_timing_2_test("blargg/mem_timing-2/rom_singles/01-read_timing.gb");
    assert_eq!(
        result,
        TestResult::Passed,
        "Memory timing 2 read test failed"
    );
}

#[ignore = "Currently times out - emulation bug to fix"]
#[test]
fn test_blargg_mem_timing_2_02_write_timing() {
    let result = run_mem_timing_2_test("blargg/mem_timing-2/rom_singles/02-write_timing.gb");
    assert_eq!(
        result,
        TestResult::Passed,
        "Memory timing 2 write test failed"
    );
}

#[ignore = "Currently times out - emulation bug to fix"]
#[test]
fn test_blargg_mem_timing_2_03_modify_timing() {
    let result = run_mem_timing_2_test("blargg/mem_timing-2/rom_singles/03-modify_timing.gb");
    assert_eq!(
        result,
        TestResult::Passed,
        "Memory timing 2 modify test failed"
    );
}

// ============================================================================
// Interrupt Timing Tests
// ============================================================================
// Note: This test currently times out - it exposes emulation bugs
// that need to be fixed. Run with --ignored to test it.

/// Run the interrupt timing test.
/// This validates that interrupts occur at the correct time.
#[test]
#[ignore = "Currently times out - emulation bug to fix"]
fn test_blargg_interrupt_time() {
    let rom = match load_test_rom("blargg/interrupt_time/interrupt_time.gb") {
        Ok(rom) => rom,
        Err(e) => panic!("Failed to load test ROM: {e}"),
    };

    let config = TestConfig {
        timeout_frames: timeouts::INTERRUPT_TIME,
        ..TestConfig::default()
    };

    let mut runner = match TestRunner::new(rom, config) {
        Ok(runner) => runner,
        Err(e) => panic!("Failed to create test runner: {e}"),
    };

    let result = runner.run();
    assert_eq!(result, TestResult::Passed, "Interrupt timing test failed");
}
