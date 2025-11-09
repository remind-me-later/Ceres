# Feature Specification: Fix `dmg-acid2` Test RGB Mismatch

**Feature Branch**: `001-fix-dmg-acid2-rgb`
**Created**: 2025-11-09
**Status**: Draft
**Input**: User description: "There is a bug in one of the integration tests recently added dmg-acid2, I think the problem is that we are not using the exact same rgb values when comparing screenshots. I want to fix that by making the output mat the screenshot for now."

## User Scenarios & Testing _(mandatory)_

### User Story 1 - Fix `dmg-acid2` Test in DMG Mode (Priority: P1)

As a developer, I want the `dmg-acid2` integration test to pass reliably in DMG mode so that I can have confidence in the correctness of the PPU implementation for both DMG and CGB environments.

**Why this priority**: A failing integration test for a specific mode (DMG) creates noise, can hide real regressions in that mode, and undermines trust in the CI/CD pipeline's ability to catch platform-specific bugs.

**Independent Test**: This can be tested by running the `ceres-test-runner` package. The `test_dmg_acid2_dmg` test should pass, while the `test_dmg_acid2_cgb` test should remain passing.

**Acceptance Scenarios**:

1. **Given** the `dmg-acid2` test ROM running in DMG mode, **When** the test runner executes the test, **Then** the generated screenshot must match the reference screenshot pixel-for-pixel using the correct RGB color palette for DMG.
2. **Given** a correct PPU implementation, **When** a developer runs `cargo test --package ceres-test-runner`, **Then** the `test_dmg_acid2_dmg` test case passes without any pixel mismatches.
3. **Given** the fix for DMG mode, **When** a developer runs `cargo test --package ceres-test-runner`, **Then** the `test_dmg_acid2_cgb` test case continues to pass.

### Edge Cases

- N/A for this bug fix. The scope is limited to correcting the color values for an existing test.

## Requirements _(mandatory)_

### Functional Requirements

- **FR-001**: The screenshot comparison logic for the `dmg-acid2` test running in DMG mode MUST use the exact RGB color values that are expected in the reference PNG image.
- **FR-002**: The emulator's rendering output for the `dmg-acid2` test in DMG mode MUST be corrected to produce the expected image, ensuring the test passes.

## Success Criteria _(mandatory)_

### Measurable Outcomes

- **SC-001**: The `test_dmg_acid2_dmg` integration test in the `ceres-test-runner` crate passes 100% of the time on the main branch.
- **SC-002**: The `test_dmg_acid2_cgb` integration test continues to pass.
- **SC-003**: The fix does not cause any other existing integration tests (e.g., Blargg's tests, `cgb-acid2`) to fail.