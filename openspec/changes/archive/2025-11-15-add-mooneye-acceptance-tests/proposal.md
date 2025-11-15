# Change: Add Mooneye Test Suite Acceptance Tests

## Why

The Mooneye Test Suite is a comprehensive set of hardware-validated Game Boy test ROMs that test various low-level
behaviors including CPU instructions, timing, interrupts, PPU rendering, timer operations, serial communication, and
OAM DMA. The acceptance test suite contains 75 test ROMs that have been verified on real hardware.

Currently, Ceres only runs Blargg tests, gbmicrotest, and Acid2 tests. Adding Mooneye acceptance tests will
significantly expand test coverage and validate emulation accuracy against hardware-verified behaviors, particularly
for timing-sensitive operations and edge cases.

The approach is to add only tests that currently pass, using `#[ignore]` for failing tests with tracking comments.
This allows incremental progress - fixing ignored tests will be addressed in future changes.

## What Changes

- Add CPU register reading API to expose B, C, D, E, H, L registers needed for Mooneye's Fibonacci validation
- Add Mooneye test validation logic that checks for Fibonacci numbers (3/5/8/13/21/34) in registers
- Create test infrastructure for running Mooneye acceptance tests with proper timeout (120 seconds = ~7160 frames
  at 59.73 Hz)
- Add individual test functions for all 75 acceptance tests (including subdirectories: bits/, instr/, interrupts/,
  oam_dma/, ppu/, serial/, timer/)
- Mark failing tests with `#[ignore]` and tracking comments
- Tests use existing `ld b, b` breakpoint detection mechanism for completion
- Reuse existing `TestRunner` infrastructure with Mooneye-specific result validation

## Impact

- Affected specs: `integration-tests`
- Affected code:
  - `ceres-core/src/lib.rs` - Add public methods to read CPU registers
  - `ceres-core/src/sm83.rs` - May need to expose register getters
  - `ceres-test-runner/tests/mooneye_tests.rs` - New test file with 75 test functions
  - `ceres-test-runner/src/test_runner.rs` - Add Mooneye validation logic and timeout constant
