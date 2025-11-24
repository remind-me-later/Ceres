# Tasks: Debug and Fix Ignored Mooneye PPU Tests

## Status Summary

**3 of 10 tests now passing:**

- ✅ `stat_irq_blocking` - Edge-triggered STAT interrupts
- ✅ `stat_lyc_onoff` - LY/LYC coincidence during LCD on/off
- ✅ `vblank_stat_intr_gs` - VBlank OAM interrupt quirk at line 144

**7 tests still failing (require cycle-accurate PPU):**

- ❌ `intr_2_mode0_timing` - Mode timing precision
- ❌ `intr_2_mode0_timing_sprites` - Mode timing with sprites
- ❌ `intr_2_mode3_timing` - Mode 3 timing
- ❌ `intr_2_oam_ok_timing` - OAM accessibility timing
- ❌ `lcdon_timing_gs` - LCD enable timing (DMG)
- ❌ `lcdon_write_timing_gs` - LCD enable write timing (DMG)
- ❌ `hblank_ly_scx_timing_gs` - SCX affects LY increment timing

## 1. Analysis and Investigation

- [x] 1.1 Run each ignored PPU test individually to capture failure details
- [x] 1.2 Add tracing to PPU mode transitions and STAT interrupt generation
- [x] 1.3 Compare Ceres PPU state machine with SameBoy's `GB_display_run` implementation
- [x] 1.4 Document exact timing differences using Perfetto traces

## 2. STAT Interrupt Line Tracking ✅

- [x] 2.1 Add `stat_interrupt_line: bool` field to track internal STAT signal
- [x] 2.2 Add `previous_stat_line: bool` to detect rising edge (implemented inline in update_stat)
- [x] 2.3 Modify interrupt generation to only fire on rising edge transitions
- [x] 2.4 Verify `stat_irq_blocking` test passes

## 3. LY=LYC Coincidence Handling ✅

- [x] 3.1 Add `ly_for_comparison: u8` field separate from displayed LY
- [x] 3.2 Implement coincidence flag retention when LCD is off
- [x] 3.3 Implement proper coincidence clock behavior (stop on LCD off, restart on LCD on)
- [x] 3.4 Implement interrupt suppression when coincidence doesn't _change_
- [x] 3.5 Verify `stat_lyc_onoff` test passes

## 4. LCD Enable Timing (DEFERRED - Requires Cycle-Accurate PPU)

- [ ] 4.1 Implement line 0 starting in Mode 0 (not Mode 2) on LCD enable
- [ ] 4.2 Implement 2 T-cycle "late" timing for first line
- [ ] 4.3 Implement proper OAM/VRAM accessibility timing after LCD enable
- [ ] 4.4 Verify `lcdon_timing-GS` test passes
- [ ] 4.5 Verify `lcdon_write_timing-GS` test passes

## 5. Mode Timing Precision (DEFERRED - Requires Cycle-Accurate PPU)

- [ ] 5.1 Implement variable Mode 3 duration based on sprite X coordinates
- [ ] 5.2 Implement proper Mode 2 to Mode 3 timing (~3-4 cycles)
- [ ] 5.3 Implement proper Mode 2 to Mode 0 timing (~46-47 cycles to OAM readable)
- [ ] 5.4 Verify `intr_2_mode0_timing` test passes
- [ ] 5.5 Verify `intr_2_mode0_timing_sprites` test passes
- [ ] 5.6 Verify `intr_2_mode3_timing` test passes
- [ ] 5.7 Verify `intr_2_oam_ok_timing` test passes

## 6. VBlank STAT Interrupt ✅

- [x] 6.1 Implement STAT Mode 2 interrupt trigger at line 144 (when Mode 2 OAM enabled)
- [x] 6.2 Verify `vblank_stat_intr-GS` test passes

## 7. SCX Timing Effect (DEFERRED - Requires Cycle-Accurate PPU)

- [ ] 7.1 Implement SCX-dependent LY increment timing after Mode 0 interrupt
- [ ] 7.2 Verify `hblank_ly_scx_timing-GS` test passes

## 8. Validation and Cleanup

- [x] 8.1 Remove `#[ignore]` from passing PPU tests (3 tests)
- [x] 8.2 Run full Mooneye test suite to ensure no regressions
- [x] 8.3 Update test documentation with new pass count
- [ ] 8.4 Consider separate proposal for cycle-accurate PPU rewrite
