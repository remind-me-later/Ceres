# Implementation Tasks

## 1. Capture Execution Trace

- [ ] 1.1 Run mbc3-tester ROM with `--trace` flag:
      `cargo run --package ceres-winit -- --trace test-roms/mbc3-tester/mbc3-tester.gb 2> mbc3-trace.log`
- [ ] 1.2 Capture first 20,000 instructions (or until bank test loop is visible)
- [ ] 1.3 Take screenshot of emulator output for comparison

## 2. Analyze MBC3 Control Register Access

- [ ] 2.1 Search trace for writes to $0000-$1FFF (RAM enable/disable)
- [ ] 2.2 Search trace for writes to $2000-$3FFF (ROM bank selection)
- [ ] 2.3 Search trace for writes to $4000-$5FFF (RAM bank/RTC register selection)
- [ ] 2.4 Search trace for writes to $6000-$7FFF (RTC latch)
- [ ] 2.5 Document the sequence of bank switching operations

## 3. Compare with Expected Behavior

- [ ] 3.1 Review mbc3-tester disassembled source code (if available)
- [ ] 3.2 Verify ROM bank numbers are written correctly (1-127, bank 0 maps to bank 1)
- [ ] 3.3 Check if ROM bank 0 special case is handled (writing 0x00 should use bank 0x01)
- [ ] 3.4 Compare emulator screenshot with reference: `test-roms/mbc3-tester/mbc3-tester-dmg.png`
- [ ] 3.5 Compare emulator screenshot with reference: `test-roms/mbc3-tester/mbc3-tester-cgb.png`

## 4. Identify Issues

- [ ] 4.1 Document any discrepancies between expected and actual bank switching
- [ ] 4.2 Check if visual output matches reference (indicates correct bank reads)
- [ ] 4.3 If issues found, identify root cause in `ceres-core/src/cartridge/mbc3.rs`
- [ ] 4.4 Create list of bugs to fix

## 5. Fix MBC3 Implementation (if needed)

- [ ] 5.1 Fix ROM bank selection logic if incorrect
- [ ] 5.2 Fix ROM bank 0 special case handling if needed
- [ ] 5.3 Fix RAM bank selection if incorrect
- [ ] 5.4 Test fixes with trace to verify correct behavior
- [ ] 5.5 Re-run mbc3-tester and compare screenshots

## 6. Add Integration Test

- [ ] 6.1 Create test file or add to existing `ceres-test-runner/tests/` file
- [ ] 6.2 Add DMG test case comparing against `mbc3-tester-dmg.png`
- [ ] 6.3 Add CGB test case comparing against `mbc3-tester-cgb.png`
- [ ] 6.4 Set appropriate timeout (test should complete in ~40 frames = ~0.7 seconds)
- [ ] 6.5 Run `cargo test --package ceres-test-runner` to verify test passes
- [ ] 6.6 Add test to CI/CD pipeline (should already run automatically)

## 7. Documentation

- [ ] 7.1 Document trace analysis findings in this change proposal
- [ ] 7.2 Document any MBC3 bugs found and fixed
- [ ] 7.3 Update parent proposal `add-sm83-disassembler` to mark MBC3 debugging complete

## Success Criteria

- Execution trace captured and analyzed
- MBC3 bank switching behavior understood
- Any bugs in MBC3 implementation fixed
- Emulator screenshots match reference images
- Integration test added and passing
- Zero regressions in existing tests

## Estimated Time

- Trace capture: 30 minutes
- Trace analysis: 1-2 hours
- Bug fixing (if needed): 2-4 hours
- Integration test: 1 hour
- **Total: 4-8 hours**
