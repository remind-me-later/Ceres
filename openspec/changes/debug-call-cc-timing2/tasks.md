## 1. Collect Execution Trace

- [ ] 1.1 Run `test_mooneye_call_cc_timing2` with trace collection enabled to capture full execution
- [ ] 1.2 Run `test_mooneye_call_cc_timing` (passing test) with trace collection for comparison
- [ ] 1.3 Collect traces at points where tests execute `CALL cc, nn` instructions

## 2. Analyze Traces

- [ ] 2.1 Identify all `CALL cc, nn` instructions executed in both test traces
- [ ] 2.2 Compare cycle counts and register states at each conditional call
- [ ] 2.3 Look for discrepancies in timing when condition is true vs false
- [ ] 2.4 Check if issue is related to stack operations, PC updates, or M-cycle timing

## 3. Compare with Reference Implementation

- [ ] 3.1 Review Pan Docs documentation for `CALL cc, nn` timing specifications
- [ ] 3.2 Check SameBoy implementation of `call_cc_a16` for timing differences
- [ ] 3.3 Compare our implementation in `ceres-core/src/sm83.rs:636-645` with reference
- [ ] 3.4 Look for edge cases in condition evaluation timing

## 4. Identify Root Cause

- [ ] 4.1 Document the specific timing issue (e.g., missing M-cycle, incorrect branch timing)
- [ ] 4.2 Determine if issue is in `call_cc_a16`, `do_call`, or `satisfies_branch_condition`
- [ ] 4.3 Verify if issue affects other conditional instructions (e.g., `ret_cc`, `jp_cc`)
- [ ] 4.4 Create minimal reproduction case if possible

## 5. Implement Fix

- [ ] 5.1 Apply timing correction to `call_cc_a16` implementation
- [ ] 5.2 Add inline comments explaining the timing behavior
- [ ] 5.3 Ensure fix maintains compatibility with passing `call_cc_timing` test

## 6. Validation

- [ ] 6.1 Run `test_mooneye_call_cc_timing2` and verify it passes
- [ ] 6.2 Run `test_mooneye_call_cc_timing` and verify it still passes
- [ ] 6.3 Run all other conditional instruction tests (e.g., `call_timing`, `jp_cc_timing`, `ret_cc_timing`)
- [ ] 6.4 Run full Mooneye test suite to ensure no regressions
- [ ] 6.5 Remove `#[ignore]` from `test_mooneye_call_cc_timing2` in `mooneye_tests.rs`

## 7. Documentation

- [ ] 7.1 Update `AGENTS.md` to increment passing Mooneye test count if test now passes
- [ ] 7.2 Add comment in code explaining the specific timing requirement
- [ ] 7.3 Document findings in change archive for future reference
