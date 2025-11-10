# Implementation Tasks

## 1. Test Infrastructure

- [ ] 1.1 Add `MBC3_TESTER` timeout constant to `test_runner.rs::timeouts` module
- [ ] 1.2 Create `mbc3_tests.rs` test file in `ceres-test-runner/tests/` directory

## 2. Test Implementation

- [ ] 2.1 Implement `test_mbc3_tester_cgb` test function for CGB mode
- [ ] 2.2 Implement `test_mbc3_tester_dmg` test function for DMG mode
- [ ] 2.3 Use screenshot comparison with reference images

## 3. Validation

- [ ] 3.1 Run new tests locally to verify they pass
- [ ] 3.2 Verify tests complete within timeout (40 frames ~0.7s)
- [ ] 3.3 Confirm CI pipeline includes new tests

## Dependencies

- Test ROM and reference screenshots already exist in `test-roms/mbc3-tester/`
- No new dependencies required
- Uses existing `TestRunner` infrastructure
