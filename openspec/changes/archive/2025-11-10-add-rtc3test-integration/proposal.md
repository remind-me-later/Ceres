# Change: Add rtc3test RTC Integration Tests

## Why

The project currently lacks integration tests for MBC3 Real-Time Clock (RTC) functionality. The rtc3test ROM is already
present in the test-roms repository and provides validation of RTC behavior with reference screenshots. However, this
test ROM requires user interaction (button presses) to select and run specific subtests, making it more complex to
automate than typical screenshot-comparison tests.

Only the "basic tests" and "range tests" subtests currently pass in the emulator. The "sub-second writes" subtest does
not pass and should not be included until the underlying RTC implementation is fixed.

## What Changes

- Extend `TestRunner` to support simulating button presses at specific frames
- Add integration tests for rtc3test ROM covering passing subtests (basic and range)
- Tests run on CGB model only (sufficient for RTC validation)
- Uses screenshot comparison against reference images after button sequence execution
- Adds appropriate timeout constants for both test variants

## Impact

- Affected specs: `integration-tests` (existing capability - adding new requirements)
- Affected code:
  - `ceres-test-runner/src/test_runner.rs` (add button press simulation, new timeout constants)
  - `ceres-test-runner/tests/` (new test file `rtc_tests.rs` or addition to existing test file)
- Test coverage: Adds validation for MBC3 RTC basic functionality and value range handling
- CI/CD: Tests will run automatically on every push
- Test duration: ~13s for basic tests, ~8s for range tests (emulated time)
