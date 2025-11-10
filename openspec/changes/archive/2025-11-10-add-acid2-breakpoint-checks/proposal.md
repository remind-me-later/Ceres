# Change: Add Breakpoint Checks to Acid2 Integration Tests

## Why

The cgb-acid2 and dmg-acid2 test ROMs use the `ld b, b` instruction (opcode 0x40) as a debug breakpoint to signal test
completion, as documented in their respective howto files. Currently, the integration tests for these ROMs rely solely
on arbitrary timeout values to determine when to capture the screenshot. This approach is inefficient and may be
unreliable if emulation speed varies.

With the recently added `ld_b_b_breakpoint` flag in `ceres-core` (from `add-ld-b-b-breakpoint-flag`), we can now detect
when these test ROMs complete, allowing for faster and more reliable test execution while keeping the timeouts as a
safety net for infinitely looping tests.

## What Changes

- Add breakpoint detection to the `TestRunner` to check the `ld_b_b_breakpoint` flag during test execution
- Update `test_cgb_acid2`, `test_dmg_acid2_dmg`, and `test_dmg_acid2_cgb` tests to use breakpoint detection for
  completion
- Keep the existing timeout values as a safety mechanism to catch infinitely looping tests that never hit the breakpoint
- Document that tests complete on breakpoint detection OR timeout, whichever comes first

## Impact

- Affected specs: `integration-tests` (modified capability)
- Affected code:
  - `ceres-test-runner/src/test_runner.rs` (add breakpoint checking in completion detection)
  - `ceres-test-runner/tests/ppu_tests.rs` (no code changes needed, behavior changes via `TestRunner`)
- Test impact: Tests complete faster when breakpoint is hit; timeouts still protect against infinite loops
- API: No changes to public APIs; internal test runner behavior only
