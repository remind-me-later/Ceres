# Ceres Test Runner

Integration test runner for the Ceres Game Boy emulator using actual Game Boy test ROMs.

## Overview

The test suite validates emulator accuracy using pixel-perfect screenshot comparison against reference images. Tests
cover:

- **CPU Instructions**: Complete SM83 instruction set validation
- **Timing**: Instruction timing, memory timing, and interrupt timing
- **PPU Rendering**: CGB and DMG display accuracy tests

### Test Completion Detection

The test runner uses multiple mechanisms to detect when tests complete:

- **Breakpoint detection**: Test ROMs like cgb-acid2 and dmg-acid2 use the `ld b, b` instruction (opcode 0x40) as a
  debug breakpoint to signal completion. When detected, tests complete immediately after the screenshot matches.
- **Screenshot comparison**: Tests pass when the emulator output matches reference images pixel-for-pixel.
- **Timeout safety**: All tests have timeout values to prevent infinite loops in broken or incomplete test ROMs.

This approach allows Acid2 tests to complete in ~0.4 seconds instead of waiting for the full timeout (~20+ seconds).

## Setup

Test ROMs are **automatically downloaded** when you build or test this crate. No manual setup required!

The build script downloads the test ROM collection (172MB) from the
[c-sp/gameboy-test-roms](https://github.com/c-sp/gameboy-test-roms) repository on first build and caches it in the
`test-roms/` directory.

## Running Tests

### Run All Tests

```bash
cargo test --package ceres-test-runner
```

This will run all integration tests (~2-3 seconds total):

1. Download test ROMs automatically (if not already present)
2. Run CPU instruction tests
3. Run timing validation tests
4. Run PPU accuracy tests
5. Run unit tests for the test infrastructure

### Run Specific Test

```bash
# Run a specific test
cargo test --package ceres-test-runner test_blargg_cpu_instrs

# Run all dmg-acid2 tests
cargo test --package ceres-test-runner test_dmg_acid2

# Run ignored tests (known failures)
cargo test --package ceres-test-runner -- --ignored
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

## Test Suite

The tests are organized into logical modules:

### Blargg Test Suite (`tests/blargg_tests.rs`)

CPU instructions, timing, and hardware bug tests from Blargg's test suite:

- **`test_blargg_cpu_instrs`** - All 11 CPU instruction categories in one comprehensive test
- **`test_blargg_instr_timing`** - Validates instruction cycle timing
- **`test_blargg_mem_timing`** - Tests memory access timing
- **`test_blargg_mem_timing_2`** - Advanced memory timing scenarios
- **`test_blargg_interrupt_time`** - Interrupt handling timing
- **`test_blargg_halt_bug`** - HALT instruction edge cases

### PPU Accuracy Tests (`tests/ppu_tests.rs`)

Visual accuracy tests for the Pixel Processing Unit. These tests use breakpoint detection for fast completion:

- **`test_cgb_acid2`** - CGB PPU rendering accuracy
- **`test_dmg_acid2_cgb`** - DMG Acid2 test running in CGB mode
- **`test_dmg_acid2_dmg`** - DMG Acid2 test running in DMG mode

The Acid2 tests complete in ~0.4 seconds total thanks to breakpoint detection, compared to ~20 seconds if relying on
timeouts alone.

### Serial Output Tests (`tests/serial_test.rs`)

Serial communication functionality tests

## Known Issues

- **DMG PPU rendering**: The `test_dmg_acid2_dmg` test is currently failing due to differences in DMG mode PPU
  rendering. The CGB mode version passes, indicating the issue is specific to DMG palette handling or rendering
  behavior.

## Trace Collection for Debugging

The test runner supports automatic trace export when tests fail, capturing the last N executed instructions for
debugging:

### Enabling Trace Export

```rust
use ceres_test_runner::{
    load_test_rom,
    test_runner::{TestConfig, TestResult, TestRunner},
};

#[test]
fn test_with_trace() {
    let rom = load_test_rom("path/to/rom.gb").expect("Failed to load ROM");

    let config = TestConfig {
        enable_trace: true,                   // Enable trace collection
        export_trace_on_failure: true,        // Export on failure
        trace_buffer_size: 2000,              // Keep last 2000 instructions
        timeout_frames: 300,
        ..TestConfig::default()
    };

    let mut runner = TestRunner::new(rom, config).expect("Failed to create runner");
    let result = runner.run();

    // If test fails, trace is automatically exported to target/traces/<timestamp>_trace.json
    assert_eq!(result, TestResult::Passed);
}
```

### Trace Output

Traces are exported as JSON to `target/traces/<timestamp>_trace.json` containing:

- **Metadata**: timestamp, entry count, buffer capacity
- **Entries**: Array of executed instructions with:
  - Program counter (PC)
  - Disassembled instruction
  - Register state (A, F, B, C, D, E, H, L, SP)
  - Cycle count

### Analyzing Traces

Use the provided Python analysis script or jq for trace analysis:

### Command-line Analysis with jq

```bash
# Show last 10 instructions
jq '.entries[-10:]' target/traces/1234567890_trace.json

# Find all JP instructions
jq '.entries[] | select(.instruction | contains("JP"))' target/traces/1234567890_trace.json

# Count instruction frequencies
jq -r '.entries[].instruction' target/traces/1234567890_trace.json | sort | uniq -c | sort -rn
```

### Python Analysis Script

A comprehensive Python analysis tool is provided in `analyze_trace.py`:

```bash
# Show last 20 instructions with register state
python ceres-test-runner/analyze_trace.py target/traces/1234567890_trace.json --last 20

# Generate instruction frequency histogram
python ceres-test-runner/analyze_trace.py target/traces/1234567890_trace.json --histogram

# Find all JP instructions
python ceres-test-runner/analyze_trace.py target/traces/1234567890_trace.json --inst JP

# Show instructions in a specific PC range
python ceres-test-runner/analyze_trace.py target/traces/1234567890_trace.json --range 0x0150 0x0160

# Detect potential infinite loops
python ceres-test-runner/analyze_trace.py target/traces/1234567890_trace.json --loops

# Combine multiple analyses
python ceres-test-runner/analyze_trace.py target/traces/1234567890_trace.json \
  --last 10 --histogram --loops
```

The script provides:

- **Metadata Display**: Timestamp, entry count, buffer capacity
- **Last N Instructions**: Show recent execution history with registers
- **Instruction Search**: Find specific mnemonics (JP, CALL, LD, etc.)
- **PC Range Filter**: View instructions in a specific address range
- **Frequency Histogram**: Most executed instructions
- **Loop Detection**: Find repeated PC sequences indicating infinite loops

See the Python examples in the repository for advanced trace analysis workflows.

## Writing New Tests

To add a new test ROM:

1. Ensure the ROM is available in the `test-roms/` collection
2. Use the `load_test_rom()` helper to load it
3. Create a `TestRunner` with appropriate configuration
4. Run the test and assert on the result

Example:

```rust
use ceres_test_runner::{
    load_test_rom,
    test_runner::{TestConfig, TestResult, TestRunner, timeouts},
};

#[test]
fn test_my_rom() {
    let rom = load_test_rom("path/to/rom.gb").expect("Failed to load ROM");

    let config = TestConfig {
        timeout_frames: 300,
        expected_screenshot: ceres_test_runner::expected_screenshot_path(
            "path/to/rom.gb",
            ceres_core::Model::Cgb,
        ),
        ..TestConfig::default()
    };

    let mut runner = TestRunner::new(rom, config).expect("Failed to create runner");
    let result = runner.run();

    assert_eq!(result, TestResult::Passed, "Test failed");
}
```

### Breakpoint Detection

If your test ROM uses the `ld b, b` instruction (opcode 0x40) as a completion signal:

1. Configure an `expected_screenshot` in the `TestConfig`
2. The test runner will automatically detect the breakpoint and complete immediately when the screenshot matches
3. Set an appropriate timeout as a safety net (the test will use whichever comes first: breakpoint match or timeout)

The breakpoint detection allows tests to complete as soon as they signal completion, rather than waiting for arbitrary
timeouts.
