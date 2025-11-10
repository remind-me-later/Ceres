# integration-tests Specification Deltas

## ADDED Requirements

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
