# Research: cgb-acid2 Test Implementation

## Decision: Test Timeout Value

**Chosen**: 600 frames (~10 seconds)

**Rationale**:

- cgb-acid2 is NOT a timing torture test - it uses simple line-based rendering
- Test completes by executing opcode 0x40 (LD B, B), but we rely on screenshot comparison
- No timing-critical operations or T-cycle accuracy required
- Writes registers during PPU mode 2 (OAM scan) on specific scanlines using LY=LYC interrupts
- Similar tests (halt_bug) use 330 frames, but cgb-acid2 is more complex visually
- Conservative estimate allows for slower CI environments
- Expected completion: <300 frames, 600 provides 2x safety margin

**Alternatives Considered**:

1. 300 frames (~5s) - May be too tight for slower CI runners
2. 1000 frames (~16s) - Unnecessarily long for a non-timing test
3. Match halt_bug (330 frames) - cgb-acid2 likely slightly longer due to complexity

## Decision: Test File Location

**Chosen**: Add test to existing `blargg_tests.rs`

**Rationale**:

- cgb-acid2 is a visual/PPU test, not strictly a Blargg test
- However, it uses the same TestRunner infrastructure
- Alternative would be creating `acid_tests.rs` for cgb-acid2 and future acid tests
- For a single test, adding to blargg_tests.rs is simpler
- Can refactor later if we add dmg-acid2 or cgb-acid-hell

**Alternatives Considered**:

1. Create new `acid_tests.rs` - Better organization but overkill for 1 test
2. Create `ppu_tests.rs` - Too generic, would need to move other PPU tests
3. Keep in `blargg_tests.rs` - Simple, works with existing infrastructure ✓

## Decision: Screenshot Comparison Approach

**Chosen**: Use existing TestConfig::expected_screenshot

**Rationale**:

- TestRunner already supports screenshot comparison via `expected_screenshot` field
- Color correction already disabled in TestRunner for accurate comparison
- RGB conversion formula (X << 3) | (X >> 2) matches Ceres PPU implementation
- Reference image cgb-acid2.png already exists in test-roms directory

**Alternatives Considered**:

1. Add exit condition detection for opcode 0x40 - Unnecessary, screenshot comparison sufficient
2. Use serial output - cgb-acid2 doesn't output to serial
3. Custom comparison logic - Redundant, existing infrastructure works

## Decision: Test Name

**Chosen**: `test_cgb_acid2`

**Rationale**:

- Follows Rust naming convention (snake_case)
- Clear and descriptive
- Matches test ROM filename
- Consistent with existing test names (test*blargg*\*)

**Alternatives Considered**:

1. `test_acid2_cgb` - Less clear (acid2 could refer to dmg-acid2)
2. `test_ppu_acid2` - Doesn't match ROM name
3. `test_cgb_acid2` - Clear, follows convention ✓

## Best Practices: Integration Test Structure

**Pattern**: Follow existing Blargg test pattern

```rust
#[test]
fn test_cgb_acid2() {
    let result = run_test_rom("cgb-acid2/cgb-acid2.gbc", timeouts::CGB_ACID2);
    assert_eq!(result, TestResult::Passed, "CGB Acid2 PPU test failed");
}
```

**Key elements**:

1. Use `run_test_rom` helper (already handles TestRunner setup)
2. Pass relative path from test-roms/ directory
3. Use timeout constant from timeouts module
4. Provide descriptive assertion message
5. TestRunner automatically uses screenshot comparison when expected_screenshot is set

## Implementation Notes

### Files to Modify

1. **ceres-test-runner/src/test_runner.rs**:
   - Add `pub const CGB_ACID2: u32 = 600;` to timeouts module

2. **ceres-test-runner/tests/blargg_tests.rs**:
   - Add test function `test_cgb_acid2()`
   - Import timeouts::CGB_ACID2

### Expected Behavior

- **Pass**: Screenshot matches cgb-acid2.png exactly
- **Fail**: Screenshot differs (PPU bug) - shows TestResult::Failed message
- **Timeout**: Test exceeds 600 frames - indicates emulation bug or timeout too short

### Testing the Test

After implementation:

1. Run `cargo test --package ceres-test-runner test_cgb_acid2`
2. Verify test passes (if PPU implementation is correct)
3. If test fails, compare actual vs expected screenshot for debugging
4. Verify test completes well under 600 frames (check frames_run)

## Risk Mitigation

**Risk**: Test may fail due to existing PPU bugs **Mitigation**: This is expected and desired - the test helps identify
PPU issues to fix

**Risk**: Timeout too short for slow CI **Mitigation**: 600 frames (10s) provides ample margin; can adjust if needed

**Risk**: Screenshot comparison false positives **Mitigation**: Color correction disabled, formula matches; unlikely to
be an issue
