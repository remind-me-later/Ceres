# Change: Add MBC3 Tester Integration Test

## Why

The project currently lacks integration tests for MBC3 cartridge bank switching functionality. The mbc3-tester ROM is
already present in the test-roms repository and provides visual verification of MBC3 ROM bank switching accuracy with
reference screenshots for both DMG and CGB modes.

## What Changes

- Add integration test for mbc3-tester ROM that validates MBC3 bank switching
- Test runs on both DMG and CGB models with screenshot comparison
- Uses existing test infrastructure (TestRunner, screenshot comparison)
- Adds appropriate timeout constant for the test

## Impact

- Affected specs: `integration-tests` (new capability)
- Affected code:
  - `ceres-test-runner/tests/` (new test file or addition to existing)
  - `ceres-test-runner/src/test_runner.rs` (new timeout constant)
- Test coverage: Adds validation for MBC3 ROM bank switching behavior
- CI/CD: Test will run automatically on every push
