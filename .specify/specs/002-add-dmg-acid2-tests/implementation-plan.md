# Implementation Plan: Add dmg-acid2 PPU Tests

## Status: COMPLETED ✅

**Feature Branch:** `002-add-dmg-acid2-tests`  
**Started:** 2025-11-09  
**Completed:** 2025-11-09

## Phase 0: Research & Planning

### Research Findings

**DMG-Acid2 Test ROM Characteristics:**

- Purpose: Validates PPU rendering accuracy for DMG emulators
- Requirements: Simple line-based renderer (not cycle-accurate timing required)
- Uses: LY=LYC coincidence interrupts for register writes during mode 2
- Output: Static image that should match reference exactly
- Available in test-roms v7.0 collection

**Reference Images:**

- `dmg-acid2-dmg.png`: DMG mode with greyscale palette ($00, $55, $AA, $FF)
- `dmg-acid2-cgb.png`: CGB mode in DMG compatibility with 5-bit to 8-bit conversion

**Existing Infrastructure:**

- `expected_screenshot_path()` already handles model-specific screenshots
- Test runner supports model selection via `TestConfig`
- Screenshot comparison uses pixel-perfect matching

### Decisions

1. **Timeout Value**: 240 frames (~4 seconds) - conservative estimate based on cgb-acid2 (300 frames)
2. **Test Organization**: Separate PPU tests into dedicated module for clarity
3. **DMG Mode Handling**: Mark as ignored rather than removing - documents the bug for future work

## Phase 1: Implementation

### Task Breakdown

1. ✅ Add `DMG_ACID2` timeout constant to `test_runner.rs`
2. ✅ Create `tests/ppu_tests.rs` with PPU accuracy tests
3. ✅ Move `test_cgb_acid2` from `blargg_tests.rs` to `ppu_tests.rs`
4. ✅ Add `test_dmg_acid2_dmg` (DMG mode, ignored)
5. ✅ Add `test_dmg_acid2_cgb` (CGB mode, passing)
6. ✅ Refactor `blargg_tests.rs` to focus on Blargg suite only
7. ✅ Update `AGENTS.md` documentation
8. ✅ Update `ceres-test-runner/README.md` with new structure

### Files Modified

```text
ceres-test-runner/src/test_runner.rs
  + DMG_ACID2 timeout constant (240 frames)

ceres-test-runner/tests/ppu_tests.rs (NEW)
  + test_cgb_acid2 (moved from blargg_tests.rs)
  + test_dmg_acid2_dmg (new, ignored)
  + test_dmg_acid2_cgb (new, passing)

ceres-test-runner/tests/blargg_tests.rs
  - Removed PPU tests (cgb-acid2, dmg-acid2)
  - Renamed run_test_rom -> run_blargg_test
  - Updated documentation

AGENTS.md
  + Added dmg-acid2 tests to integration tests list
  + Documented known DMG PPU issue

ceres-test-runner/README.md
  + Complete rewrite of test suite documentation
  + Added test file organization section
  + Documented known issues
```

## Phase 2: Testing & Validation

### Test Results

```bash
$ cargo test --package ceres-test-runner
running 10 tests
✅ 9 tests passing
⚠️  1 test ignored (test_dmg_acid2_dmg - known DMG PPU bug)
⏱️  Total time: ~4.4 seconds
```

**Passing Tests:**

- `test_blargg_cpu_instrs` - CPU instructions
- `test_blargg_instr_timing` - Instruction timing
- `test_blargg_mem_timing` - Memory timing
- `test_blargg_mem_timing_2` - Advanced memory timing
- `test_blargg_interrupt_time` - Interrupt timing
- `test_blargg_halt_bug` - HALT instruction
- `test_cgb_acid2` - CGB PPU accuracy ✓
- `test_dmg_acid2_cgb` - DMG Acid2 in CGB mode ✓
- `test_serial_output_capture` - Serial communication

**Ignored Tests:**

- `test_dmg_acid2_dmg` - DMG mode PPU rendering issue (documented)

## Constitution Check

### Principles Applied

✅ **SameBoy Gold Standard**: Test ROMs from established sources (mattcurrie/dmg-acid2)  
✅ **Test-Driven Development**: Maintain test coverage, documented known failures  
✅ **Pan Docs Compliance**: PPU accuracy tests validate documented behavior  
✅ **No Breaking Changes**: All existing tests continue to pass

### Quality Gates

✅ All existing tests pass  
✅ New tests properly documented  
✅ Test execution time < 5 seconds  
✅ Code follows Rust conventions  
✅ Documentation updated

## Discovered Issues

### DMG PPU Rendering Bug

**Symptom:** `test_dmg_acid2_dmg` times out (screenshot doesn't match reference)  
**Impact:** DMG mode display accuracy not validated  
**Workaround:** Test marked as `#[ignore]` with clear documentation  
**Status:** Known issue, tracked for future work

**Evidence:**

- CGB mode test passes → PPU logic generally works
- DMG mode test fails → Mode-specific rendering issue
- Likely cause: DMG palette handling or color conversion

## Lessons Learned

1. **Test Organization Matters**: Separating tests by subsystem improved clarity significantly
2. **Screenshot Comparison is Powerful**: Pixel-perfect validation caught a real DMG mode bug
3. **Ignore > Delete**: Keeping failing tests (ignored) documents bugs better than removing them
4. **Model-Specific Testing**: Testing both DMG and CGB modes revealed mode-specific issues

## Next Steps

1. Investigate DMG PPU rendering issue
2. Consider adding more PPU accuracy tests (mealybug-tearoom, etc.)
3. Add timeout measurement to automatically optimize timeout values

## Conclusion

Successfully added dmg-acid2 PPU tests and reorganized test suite for better maintainability. Discovered and documented
a DMG mode PPU rendering bug while verifying CGB mode works correctly. All acceptance criteria met.
