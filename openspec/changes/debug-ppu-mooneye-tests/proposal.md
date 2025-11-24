# Change: Debug and Fix Ignored Mooneye PPU Tests

## Why

The Ceres emulator currently fails 10 out of 12 Mooneye PPU timing tests. These tests validate critical PPU behaviors
including STAT interrupt blocking, mode timing, LY/LYC coincidence handling, and LCD enable/disable behavior. Passing
these tests is essential for accurate PPU emulation as verified against real hardware.

The failing tests are:

| Test                          | Status     | Description                                             |
| ----------------------------- | ---------- | ------------------------------------------------------- |
| `hblank_ly_scx_timing-GS`     | ❌ Ignored | Tests how SCX affects HBlank-to-LY-increment timing     |
| `intr_2_mode0_timing`         | ❌ Ignored | Tests Mode 2 to Mode 0 transition timing                |
| `intr_2_mode0_timing_sprites` | ❌ Ignored | Tests Mode 0 timing with sprites                        |
| `intr_2_mode3_timing`         | ❌ Ignored | Tests Mode 2 to Mode 3 transition timing                |
| `intr_2_oam_ok_timing`        | ❌ Ignored | Tests when OAM becomes readable after Mode 2 interrupt  |
| `lcdon_timing-GS`             | ❌ Ignored | Tests PPU behavior after LCD enable                     |
| `lcdon_write_timing-GS`       | ❌ Ignored | Tests OAM/VRAM write timing after LCD enable            |
| `stat_irq_blocking`           | ❌ Ignored | Tests STAT IRQ blocking when internal signal stays high |
| `stat_lyc_onoff`              | ❌ Ignored | Tests LY=LYC behavior when LCD is turned on/off         |
| `vblank_stat_intr-GS`         | ❌ Ignored | Tests Mode 2 OAM interrupt at line 144 (VBlank start)   |

**Note:** Tests with `-GS` suffix are DMG/SGB-specific and fail on CGB. Tests without suffix should pass on all models.

## What Changes

### Missing Feature: STAT Interrupt Line Tracking (Blocking)

**Current Behavior:** Ceres fires STAT interrupts on every condition match.

**Expected Behavior (SameBoy):** SameBoy tracks a `stat_interrupt_line` and only fires interrupts on the _rising edge_
(transition from false to true). This prevents multiple interrupts while the STAT line stays high.

The `stat_irq_blocking` test validates this behavior:

- When LY=LYC is set before entering Mode 3 and kept set through the scanline
- Only Mode 3 can clear the internal interrupt line
- If coincidence remains, no new interrupts should fire

### Missing Feature: LY=LYC Handling During LCD On/Off

**Current Behavior:** Ceres resets LY to 0 on LCD disable but doesn't properly track the coincidence flag.

**Expected Behavior (SameBoy):**

- When LCD is off, the LY=LYC coincidence flag is _retained_ (not reset)
- When LYC is changed while LCD is off, the flag should NOT update
- When LCD is enabled, the comparison clock starts again
- If LY=0 matches LYC=0 on LCD enable, this should trigger an interrupt (if not already matching)

### Missing Feature: Precise Mode Timing

**Current Behavior:** Mode durations use simplified calculations.

**Expected Behavior:**

- Mode 3 duration varies based on sprite X coordinates (affects when Mode 0 starts)
- Mode 2 to Mode 0 transition: ~80 cycles OAM scan + ~168-291 cycles Mode 3
- OAM becomes readable ~46-47 cycles after Mode 2 interrupt
- Mode 3 starts ~3-4 cycles after Mode 2 interrupt

### Missing Feature: LCD Enable First-Line Behavior

**Current Behavior:** Ceres sets Mode to HBlank immediately on LCD enable.

**Expected Behavior:**

- Line 0 starts in Mode 0 (not Mode 2) and goes straight to Mode 3
- Line 0 has different timings - PPU is "late" by 2 T-cycles
- Lines 1+ have normal timings

### Missing Feature: VBlank STAT Mode 2 Interrupt at Line 144

**Current Behavior:** Only VBlank interrupt fires at line 144.

**Expected Behavior:** If STAT bit 5 (Mode 2 OAM interrupt) is enabled, a STAT interrupt should also fire at line 144
when VBlank begins (same cycle as VBlank interrupt).

### Missing Feature: SCX Effect on HBlank/LY Timing

**Current Behavior:** SCX affects Mode 3 duration but not the relationship to LY increment.

**Expected Behavior:**

- (SCX mod 8) = 0: LY increments 51 cycles after Mode 0 interrupt
- (SCX mod 8) = 1-4: LY increments 50 cycles after Mode 0 interrupt
- (SCX mod 8) = 5-7: LY increments 49 cycles after Mode 0 interrupt

## Impact

- **Affected specs:** `ppu-stat` (new capability)
- **Affected code:**
  - `ceres-core/src/ppu/mod.rs` - STAT interrupt logic, mode transitions
  - `ceres-core/src/interrupts.rs` - May need interrupt line tracking
- **Breaking changes:** None (behavior corrections for accuracy)
- **Test impact:** Should enable 10 currently ignored Mooneye PPU tests
