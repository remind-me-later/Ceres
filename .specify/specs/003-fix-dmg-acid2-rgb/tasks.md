# Tasks for: Fix `dmg-acid2` Test RGB Mismatch

**Feature**: [spec.md](./spec.md)
**Plan**: [plan.md](./plan.md)

## Phase 1: Setup

- [X] T001 Create project structure per implementation plan

## Phase 2: Foundational Tasks

*(No foundational tasks for this feature)*

## Phase 3: User Story 1 - Fix `dmg-acid2` Test in DMG Mode

**Goal**: The `dmg-acid2` integration test passes reliably in DMG mode.
**Independent Test**: Run `cargo test --package ceres-test-runner -- --nocapture` and confirm that `test_dmg_acid2_dmg` passes.

### Implementation Tasks

- [X] T002 [US1] Modify the `GRAYSCALE_PALETTE` constant in `ceres-core/src/ppu/color_palette.rs` to use the correct RGB values.
- [X] T003 [US1] Remove the `#[ignore]` attribute from the `test_dmg_acid2_dmg` test in `ceres-test-runner/tests/ppu_tests.rs`.

## Phase 4: Polish & Cross-Cutting Concerns

- [X] T004 Run all integration tests to ensure no regressions were introduced. (Note: A performance regression was found and documented in the plan.)

## Dependencies

- User Story 1 (Phase 3) is the only user story and has no dependencies.

## Parallel Execution

- The tasks in this plan are sequential and should be executed in order.

## Implementation Strategy

1.  **MVP First**: The MVP is to get the `test_dmg_acid2_dmg` test to pass. This is achieved by completing Phase 3.
2.  **Incremental Delivery**: The entire feature is a single, small bug fix and will be delivered in one increment.
