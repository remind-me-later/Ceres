# Design: Cycle-Accurate PPU with Pixel FIFO

## Context

The Game Boy PPU (Pixel Processing Unit) renders 160x144 pixels at ~59.7 FPS. Each scanline takes exactly 456 T-cycles
(114 M-cycles), divided into:

- **Mode 2 (OAM Scan)**: 80 dots - Search OAM for sprites on this line
- **Mode 3 (Drawing)**: 172-289 dots - Fetch tiles and push pixels to LCD
- **Mode 0 (HBlank)**: 87-204 dots - Horizontal blanking (remainder of 456)
- **Mode 1 (VBlank)**: 4560 dots (10 lines × 456) - Vertical blanking

Current Ceres implementation uses fixed mode durations with a simple SCX adjustment, which cannot pass timing tests that
measure exact T-cycle transitions.

### Stakeholders

- Game compatibility: Titles using raster effects (mid-scanline register changes)
- Test compliance: Mooneye, SameSuite, and other accuracy test ROMs
- Performance: Must maintain 60 FPS on target platforms

## Goals / Non-Goals

### Goals

1. Pass all 10 Mooneye PPU timing tests
2. Implement pixel FIFO architecture matching real hardware
3. Cycle-accurate mode transitions detectable by software
4. Maintain backward compatibility with existing frontends

### Non-Goals

1. Sub-T-cycle accuracy (4 MHz granularity is sufficient)
2. CGB-specific double-speed mode differences (defer to later)
3. LCD analog artifacts (ghosting, response time)
4. SGB timing differences

## Decisions

### Decision 1: Pixel FIFO Architecture

Implement two separate FIFOs as described in Pan Docs:

```rust
/// A single pixel in the FIFO
#[derive(Clone, Copy, Default)]
pub struct FifoPixel {
    /// Color index (0-3)
    pub color: u8,
    /// Palette index (BG: 0-7 CGB, 0 DMG; OBJ: 0-7 CGB, 0-1 DMG)
    pub palette: u8,
    /// Sprite priority (OAM index for CGB, 0 for DMG)
    pub priority: u8,
    /// Background priority flag (BG-to-OAM priority in CGB mode)
    pub bg_priority: bool,
}

/// Fixed-size FIFO with 8-pixel capacity
pub struct PixelFifo {
    pixels: [FifoPixel; 8],
    read_pos: u8,
    size: u8,
}
```

**Why**: This matches the hardware implementation where two independent FIFOs (background and sprite) operate in
parallel, with sprite pixels overlaying background pixels based on priority rules.

**Alternatives considered**:

- Single FIFO with pre-mixed pixels: Would lose sprite priority information needed for proper mixing
- Ring buffer with dynamic sizing: More complex, no real benefit for fixed 8-pixel size

### Decision 2: Fetcher State Machine

Implement a 5-step state machine that runs in parallel with pixel output:

```rust
#[derive(Clone, Copy, Debug, Default)]
pub enum FetcherState {
    #[default]
    GetTileT1,      // Read tile index from tilemap (cycle 1)
    GetTileT2,      // Read tile index from tilemap (cycle 2)
    GetDataLowT1,   // Read low byte of tile data (cycle 1)
    GetDataLowT2,   // Read low byte of tile data (cycle 2)
    GetDataHighT1,  // Read high byte of tile data (cycle 1)
    GetDataHighT2,  // Read high byte of tile data (cycle 2)
    Push,           // Attempt to push 8 pixels to FIFO
}
```

Each step takes 2 T-cycles (1 dot on the 4 MHz pixel clock). The Push step repeats until the FIFO has space.

**Why**: Direct mapping to SameBoy's proven implementation. Each T-cycle pair performs one VRAM access.

### Decision 3: Mode 3 Timing Calculation

Mode 3 duration = base (172 dots) + penalties:

1. **SCX penalty**: `SCX % 8` dots discarded at scanline start
2. **Window penalty**: 6 dots when window becomes active mid-scanline
3. **Sprite penalty**: 6-11 dots per sprite, calculated as:

   ```rust
   fn sprite_penalty(sprite_x: u8) -> u8 {
       let base_penalty = 11 - min((sprite_x + (scx % 8)) % 8, 5);
       // Additional penalty if sprite fetch interrupts background fetch
       base_penalty
   }
   ```

**Why**: This formula matches Pan Docs and SameBoy behavior. Sprites at the left edge of the screen (low X) cause more
penalty because the background fetcher must be halted and restarted.

### Decision 4: Per-Dot Execution Model

Replace the current `run(dots: i32)` approach with a state machine that can be advanced one dot at a time:

```rust
impl Ppu {
    /// Advance PPU by one T-cycle (dot)
    pub fn tick(&mut self, ints: &mut Interrupts, cgb_mode: CgbMode) {
        match self.mode {
            Mode::OamScan => self.tick_oam_scan(ints),
            Mode::Drawing => self.tick_drawing(ints, cgb_mode),
            Mode::HBlank => self.tick_hblank(ints),
            Mode::VBlank => self.tick_vblank(ints),
        }
    }

    /// Advance PPU by multiple T-cycles (for bulk operation)
    pub fn run(&mut self, dots: i32, ints: &mut Interrupts, cgb_mode: CgbMode) {
        for _ in 0..dots {
            self.tick(ints, cgb_mode);
        }
    }
}
```

**Why**: Tests measure exact cycle counts. Bulk execution can still call `tick()` in a loop, but timing-sensitive code
paths get exact T-cycle resolution.

**Performance consideration**: Hot path optimization - when no mid-scanline effects are needed (no sprites on line, no
window, etc.), batch operations can be used.

### Decision 5: Memory Access Blocking

Track OAM and VRAM accessibility per-dot:

```rust
pub struct Ppu {
    // ... existing fields ...

    /// OAM is blocked during Mode 2 and Mode 3
    oam_blocked: bool,
    /// VRAM is blocked during Mode 3 (when fetcher is active)
    vram_blocked: bool,
}

impl Ppu {
    pub fn is_oam_accessible(&self) -> bool {
        !self.oam_blocked || self.lcdc & LCDC_ON_B == 0
    }

    pub fn is_vram_accessible(&self) -> bool {
        !self.vram_blocked || self.lcdc & LCDC_ON_B == 0
    }
}
```

Memory module calls these during OAM/VRAM reads to return 0xFF when blocked.

## Risks / Trade-offs

### Performance Risk

**Risk**: Per-dot execution is ~4x more function calls than per-scanline.

**Mitigation**:

1. Inline hot paths aggressively
2. Add fast path for "no sprites on line" case (batch Mode 3)
3. Profile and optimize after correctness is achieved
4. Consider SIMD for FIFO operations if needed

### Complexity Risk

**Risk**: State machine is significantly more complex than current implementation.

**Mitigation**:

1. Extensive test coverage (10 Mooneye tests + existing tests)
2. Add tracing/debugging support for state machine visualization
3. Document state transitions thoroughly

### Regression Risk

**Risk**: Changes may break games that accidentally worked with incorrect timing.

**Mitigation**:

1. Run full test suite before and after
2. Test popular games manually
3. Keep old implementation behind feature flag during transition

## Migration Plan

### Phase 1: Infrastructure (Non-Breaking)

1. Add `PixelFifo` and `FetcherState` types
2. Add new fields to `Ppu` struct without changing behavior
3. Add `tick()` method that calls through to existing `run()` logic

### Phase 2: FIFO Implementation

1. Implement `tick_oam_scan()` with proper sprite collection
2. Implement `tick_drawing()` with fetcher state machine
3. Implement pixel mixing logic
4. Run rendering through FIFO path, verify visual output matches

### Phase 3: Timing Accuracy

1. Implement variable Mode 3 duration
2. Add sprite penalty calculation
3. Add window penalty handling
4. Add SCX penalty handling

### Phase 4: Memory Blocking

1. Implement OAM blocking during Mode 2/3
2. Implement VRAM blocking during Mode 3
3. Update memory read/write paths

### Phase 5: Validation

1. Enable all 10 Mooneye PPU tests
2. Run full test suite
3. Test raster effect games (if available)

### Rollback

If critical issues are found:

1. `run()` can fall back to scanline rendering via feature flag
2. Tests can be re-ignored while issues are investigated

## Data Structures (Detailed)

### PPU State

```rust
pub struct Ppu {
    // Existing fields (preserved)
    lcdc: u8,
    stat: u8,
    ly: u8,
    lyc: u8,
    scx: u8,
    scy: u8,
    wx: u8,
    wy: u8,
    bgp: u8,
    obp0: u8,
    obp1: u8,
    // ... palettes, buffers, etc.

    // New FIFO fields
    bg_fifo: PixelFifo,
    oam_fifo: PixelFifo,
    fetcher_state: FetcherState,
    fetcher_x: u8,           // Current tile X being fetched
    fetcher_y: u8,           // Y coordinate for tile lookup
    current_tile: u8,        // Tile index being fetched
    current_tile_attrs: u8,  // CGB tile attributes
    current_tile_data: [u8; 2], // Low and high bytes

    // Sprite fetching
    visible_sprites: [SpriteEntry; 10],
    n_visible_sprites: u8,
    sprite_fetch_index: u8,

    // Timing
    dots_in_line: u16,      // Current dot within scanline (0-455)
    position_in_line: i16,  // LCD X position (-8 to 167, negative = scroll discard)
    lcd_x: u8,              // Actual LCD X (0-159)

    // Window state
    window_triggered: bool,
    window_line: u8,

    // Memory blocking
    oam_blocked: bool,
    vram_blocked: bool,
}
```

### Sprite Entry

```rust
#[derive(Clone, Copy, Default)]
pub struct SpriteEntry {
    pub y: u8,        // Y position (actual = y - 16)
    pub x: u8,        // X position (actual = x - 8)
    pub tile: u8,     // Tile index
    pub flags: u8,    // Attributes (palette, flip, priority)
    pub oam_index: u8, // Original OAM index (for priority)
}
```

## Open Questions

1. **Double-speed CGB mode**: Does the fetcher run at 8 MHz or 4 MHz? (Likely 8 MHz with same relative timing - defer
   investigation)

2. **HDMA during Mode 3**: How does HDMA interact with pixel FIFO? (Research needed, not blocking for basic
   implementation)

3. **Mid-scanline LCDC writes**: How exactly do writes to LCDC.4 (tile data select) affect in-progress fetches? (SameBoy
   has `tile_sel_glitch` handling)

4. **Sprite-at-X-0 behavior**: Sprites at X=0 have special handling in SameBoy (`objects_x[gb->n_visible_objs - 1] == 0`
   check) - need to understand exact behavior.
