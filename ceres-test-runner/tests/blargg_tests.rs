//! Integration tests using Blargg test ROMs
//!
//! These tests validate the CPU instruction implementation against
//! Blargg's comprehensive test suite.

use ceres_test_runner::{
    load_test_rom,
    test_runner::{timeouts, TestConfig, TestResult, TestRunner},
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
