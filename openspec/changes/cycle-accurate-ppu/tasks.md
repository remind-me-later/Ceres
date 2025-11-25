# Tasks: Implement Cycle-Accurate PPU with Pixel FIFO

## Target Tests

These 7 Mooneye PPU tests currently fail and should pass after implementation:

- [ ] `intr_2_mode0_timing` - Mode 0 interrupt timing
- [ ] `intr_2_mode0_timing_sprites` - Mode 0 timing with sprites
- [ ] `intr_2_mode3_timing` - Mode 3 duration timing
- [ ] `intr_2_oam_ok_timing` - OAM accessibility timing
- [ ] `lcdon_timing_gs` - LCD enable timing (DMG)
- [ ] `lcdon_write_timing_gs` - LCD enable write timing (DMG)
- [ ] `hblank_ly_scx_timing_gs` - SCX effect on HBlank timing

## Phase 1: Infrastructure

- [x] 1.1 Create `ceres-core/src/ppu/fifo.rs` with `FifoPixel` and `PixelFifo` structs
- [x] 1.2 Create `ceres-core/src/ppu/fetcher.rs` with `FetcherState` enum
- [x] 1.3 Create `ceres-core/src/ppu/sprite.rs` with `SpriteEntry` struct
- [x] 1.4 Add new fields to `Ppu` struct (FIFOs, fetcher state, timing counters)
- [x] 1.5 Add `tick()` method stub that calls existing `run()` logic
- [x] 1.7 Verify existing tests still pass

## Phase 2: Pixel FIFO Implementation

- [x] 2.1 Implement `PixelFifo::push_row()` - push 8 background pixels
- [x] 2.2 Implement `PixelFifo::pop()` - pop single pixel for LCD output
- [x] 2.3 Implement `PixelFifo::overlay_sprite_row()` - sprite mixing with priority
- [x] 2.4 Implement `PixelFifo::clear()` - reset FIFO state
- [x] 2.5 Implement `PixelFifo::size()` - current pixel count (0-8)
- [x] 2.6 Add unit tests for FIFO operations

## Phase 3: Fetcher State Machine

- [x] 3.1 Implement `GetTileT1/T2` - read tile index from tilemap
- [x] 3.2 Implement `GetDataLowT1/T2` - read low byte of tile data
- [x] 3.3 Implement `GetDataHighT1/T2` - read high byte of tile data
- [x] 3.4 Implement `Push` state - attempt to push pixels, loop until space
- [x] 3.5 Implement fetcher Y coordinate calculation (SCY + LY)
- [x] 3.6 Implement tile address calculation (respecting LCDC bits)
- [x] 3.7 Add CGB attribute handling (VRAM bank, flip, priority)

## Phase 4: Mode 2 - OAM Scan

- [x] 4.1 Implement sprite search (find up to 10 sprites on current line)
- [x] 4.2 Implement DMG sprite priority (X position, then OAM index)
- [x] 4.3 Implement CGB sprite priority (OAM index only when OPRI=0)
- [x] 4.4 Store sprites sorted for later fetching
- [x] 4.5 Track OAM blocking during Mode 2

## Phase 5: Mode 3 - Drawing

- [x] 5.1 Implement `tick_drawing()` main loop
- [x] 5.2 Advance fetcher state machine each dot
- [x] 5.3 Pop pixels from BG FIFO when size > 0
- [x] 5.4 Mix with OAM FIFO using priority rules
- [x] 5.5 Handle SCX scroll discard (first SCX % 8 pixels)
- [x] 5.6 Implement sprite fetch trigger (when sprite X matches position)
- [x] 5.7 Implement sprite penalty calculation
- [x] 5.8 Track VRAM blocking during Mode 3

## Phase 6: Window Handling

- [x] 6.1 Detect window activation (WX-7 == position_in_line && WY triggered)
- [x] 6.2 Implement window Y counter (increments only when window visible)
- [x] 6.3 Clear BG FIFO and restart fetcher when window activates
- [x] 6.4 Switch fetcher to window tilemap/coordinates
- [x] 6.5 Add window activation penalty (6 dots)

## Phase 7: Mode Transitions

- [x] 7.1 Implement Mode 2 → Mode 3 transition (after 80 dots)
- [x] 7.2 Implement Mode 3 → Mode 0 transition (after 160 pixels rendered)
- [x] 7.3 Implement Mode 0 → Mode 2/1 transition (at dot 456)
- [x] 7.4 Implement variable Mode 3 duration tracking
- [x] 7.5 Update STAT mode bits on exact cycle

## Phase 8: LCD Enable Timing

- [x] 8.1 Implement first-frame skip after LCD enable
- [x] 8.2 Implement 76-dot delay before Mode 3 on first line
- [x] 8.3 Handle LY=0 starting in Mode 0 (not Mode 2)
- [x] 8.4 Test with `lcdon_timing_gs`
- [x] 8.5 Test with `lcdon_write_timing_gs`

## Phase 9: Memory Blocking

- [x] 9.1 Add `is_oam_accessible()` method to Ppu
- [x] 9.2 Add `is_vram_accessible()` method to Ppu
- [x] 9.3 Update `memory/mod.rs` to check accessibility on OAM reads
- [x] 9.4 Update `memory/mod.rs` to check accessibility on VRAM reads
- [x] 9.5 Return 0xFF for blocked reads
- [x] 9.6 Implement OAM write blocking behavior
- [x] 9.7 Test with `intr_2_oam_ok_timing`

## Phase 10: Integration and Validation

- [ ] 10.2 Run all Mooneye PPU tests, verify 10/10 pass
- [ ] 10.3 Run full Mooneye test suite, check for regressions
- [ ] 10.4 Run Blargg CPU tests, verify no regressions
- [ ] 10.5 Test popular games with raster effects (if available)
- [ ] 10.6 Profile performance, optimize if needed
- [ ] 10.7 Remove `#[ignore]` from 7 PPU tests
- [ ] 10.8 Update test documentation with new pass count

## Phase 11: Cleanup

- [ ] 11.1 Remove scanline rendering code (or keep behind feature flag)
- [ ] 11.2 Update code documentation
- [ ] 11.3 Add debug tracing for FIFO state visualization
- [ ] 11.4 Archive this change proposal

## Dependencies

- Depends on: `debug-ppu-mooneye-tests` (completed - STAT interrupt fixes)
- Blocks: Any future PPU-related changes

## Notes

- SameBoy reference: `Core/display.c` - `GB_display_run()`, `advance_fetcher_state_machine()`
- Pan Docs reference: https://gbdev.io/pandocs/pixel_fifo.html
- Each fetcher step = 2 T-cycles (1 M-cycle)
- FIFO must have ≤8 pixels before push can succeed
