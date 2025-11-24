# Change: Implement Cycle-Accurate PPU with Pixel FIFO

## Why

The current PPU implementation uses a scanline-based rendering approach with approximate mode timings. While this works
for most games, it fails 7 out of 10 Mooneye PPU timing tests that require cycle-accurate behavior. These tests verify:

1. **Mode transition timing** - Exact T-cycle counts between mode changes
2. **SCX timing effects** - How scroll position affects Mode 3 duration
3. **Sprite timing penalties** - How sprites at various X positions extend Mode 3
4. **LCD enable behavior** - Precise timing of first scanline after LCD turns on
5. **Memory access blocking** - When exactly OAM/VRAM become inaccessible

Games that rely on mid-scanline LCDC/scroll register changes (raster effects) or precise STAT interrupt timing will
exhibit visual glitches or incorrect behavior without cycle-accurate PPU emulation.

## What Changes

### Core Architecture

- **BREAKING**: Replace scanline-based rendering with dot-by-dot pixel FIFO execution
- Add two pixel FIFOs (background and OAM/sprite) with 8-pixel capacity each
- Implement 5-step fetcher state machine running in parallel with FIFO consumption
- Track PPU progress per T-cycle instead of per scanline

### Timing Model

- Replace fixed mode durations with variable timing based on:
  - SCX % 8 penalty at scanline start (0-7 dots)
  - Window activation penalty (6 dots)
  - Per-sprite penalties (6-11 dots each, based on X position)
- Implement proper Mode 3 duration: 172 base + penalties (up to 289 dots total)

### Memory Access

- Implement cycle-accurate OAM blocking during Mode 2 and Mode 3
- Implement cycle-accurate VRAM blocking during Mode 3
- Track which memory regions are accessible at each dot

### LCD Enable

- Implement proper first-frame behavior after LCD enable
- Handle Mode 0 start condition on line 0
- Track 76-dot delay before normal operation

## Impact

- **Affected specs**: `openspec/specs/ppu/` (new capability spec to be created)
- **Affected code**:
  - `ceres-core/src/ppu/mod.rs` - Complete rewrite of `Ppu::run()`
  - `ceres-core/src/ppu/fifo.rs` - New pixel FIFO implementation
  - `ceres-core/src/ppu/fetcher.rs` - New fetcher state machine
  - `ceres-core/src/memory/mod.rs` - Memory access blocking updates
- **Test targets**: 7 currently failing Mooneye PPU tests:
  - `intr_2_mode0_timing`
  - `intr_2_mode0_timing_sprites`
  - `intr_2_mode3_timing`
  - `intr_2_oam_ok_timing`
  - `lcdon_timing_gs`
  - `lcdon_write_timing_gs`
  - `hblank_ly_scx_timing_gs`

## References

- **Pan Docs**: https://gbdev.io/pandocs/pixel_fifo.html
- **SameBoy Implementation**: `Core/display.c` - `GB_display_run()`, `advance_fetcher_state_machine()`,
  `render_pixel_if_possible()`
- **Previous Work**: `openspec/changes/debug-ppu-mooneye-tests/` - STAT interrupt fixes (completed)
