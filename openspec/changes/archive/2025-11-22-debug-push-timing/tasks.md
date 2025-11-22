# Implementation Tasks

## 1. Core Implementation

- [x] 1.1 Add internal delay M-cycle at start of `push()` function in `sm83.rs`
- [x] 1.2 Remove the final `tick_m_cycle()` at end of `push()` function
- [x] 1.3 Restructure memory write timing to ensure writes occur at M=2 and M=3
- [x] 1.4 Consider adding `write_mem_only()` helper or similar to write without advancing time

## 2. Testing

- [x] 2.1 Run `test_mooneye_push_timing` and verify it passes
- [x] 2.2 Run `test_mooneye_pop_timing` to ensure no regression
- [x] 2.3 Run `test_mooneye_call_timing` to ensure no regression
- [x] 2.4 Run `test_mooneye_call_cc_timing` to ensure no regression
- [x] 2.5 Run `test_mooneye_ret_timing` to ensure no regression
- [x] 2.6 Run all Mooneye acceptance tests to verify no regressions
- [x] 2.7 Run all Blargg tests to verify no regressions

## 3. Documentation

- [x] 3.1 Update TODO comment in `push()` about SP write modification during push
- [x] 3.2 Add comment explaining PUSH timing pattern (M=1: internal delay, M=2: high byte, M=3: low byte)
- [x] 3.3 Update test documentation in `mooneye_tests.rs` to remove `#[ignore]` from `test_mooneye_push_timing`

## 4. Code Review

- [x] 4.1 Verify that all callers of `push()` (PUSH rr, CALL, RST, interrupts) work correctly with new timing
- [x] 4.2 Audit uses of `write_cpu()` to ensure timing semantics are consistent across the codebase
- [x] 4.3 Check if any other instructions need similar timing adjustments (e.g., RST)
