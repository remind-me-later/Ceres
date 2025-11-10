# integration-tests Specification

## Purpose

TBD - created by archiving change add-rtc3test-integration. Update Purpose after archive.

## Requirements

### Requirement: Button Input Simulation

The test runner SHALL support simulating button presses at specific frames during test execution.

#### Scenario: Schedule button press

- **WHEN** a test configures a button event to press button A at frame 240
- **THEN** the test runner presses button A on the emulated Game Boy when frame 240 is reached
- **AND** the button press triggers the appropriate interrupt in the emulated hardware

#### Scenario: Schedule button release

- **WHEN** a test configures a button event to release button A at frame 250
- **THEN** the test runner releases button A on the emulated Game Boy when frame 250 is reached
- **AND** subsequent button state reads reflect the button is not pressed

#### Scenario: Multiple button events

- **WHEN** a test schedules multiple button events (Down at frame 240, A at frame 270)
- **THEN** the test runner executes each button event at its scheduled frame in order
- **AND** each button event affects the emulated Game Boy independently

### Requirement: MBC3 RTC Basic Tests Validation

The test suite SHALL validate MBC3 RTC basic functionality using the rtc3test ROM's "basic tests" subtest on CGB
hardware.

#### Scenario: RTC basic tests pass on CGB

- **WHEN** the rtc3test.gb ROM is executed on CGB model with button A pressed at frame 240 (after CGB intro)
- **THEN** the basic tests subtest runs for approximately 13 seconds after button press
- **AND** the emulator's screen output matches the reference screenshot `rtc3test-basic-tests-cgb.png` pixel-for-pixel
- **AND** the test completes within 1050 frames total (CGB intro + test duration + margin)

#### Scenario: RTC basic tests validate core functionality

- **WHEN** the basic tests subtest completes successfully
- **THEN** the following RTC behaviors are validated: RTC enable/disable, tick timing, register writes, seconds
  increment, rollovers, overflow flag handling, and overflow stickiness
- **AND** each validation is reflected in the reference screenshot

### Requirement: MBC3 RTC Range Tests Validation

The test suite SHALL validate MBC3 RTC register value ranges using the rtc3test ROM's "range tests" subtest on CGB
hardware.

#### Scenario: RTC range tests pass on CGB

- **WHEN** the rtc3test.gb ROM is executed on CGB model with Down button pressed at frame 240 and A button pressed at
  frame 270 (after CGB intro)
- **THEN** the range tests subtest runs for approximately 8 seconds after button press
- **AND** the emulator's screen output matches the reference screenshot `rtc3test-range-tests-cgb.png` pixel-for-pixel
- **AND** the test completes within 750 frames total (CGB intro + test duration + margin)

#### Scenario: RTC range tests validate register behavior

- **WHEN** the range tests subtest completes successfully
- **THEN** the following RTC register behaviors are validated: all bits clear, all valid bits set, valid bits mask,
  invalid value tick handling, invalid rollovers, high minutes rollover, and high hours rollover
- **AND** each validation is reflected in the reference screenshot

### Requirement: RTC Test Timeout Configuration

The test runner SHALL define appropriate timeouts for rtc3test ROM subtests based on documented test durations.

#### Scenario: Basic tests timeout configured

- **WHEN** the RTC3TEST_BASIC timeout constant is defined as 1050 frames
- **THEN** the basic tests have sufficient time to complete (4s CGB intro + 13s test + 0.5s margin at 59.73 fps)
- **AND** the timeout accounts for the CGB boot animation before the test ROM runs

#### Scenario: Range tests timeout configured

- **WHEN** the RTC3TEST_RANGE timeout constant is defined as 750 frames
- **THEN** the range tests have sufficient time to complete (4s CGB intro + 8s test + 0.5s margin at 59.73 fps)
- **AND** the timeout accounts for the CGB boot animation before the test ROM runs

### Requirement: Selective RTC Test Execution

The test suite SHALL only include rtc3test subtests that currently pass, excluding subtests with known failures.

#### Scenario: Sub-second writes tests excluded

- **WHEN** the rtc3test integration tests are run
- **THEN** the "sub-second writes" subtest is not executed
- **AND** only the "basic tests" and "range tests" subtests are included
- **AND** the exclusion is documented with the reason (test currently fails due to incomplete RTC implementation)

#### Scenario: Test exclusion is temporary

- **WHEN** the RTC implementation is improved to support sub-second write behavior
- **THEN** the "sub-second writes" test can be added in a future change
- **AND** the test infrastructure already supports the necessary button press simulation (Down, Down, A sequence)

### Requirement: CGB Acid2 PPU Test Validation

The test suite SHALL validate CGB Acid2 PPU rendering accuracy using breakpoint detection for test completion while
maintaining timeout-based safety.

#### Scenario: CGB Acid2 completes on breakpoint

- **WHEN** the cgb-acid2.gbc ROM is executed on CGB model
- **THEN** the test runner monitors the `ld b, b` breakpoint flag during execution
- **AND** when the breakpoint flag is set (indicating test completion), the test runner immediately captures the
  screenshot
- **AND** the screenshot is compared pixel-for-pixel against the reference `cgb-acid2.png`
- **AND** the test passes if the screenshots match exactly

#### Scenario: CGB Acid2 timeout as safety net

- **WHEN** the cgb-acid2.gbc ROM is executed but the breakpoint is never hit
- **THEN** the test runner continues execution until the timeout of 300 frames is reached
- **AND** the test fails with a timeout result
- **AND** this protects against infinitely looping test ROMs that never signal completion

#### Scenario: CGB Acid2 validates PPU accuracy

- **WHEN** the CGB Acid2 test completes successfully via breakpoint
- **THEN** the test validates accurate PPU rendering of sprites, backgrounds, windows, and color palettes
- **AND** the validation reflects correct CGB-specific rendering behavior
- **AND** color correction is disabled for accurate pixel comparison

### Requirement: DMG Acid2 PPU Test Validation

The test suite SHALL validate DMG Acid2 PPU rendering accuracy in both DMG and CGB modes using breakpoint detection for
test completion while maintaining timeout-based safety.

#### Scenario: DMG Acid2 DMG mode completes on breakpoint

- **WHEN** the dmg-acid2.gb ROM is executed on DMG model
- **THEN** the test runner monitors the `ld b, b` breakpoint flag during execution
- **AND** when the breakpoint flag is set, the test runner immediately captures the screenshot
- **AND** the screenshot is compared against the DMG-specific reference screenshot
- **AND** the test passes if the screenshots match exactly

#### Scenario: DMG Acid2 CGB mode completes on breakpoint

- **WHEN** the dmg-acid2.gb ROM is executed on CGB model
- **THEN** the test runner monitors the `ld b, b` breakpoint flag during execution
- **AND** when the breakpoint flag is set, the test runner immediately captures the screenshot
- **AND** the screenshot is compared against the CGB-specific reference screenshot
- **AND** the test passes if the screenshots match exactly

#### Scenario: DMG Acid2 timeout as safety net

- **WHEN** the dmg-acid2.gb ROM is executed but the breakpoint is never hit
- **THEN** the test runner continues execution until the timeout of 480 frames is reached
- **AND** the test fails with a timeout result
- **AND** this protects against infinitely looping test ROMs in both DMG and CGB modes

#### Scenario: DMG Acid2 validates PPU accuracy

- **WHEN** the DMG Acid2 test completes successfully via breakpoint
- **THEN** the test validates accurate PPU rendering of sprites, backgrounds, and grayscale palettes
- **AND** the validation works correctly in both DMG native mode and CGB compatibility mode
- **AND** color correction is disabled for accurate pixel comparison

### Requirement: Breakpoint-Based Test Completion

The test runner SHALL support detecting test completion via the `ld b, b` debug breakpoint instruction in addition to
timeout-based completion.

#### Scenario: Check breakpoint flag during test execution

- **WHEN** the test runner is executing a test ROM frame by frame
- **THEN** after each frame, the test runner checks if the `ld_b_b_breakpoint` flag is set
- **AND** if the flag is set, the test runner proceeds to the completion check (e.g., screenshot comparison)
- **AND** the check is performed before evaluating timeout conditions

#### Scenario: Breakpoint takes precedence over timeout

- **WHEN** a test ROM executes the `ld b, b` breakpoint instruction
- **THEN** the test runner completes the test immediately upon breakpoint detection
- **AND** the timeout is not waited for
- **AND** this allows tests to complete faster when the ROM signals completion

#### Scenario: Timeout still enforced when no breakpoint

- **WHEN** a test ROM never executes the `ld b, b` instruction
- **THEN** the test runner continues until the configured timeout is reached
- **AND** the timeout prevents infinite loops in broken or incomplete test ROMs
- **AND** the dual mechanism (breakpoint OR timeout) ensures robustness

#### Scenario: Breakpoint flag is reset after checking

- **WHEN** the test runner checks the `ld_b_b_breakpoint` flag
- **THEN** the flag is automatically reset by the `check_and_reset_ld_b_b_breakpoint()` method
- **AND** subsequent frames do not incorrectly detect stale breakpoint signals
- **AND** this ensures clean state management across test frames
