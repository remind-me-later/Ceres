# Tasks: Debug and Fix Ignored Mooneye PPU Tests

## Status Summary

**4 tests passing, 6 tests deferred.**

## 1. Implementation ✅

- [x] 1.1 Implement STAT interrupt edge detection (`stat_irq_blocking`)
- [x] 1.2 Implement LY=LYC coincidence retention (`stat_lyc_onoff`)
- [x] 1.3 Implement VBlank STAT Mode 2 interrupt (`vblank_stat_intr-GS`)
- [x] 1.4 Implement correct OAM scan duration (80 cycles) (`intr_2_oam_ok_timing`)

## 2. Verification ✅

- [x] 2.1 Verify `stat_irq_blocking` passes
- [x] 2.2 Verify `stat_lyc_onoff` passes
- [x] 2.3 Verify `vblank_stat_intr-GS` passes
- [x] 2.4 Verify `intr_2_oam_ok_timing` passes

## 3. Deferred (Requires Cycle-Accurate Fetcher)

- [ ] 3.1 `intr_2_mode0_timing`
- [ ] 3.2 `intr_2_mode0_timing_sprites`
- [ ] 3.3 `intr_2_mode3_timing`
- [ ] 3.4 `lcdon_timing-GS`
- [ ] 3.5 `hblank_ly_scx_timing-GS`