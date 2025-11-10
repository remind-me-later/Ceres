## 1. Implementation

- [x] 1.1 Add `ld_b_b_breakpoint` boolean field to the `Gb` struct in `ceres-core/src/lib.rs`
- [x] 1.2 Initialize the flag to `false` in the `Gb::new` method
- [x] 1.3 Reset the flag to `false` in the `Gb::soft_reset` method
- [x] 1.4 Add public method `check_and_reset_ld_b_b_breakpoint(&mut self) -> bool` to the `Gb` struct
- [x] 1.5 Modify `Sm83::ld_b_b` in `ceres-core/src/sm83.rs` to set the flag (requires access via `Gb` context)
- [x] 1.6 Update the CPU execution logic to pass necessary context for setting the flag

## 2. Documentation

- [x] 2.1 Add inline documentation to the new method explaining its purpose for test ROM debugging
- [x] 2.2 Document that this is specifically for test ROMs that use `ld b, b` as a breakpoint
- [x] 2.3 Add a comment in `sm83.rs` explaining why `ld_b_b` sets this flag

## 3. Validation

- [x] 3.1 Verify the flag is set when `ld b, b` executes during normal operation
- [x] 3.2 Verify the check-and-reset pattern works correctly
- [x] 3.3 Verify the flag survives frame boundaries until checked
- [x] 3.4 Confirm no breaking changes to existing API
- [x] 3.5 Run existing test suite to ensure no regressions
