## 1. Implementation

- [x] 1.1 Add breakpoint checking to `TestRunner::check_completion()` in `ceres-test-runner/src/test_runner.rs`
- [x] 1.2 Check the `ld_b_b_breakpoint` flag using `gb.check_and_reset_ld_b_b_breakpoint()` before screenshot comparison
- [x] 1.3 If breakpoint flag is set, proceed with screenshot comparison immediately
- [x] 1.4 Ensure timeout values remain in place as a safety net for tests that never hit the breakpoint

## 2. Documentation

- [x] 2.1 Add a comment in `TestRunner::check_completion()` explaining the dual completion criteria (breakpoint OR
      timeout)
- [x] 2.2 Document that the `ld b, b` breakpoint is the primary completion signal for Acid2 tests
- [x] 2.3 Note that timeouts serve as a safety mechanism for infinitely looping tests

## 3. Validation

- [x] 3.1 Run `test_cgb_acid2` and verify it completes on breakpoint detection
- [x] 3.2 Run `test_dmg_acid2_dmg` and verify it completes on breakpoint detection
- [x] 3.3 Run `test_dmg_acid2_cgb` and verify it completes on breakpoint detection
- [x] 3.4 Verify tests complete faster than the timeout when the breakpoint is hit
- [x] 3.5 Verify the timeout still triggers if the breakpoint is never executed (simulate by commenting out breakpoint
      check)
- [x] 3.6 Run the full test suite to ensure no regressions
