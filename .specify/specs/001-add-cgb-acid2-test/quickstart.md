# Quickstart: Adding cgb-acid2 Test

## Prerequisites

- Rust toolchain installed
- Ceres repository cloned
- Test ROMs downloaded (run `cargo build` once to trigger download)

## Implementation Steps

### 1. Add Timeout Constant

Edit `ceres-test-runner/src/test_runner.rs`:

```rust
pub mod timeouts {
    pub const CPU_INSTRS: u32 = 2091;
    pub const INSTR_TIMING: u32 = 250;
    pub const MEM_TIMING: u32 = 300;
    pub const MEM_TIMING_2: u32 = 360;
    pub const INTERRUPT_TIME: u32 = 240;
    pub const HALT_BUG: u32 = 330;
    pub const CGB_ACID2: u32 = 600;  // ADD THIS LINE
}
```

### 2. Add Test Function

Edit `ceres-test-runner/tests/blargg_tests.rs`:

```rust
#[test]
fn test_cgb_acid2() {
    let result = run_test_rom("cgb-acid2/cgb-acid2.gbc", timeouts::CGB_ACID2);
    assert_eq!(result, TestResult::Passed, "CGB Acid2 PPU test failed");
}
```

### 3. Verify Test Files Exist

Check that these files are present:

- `test-roms/cgb-acid2/cgb-acid2.gbc`
- `test-roms/cgb-acid2/cgb-acid2.png`

If missing, the test ROM download may need to be re-triggered.

### 4. Run the Test

```bash
# Run only the cgb-acid2 test
cargo test --package ceres-test-runner test_cgb_acid2

# Run all tests
cargo test --package ceres-test-runner
```

### 5. Interpret Results

**Expected outcomes**:

✅ **PASSED**: PPU implementation is correct

```text
test test_cgb_acid2 ... ok
```

❌ **FAILED**: PPU has bugs (expected until PPU is fully accurate)

```text
test test_cgb_acid2 ... FAILED
assertion failed: CGB Acid2 PPU test failed
```

⏱️ **TIMEOUT**: Test didn't complete in 600 frames (emulation issue)

```text
test test_cgb_acid2 ... FAILED
TestResult::Timeout
```

## Verification Checklist

- [ ] Timeout constant added to `test_runner.rs`
- [ ] Test function added to `blargg_tests.rs`
- [ ] Test compiles without errors
- [ ] Test runs (pass or fail is acceptable at this stage)
- [ ] Test completes in <10 seconds
- [ ] No changes to ceres-core required

## Debugging Failed Tests

If the test fails, compare screenshots to identify PPU issues:

1. Run test with verbose output:

   ```bash
   cargo test --package ceres-test-runner test_cgb_acid2 -- --nocapture
   ```

2. Check the failure message for details

3. Compare actual vs expected:
   - Expected: `test-roms/cgb-acid2/cgb-acid2.png`
   - Actual: Captured during test run (visible in failure output)

4. Refer to `test-roms/cgb-acid2/README.md` for failure examples
   - Each PPU feature has a specific visual failure pattern

## Common Issues

**Issue**: Test ROM not found **Solution**: Run `cargo build` to trigger test ROM download (172MB)

**Issue**: Screenshot mismatch **Solution**: This is expected if PPU has bugs. Use failure pattern to identify issue.

**Issue**: Timeout **Solution**: Check emulation loop for infinite loops or timing bugs

**Issue**: Color correction interference **Solution**: TestRunner already disables color correction, no action needed

## Integration with CI

The test will automatically run in GitHub Actions:

```yaml
- name: Run tests
  run: cargo test --package ceres-core --package ceres-test-runner
```

No CI configuration changes needed.

## Next Steps

After implementation:

1. Commit changes to feature branch
2. Run full test suite
3. Push to trigger CI
4. If test fails, create follow-up spec to fix PPU bugs identified by cgb-acid2

## Files Modified

| File                                      | Change                      |
| ----------------------------------------- | --------------------------- |
| `ceres-test-runner/src/test_runner.rs`    | Add CGB_ACID2 constant      |
| `ceres-test-runner/tests/blargg_tests.rs` | Add test_cgb_acid2 function |

## Estimated Time

- Implementation: 5 minutes
- Testing: 2 minutes
- Total: ~10 minutes
