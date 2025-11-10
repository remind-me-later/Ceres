# Implementation Tasks

## 1. Test Infrastructure Enhancement

- [x] 1.1 Add button press scheduling capability to `TestConfig` struct
- [x] 1.2 Add method to `TestRunner` to process scheduled button presses during frame execution
- [x] 1.3 Add `RTC3TEST_BASIC` timeout constant to `test_runner.rs::timeouts` module (~1050 frames for 17.5s total)
- [x] 1.4 Add `RTC3TEST_RANGE` timeout constant to `test_runner.rs::timeouts` module (~750 frames for 12.5s total)
- [x] 1.5 Create `rtc_tests.rs` test file in `ceres-test-runner/tests/` directory

## 2. Button Press Simulation

- [x] 2.1 Design button press scheduling API (frame number + button + press/release)
- [x] 2.2 Implement button press queue processing in `TestRunner::run_frame`
- [x] 2.3 Handle button press timing (press for multiple frames if needed)

## 3. Test Implementation

- [x] 3.1 Implement `test_rtc3test_basic_cgb` test function
  - [x] 3.1.1 Schedule button press: A button at frame 240 (~4 seconds after CGB intro completes)
  - [x] 3.1.2 Use screenshot comparison with `rtc3test-basic-tests-cgb.png`
  - [x] 3.1.3 Set timeout to `RTC3TEST_BASIC` constant
- [x] 3.2 Implement `test_rtc3test_range_cgb` test function
  - [x] 3.2.1 Schedule button presses: Down at frame 240, A at frame 270 (0.5s delay)
  - [x] 3.2.2 Use screenshot comparison with `rtc3test-range-tests-cgb.png`
  - [x] 3.2.3 Set timeout to `RTC3TEST_RANGE` constant

## 4. Validation

- [x] 4.1 Run new tests locally to verify they pass
- [x] 4.2 Verify tests complete within timeout
- [x] 4.3 Confirm screenshot comparison works correctly with color correction disabled
- [x] 4.4 Confirm CI pipeline includes new tests and passes

## Dependencies

- Test ROM and reference screenshots already exist in `test-roms/rtc3test/`
- No new external dependencies required
- Uses existing `TestRunner` infrastructure with button press extension
- Requires `Button` enum from `ceres_core` (already available)

## Notes

- Sub-second writes test is intentionally excluded as it currently fails
- Tests run only on CGB model (sufficient for RTC validation per documentation)
- Button timing accounts for ~4 second CGB boot intro animation
- Button timing may need adjustment based on test behavior during implementation
- Timeouts include CGB intro duration + test duration + safety margin
