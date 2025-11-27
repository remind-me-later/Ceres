# Change: Debug and Fix Ignored Mooneye PPU Tests (Partial)

## Why

This change delivers verified fixes for STAT interrupt edge detection and LY=LYC coincidence logic, enabling 4 Mooneye tests to pass. Cycle-accurate timing changes (sprites, SCX, window) were investigated but deferred due to complexity and the need for a complete PPU fetcher rewrite.

## What Changes

- **STAT Interrupt Logic:**
  - Implemented internal STAT line tracking (`stat_interrupt_line`).
  - Interrupts now fire only on rising edge transitions.
  - Fixes `stat_irq_blocking` test.

- **LY=LYC Coincidence:**
  - Separated displayed `ly` from `ly_for_comparison`.
  - Implemented coincidence flag retention when LCD is off.
  - Fixes `stat_lyc_onoff` test.

- **VBlank Interrupt:**
  - Implemented quirk where STAT Mode 2 interrupt fires at line 144 (VBlank start) if OAM interrupt is enabled.
  - Fixes `vblank_stat_intr-GS` test.

- **OAM Timing:**
  - Adjusted Mode 2 duration to 80 cycles.
  - Passes `intr_2_oam_ok_timing` test.

## Deferred Work

The following timing-sensitive tests remain failing and require a cycle-accurate fetcher implementation:
- `intr_2_mode0_timing`
- `intr_2_mode0_timing_sprites`
- `intr_2_mode3_timing`
- `lcdon_timing-GS`
- `hblank_ly_scx_timing-GS`

## Impact

- **Passing Tests:** 4 additional Mooneye PPU tests now pass.
- **Codebase:** Cleaned up PPU interrupt logic in `ceres-core`.