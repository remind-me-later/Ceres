# Feature Specification: Add dmg-acid2 PPU Tests

## Overview

Add integration tests for the dmg-acid2 PPU accuracy test ROM, validating rendering behavior in both DMG and CGB modes.
Reorganize test suite into logical modules for better maintainability.

## Problem Statement

The test suite lacks PPU rendering accuracy validation, making it difficult to verify correct display output behavior. All
tests were grouped in `blargg_tests.rs` despite testing different subsystems (CPU, PPU, timing).

## Requirements

### Functional Requirements

1. **DMG-Acid2 Test Coverage**
   - MUST test dmg-acid2.gb in DMG mode
   - MUST test dmg-acid2.gb in CGB mode
   - MUST use pixel-perfect screenshot comparison against reference images
   - MUST use appropriate timeout values based on test completion time

2. **Test Organization**
   - MUST separate PPU tests from CPU/timing tests
   - MUST maintain all existing test functionality
   - MUST preserve test execution time (~2-4 seconds total)

3. **Documentation**
   - MUST document known issues (DMG mode rendering)
   - MUST update README with new test structure
   - MUST update AGENTS.md with test additions

### Non-Functional Requirements

1. **Performance**: Total test suite execution time MUST remain under 5 seconds
2. **Reliability**: Tests MUST be deterministic and repeatable
3. **Maintainability**: Test organization MUST clearly indicate what each module tests

## User Stories

### As a developer

- I want to validate PPU rendering accuracy so I can ensure display output is correct
- I want to know when DMG vs CGB mode rendering differs so I can fix mode-specific bugs
- I want test files organized by subsystem so I can easily find and add related tests

### As a CI system

- I want fast test execution so builds complete quickly
- I want ignored tests documented so I know which features have known issues

## Acceptance Criteria

1. ✅ `test_dmg_acid2_cgb` passes (CGB mode rendering is correct)
2. ✅ `test_dmg_acid2_dmg` exists but is ignored with clear documentation of the issue
3. ✅ All existing Blargg tests continue to pass
4. ✅ Test suite completes in under 5 seconds
5. ✅ New `ppu_tests.rs` module contains all PPU-related tests
6. ✅ `blargg_tests.rs` contains only Blargg's test suite
7. ✅ Documentation reflects new test structure

## Technical Details

### Test ROM Source

- **dmg-acid2**: <https://github.com/mattcurrie/dmg-acid2>
- Reference images: `dmg-acid2-dmg.png`, `dmg-acid2-cgb.png`
- Already included in `test-roms/` collection (v7.0)

### Implementation Approach

1. Add `DMG_ACID2` timeout constant (240 frames)
2. Create `tests/ppu_tests.rs` with three tests:
   - `test_cgb_acid2` (existing, moved)
   - `test_dmg_acid2_dmg` (new, ignored)
   - `test_dmg_acid2_cgb` (new, passing)
3. Update `tests/blargg_tests.rs`:
   - Remove PPU tests
   - Rename helper to `run_blargg_test` for clarity
4. Update documentation

### Known Issues

- **DMG mode rendering**: Screenshot comparison fails in DMG mode, indicating a PPU rendering bug specific to DMG
  palette handling or display behavior. CGB mode passes, confirming the issue is mode-specific.

## Out of Scope

- Fixing the DMG PPU rendering bug (tracked separately)
- Adding other PPU accuracy tests (mealybug-tearoom, etc.)
- Optimizing test execution time further

## Dependencies

- Existing test infrastructure (`ceres-test-runner`)
- Test ROM collection v7.0 or later (includes dmg-acid2)
- `expected_screenshot_path` helper function for model-specific screenshots

## Success Metrics

- All 8 non-ignored tests pass
- Total test suite execution time < 5 seconds
- Clear separation between test categories (Blargg vs PPU)
- Known issue documented and tracked

## Related Work

- Original cgb-acid2 test: commit `<hash>` (001-add-cgb-acid2-test)
- Test infrastructure: `ceres-test-runner/`
- Reference: <https://github.com/mattcurrie/dmg-acid2>
