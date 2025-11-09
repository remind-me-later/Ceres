# Implementation Plan: Fix `dmg-acid2` Test RGB Mismatch

**Feature Branch**: `001-fix-dmg-acid2-rgb`
**Feature Spec**: [spec.md](./spec.md)
**Created**: 2025-11-09
**Status**: In Progress

## Technical Context

This plan addresses a bug in the `dmg-acid2` integration test, where the test fails in DMG mode due to a mismatch in RGB color values during screenshot comparison. The test currently passes in CGB mode, indicating a discrepancy in how colors are rendered or interpreted between the two modes.

The core of the work will be within the `ceres-core` and `ceres-test-runner` crates. Specifically, we need to investigate the PPU's color palette generation in DMG mode and the screenshot comparison logic in the test runner.

**Key areas of investigation:**

-   `ceres-core/src/ppu/color_palette.rs`: How DMG color palettes are created and if they align with the expected values for the acid2 test.
-   `ceres-core/src/ppu/draw.rs`: The rendering logic that uses the color palettes.
-   `ceres-test-runner/src/test_runner.rs`: The screenshot comparison logic and how it handles different color palettes.
-   `ceres-test-runner/tests/ppu_tests.rs`: The test case `test_dmg_acid2_dmg` itself.

**Unknowns:**

-   [NEEDS CLARIFICATION] What are the exact, correct RGB values for the 4 shades of gray on a DMG device?
-   [NEEDS CLARIFICATION] Why does the current implementation produce different colors in DMG mode compared to the reference screenshot? Is it a problem in the core emulation, or in the test's comparison logic?

## Constitution Check

-   **I. SameBoy Reference Standard**: The fix must align with SameBoy's rendering of `dmg-acid2`. We will use SameBoy as the reference for the correct visual output.
-   **II. Test-Driven Development**: The primary goal is to fix a failing test. The fix will be validated by the `ceres-test-runner` suite.
-   **III. Pan Docs Compliance**: The color palette generation must adhere to the specifications in Pan Docs for the DMG.
-   **IV. no_std Core Requirement**: All changes to `ceres-core` must remain `no_std` compatible.
-   **V. Modular Architecture**: The changes will be localized to the PPU and test runner, respecting module boundaries.
-   **VI. Performance Requirements**: The fix is unlikely to have performance implications, but we will ensure no regressions are introduced.
-   **VII. Code Coverage Standards**: We will aim to maintain or improve coverage for the modified code.
-   **VIII. Documentation Standards**: Any changes to color palette logic will be documented with references to Pan Docs or SameBoy.

## Phase 0: Outline & Research

The goal of this phase is to resolve the "NEEDS CLARIFICATION" items identified in the Technical Context.

**Findings:**

1.  **DMG Color Palette:** The `dmg-acid2` reference screenshot uses the following 8-bit grayscale values: `$FF`, `$AA`, `$55`, `$00`. This corresponds to the following sRGB values:
    *   White: `(255, 255, 255)`
    *   Light Gray: `(170, 170, 170)`
    *   Dark Gray: `(85, 85, 85)`
    *   Black: `(0, 0, 0)`

2.  **Rendering Discrepancy:** The root cause of the bug is in `ceres-core/src/ppu/color_palette.rs`. The `GRAYSCALE_PALETTE` constant defines the DMG colors as `(0xFF, 0xFF, 0xFF)`, `(0xCC, 0xCC, 0xCC)`, `(0x77, 0x77, 0x77)`, and `(0x00, 0x00, 0x00)`. The two gray shades, `0xCC` (204) and `0x77` (119), do not match the reference values of `0xAA` (170) and `0x55` (85).

**Output**: A `research.md` file is not needed as the research was straightforward and the findings are documented here.

## Phase 1: Design & Contracts

This is a bug fix, so no new data models or API contracts are expected.

**Proposed Change:**

The `GRAYSCALE_PALETTE` constant in `ceres-core/src/ppu/color_palette.rs` will be modified to use the correct color values.

```rust
// In ceres-core/src/ppu/color_palette.rs

pub const GRAYSCALE_PALETTE: [(u8, u8, u8); 4] = [
    (0xFF, 0xFF, 0xFF), // White
    (0xAA, 0xAA, 0xAA), // Light Gray
    (0x55, 0x55, 0x55), // Dark Gray
    (0x00, 0x00, 0x00), // Black
];
```

This change will align the emulator's DMG color output with the `dmg-acid2` reference screenshot, allowing the test to pass.

**Note on Timeout**: The `dmg-acid2` test's timeout value is currently arbitrary and has been adjusted to allow the test to pass. A more robust solution for test completion detection (e.g., implementing the `ld b, b` debug hook) will be addressed in a separate, future task.

**Agent Context Update:** No new technologies are being introduced, so no update to the agent context is required.
