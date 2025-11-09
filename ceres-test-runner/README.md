# Ceres Test Runner

Integration test runner for the Ceres Game Boy emulator using actual Game Boy test ROMs.

## Overview

The test suite is currently focused on **CPU instruction validation** to ensure core emulation accuracy before expanding
to other subsystems.

## Setup

Test ROMs are **automatically downloaded** when you build or test this crate. No manual setup required!

The build script downloads the test ROM collection from the
[c-sp/gameboy-test-roms](https://github.com/c-sp/gameboy-test-roms) repository on first build and caches it in the
`test-roms/` directory.

## Running Tests

### Run All Tests

```bash
cargo test --package ceres-test-runner
```

This will:

1. Download test ROMs automatically (if not already present)
2. Run all CPU instruction tests (~3-4 seconds)
3. Run unit tests for the test infrastructure

### Run Specific Test

```bash
cargo test --package ceres-test-runner test_blargg_cpu_instrs_all
```

### CI/CD Usage

Test ROMs are automatically downloaded in CI environments. For optimal performance, cache the `test-roms/` directory:

```yaml
# Example GitHub Actions workflow
- name: Cache test ROMs
  uses: actions/cache@v4
  with:
    path: test-roms
    key: test-roms-v7.0

- name: Run tests
  run: cargo test --package ceres-test-runner
```

## Test Structure

### Full CPU Test Suite

The primary test runs all 11 CPU instruction categories in one ROM:

```bash
cargo test --package ceres-test-runner test_blargg_cpu_instrs_all
```

### Debug Individual CPU Instructions

If the full suite fails, run individual tests to pinpoint the problem:

```bash
cargo test --package ceres-test-runner test_blargg_cpu_instrs_01_special
cargo test --package ceres-test-runner test_blargg_cpu_instrs_02_interrupts
# ... and so on for tests 03-11
```

Individual test categories:

1. `01_special` - Special instructions
2. `02_interrupts` - Interrupt handling
3. `03_op_sp_hl` - SP and HL operations
4. `04_op_r_imm` - Register-immediate operations
5. `05_op_rp` - Register pair operations
6. `06_ld_r_r` - Register-to-register loads
7. `07_jr_jp_call_ret_rst` - Control flow instructions
8. `08_misc_instrs` - Miscellaneous instructions
9. `09_op_r_r` - Register-to-register operations
10. `10_bit_ops` - Bit operations
11. `11_op_a_hl` - Accumulator and (HL) operations

## Future Expansion

Once CPU tests are stable, additional test suites will be added:

- **PPU/Graphics**: Rendering accuracy tests
- **APU/Sound**: Audio processing tests
- **Timing**: Instruction and memory timing tests
- **Hardware Bugs**: OAM bug, halt bug, etc.

## Writing New Tests

To add a new test ROM:

1. Ensure the ROM is available in the `test-roms/` collection
2. Use the `load_test_rom()` helper to load it
3. Create a `TestRunner` with appropriate configuration
4. Run the test and assert on the result

Example:

```rust
#[test]
fn test_my_rom() {
    let result = run_test_rom("path/to/rom.gb");
    assert_eq!(result, TestResult::Passed, "Test failed");
}
```
