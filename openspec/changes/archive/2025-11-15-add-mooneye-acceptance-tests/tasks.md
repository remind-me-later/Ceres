## 1. Add CPU Register Reading API

- [x] 1.1 Add public `cpu_b()` method to `Gb` struct in `ceres-core/src/lib.rs` that returns `(self.cpu.bc() >> 8) as
  u8`
- [x] 1.2 Add public `cpu_c()` method to `Gb` struct that returns `(self.cpu.bc() & 0xFF) as u8`
- [x] 1.3 Add public `cpu_d()` method to `Gb` struct that returns `(self.cpu.de() >> 8) as u8`
- [x] 1.4 Add public `cpu_e()` method to `Gb` struct that returns `(self.cpu.de() & 0xFF) as u8`
- [x] 1.5 Add public `cpu_h()` method to `Gb` struct that returns `(self.cpu.hl() >> 8) as u8`
- [x] 1.6 Add public `cpu_l()` method to `Gb` struct that returns `(self.cpu.hl() & 0xFF) as u8`

## 2. Add Mooneye Test Validation Logic

- [x] 2.1 Add `MOONEYE_ACCEPTANCE` timeout constant (7160 frames) to `test_runner::timeouts` module
- [x] 2.2 Add `check_mooneye_result()` method to `TestRunner` that reads CPU registers and checks for Fibonacci
  sequence (3, 5, 8, 13, 21, 34) or failure code (0x42 in all registers)
- [x] 2.3 Update `check_completion()` method to call `check_mooneye_result()` when breakpoint is detected for
  Mooneye tests
- [x] 2.4 Add `TestConfig` field to distinguish Mooneye tests from other test types (or use a new validation mode enum)

## 3. Create Mooneye Test Runner Helper

- [x] 3.1 Create helper function `run_mooneye_test(path: &str, model: Model)` that loads ROM, configures TestRunner
  with Mooneye validation, and returns TestResult
- [x] 3.2 Helper should use `MOONEYE_ACCEPTANCE` timeout
- [x] 3.3 Helper should set appropriate model (DMG, MGB, CGB) based on test name hints
- [x] 3.4 Helper should enable Mooneye-specific validation mode

## 4. Add Mooneye Test File Structure

- [x] 4.1 Create `ceres-test-runner/tests/mooneye_tests.rs` file
- [x] 4.2 Add module documentation explaining Mooneye test suite and validation approach
- [x] 4.3 Import necessary types from `ceres_test_runner` and `ceres_core`
- [x] 4.4 Add comment explaining that failing tests are marked with `#[ignore]` and will be fixed incrementally

## 5. Implement Root-Level Acceptance Tests

- [x] 5.1 Add tests for timing tests (e.g., `add_sp_e_timing`, `call_cc_timing`, `div_timing`, etc.)
- [x] 5.2 Add tests for interrupt tests (e.g., `ei_sequence`, `ei_timing`, `halt_ime0_ei`, etc.)
- [x] 5.3 Add tests for instruction tests (e.g., `call_timing`, `jp_timing`, `ret_timing`, etc.)
- [x] 5.4 Add tests for boot register tests with model-specific variants (e.g., `boot_regs_dmg0`, `boot_regs_cgb`)
- [x] 5.5 Add tests for OAM DMA tests (e.g., `oam_dma_restart`, `oam_dma_start`, `oam_dma_timing`)
- [x] 5.6 Mark failing tests with `#[ignore]` and tracking comments

## 6. Implement bits/ Subdirectory Tests

- [x] 6.1 Add test for `bits/mem_oam.gb`
- [x] 6.2 Add test for `bits/reg_f.gb`
- [x] 6.3 Add test for `bits/unused_hwio-GS.gb`
- [x] 6.4 Mark failing tests with `#[ignore]` and tracking comments

## 7. Implement instr/ Subdirectory Tests

- [x] 7.1 Add test for `instr/daa.gb`
- [x] 7.2 Mark as ignored if failing with tracking comment

## 8. Implement interrupts/ Subdirectory Tests

- [x] 8.1 Add test for `interrupts/ie_push.gb`
- [x] 8.2 Mark as ignored if failing with tracking comment

## 9. Implement oam_dma/ Subdirectory Tests

- [x] 9.1 Add test for `oam_dma/basic.gb`
- [x] 9.2 Add test for `oam_dma/reg_read.gb`
- [x] 9.3 Add test for `oam_dma/sources-GS.gb`
- [x] 9.4 Mark failing tests with `#[ignore]` and tracking comments

## 10. Implement ppu/ Subdirectory Tests

- [x] 10.1 Add tests for STAT interrupt timing tests (e.g., `intr_1_2_timing-GS`, `intr_2_0_timing`, etc.)
- [x] 10.2 Add tests for LCD timing tests (e.g., `lcdon_timing-GS`, `hblank_ly_scx_timing-GS`)
- [x] 10.3 Add tests for STAT interrupt behavior (e.g., `stat_irq_blocking`, `stat_lyc_onoff`, `vblank_stat_intr-GS`)
- [x] 10.4 Mark failing tests with `#[ignore]` and tracking comments

## 11. Implement serial/ Subdirectory Tests

- [x] 11.1 Add test for `serial/boot_sclk_align-dmgABCmgb.gb`
- [x] 11.2 Mark as ignored if failing with tracking comment

## 12. Implement timer/ Subdirectory Tests

- [x] 12.1 Add tests for DIV register tests (e.g., `div_write`)
- [x] 12.2 Add tests for timer tests (e.g., `tim00`, `tim01`, `tim10`, `tim11`)
- [x] 12.3 Add tests for timer trigger tests (e.g., `tim00_div_trigger`, `tim01_div_trigger`, etc.)
- [x] 12.4 Add tests for TIMA register tests (e.g., `tima_reload`, `tima_write_reloading`, `tma_write_reloading`)
- [x] 12.5 Add test for `rapid_toggle`
- [x] 12.6 Mark failing tests with `#[ignore]` and tracking comments

## 13. Run Tests and Document Passing Tests

- [x] 13.1 Run all Mooneye tests with `cargo test --package ceres-test-runner mooneye`
- [x] 13.2 Document which tests currently pass (unignore them)
- [x] 13.3 Document which tests fail (keep them ignored with clear tracking comments)
- [x] 13.4 Update CI configuration if needed to include Mooneye tests in the test suite

## 14. Documentation and Validation

- [x] 14.1 Update `AGENTS.md` to mention Mooneye acceptance tests in the Testing section
- [x] 14.2 Add comment in test file listing the count of passing vs failing tests
- [x] 14.3 Run `cargo fmt` on all modified files
- [x] 14.4 Run `cargo clippy` and fix any warnings
- [x] 14.5 Verify all passing tests complete successfully in CI
