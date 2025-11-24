# ppu-fifo Specification Delta

## ADDED Requirements

### Requirement: Pixel FIFO Architecture

The PPU SHALL implement a pixel FIFO (First-In-First-Out) rendering pipeline with two independent
8-pixel FIFOs: one for background/window pixels and one for sprite (OAM) pixels.

#### Scenario: Background FIFO receives tile row

- **WHEN** the background fetcher completes fetching a tile row (8 pixels)
- **AND** the background FIFO has 0 pixels remaining
- **THEN** the fetcher SHALL push all 8 pixels to the background FIFO
- **AND** each pixel SHALL contain: color index (0-3), palette, and BG priority flag

#### Scenario: Sprite FIFO overlays background

- **WHEN** a sprite pixel is fetched and the sprite FIFO has space
- **AND** the sprite X position matches the current LCD position
- **THEN** sprite pixels SHALL be overlaid onto the OAM FIFO
- **AND** sprite priority SHALL be respected (lower X or lower OAM index wins)
- **AND** transparent sprite pixels (color 0) SHALL NOT replace existing pixels

#### Scenario: Pixel mixing for LCD output

- **WHEN** both FIFOs have at least one pixel available
- **THEN** the PPU SHALL pop one pixel from each FIFO
- **AND** mix them according to priority rules (BG priority bit, sprite priority)
- **AND** output the final pixel to the LCD buffer

### Requirement: Fetcher State Machine

The PPU SHALL implement a 5-step fetcher state machine that retrieves tile data from VRAM:

1. Get Tile (2 T-cycles) - Read tile index from tilemap
2. Get Tile Data Low (2 T-cycles) - Read low byte of tile data
3. Get Tile Data High (2 T-cycles) - Read high byte of tile data
4. Sleep (optional, 2 T-cycles) - Idle cycle for timing alignment
5. Push - Attempt to push 8 pixels to FIFO (repeats until FIFO has space)

#### Scenario: Fetcher advances through states

- **WHEN** the PPU is in Mode 3 (Drawing)
- **THEN** the fetcher SHALL advance one state every 2 T-cycles
- **AND** VRAM accesses SHALL occur on Get Tile, Get Tile Data Low, and Get Tile Data High steps
- **AND** the Push state SHALL repeat until the background FIFO size is ≤ 0

#### Scenario: Fetcher handles window activation

- **WHEN** the window becomes active mid-scanline (WX-7 == LCD X position)
- **AND** WY has been reached on a previous or current scanline
- **THEN** the background FIFO SHALL be cleared
- **AND** the fetcher SHALL restart from Get Tile state
- **AND** the fetcher SHALL switch to window tilemap and coordinates
- **AND** a 6-dot penalty SHALL be incurred

### Requirement: Mode 3 Variable Duration

Mode 3 (Drawing) duration SHALL vary based on rendering conditions, ranging from 172 to 289 T-cycles:

- Base duration: 172 T-cycles (no penalties)
- SCX penalty: +(SCX % 8) T-cycles for scroll discard
- Window penalty: +6 T-cycles when window activates
- Sprite penalty: +6 to +11 T-cycles per sprite

#### Scenario: SCX scroll discard penalty

- **WHEN** SCX is not a multiple of 8
- **THEN** the first (SCX % 8) pixels from the FIFO SHALL be discarded
- **AND** Mode 3 duration SHALL increase by (SCX % 8) T-cycles

#### Scenario: Sprite timing penalty calculation

- **WHEN** a sprite is visible on the current scanline
- **AND** the sprite's X position triggers sprite fetching
- **THEN** the sprite penalty SHALL be calculated as: `11 - min((sprite_x + (SCX % 8)) % 8, 5)`
- **AND** this penalty SHALL be added to Mode 3 duration
- **AND** the background fetcher SHALL be paused during sprite fetch

#### Scenario: Multiple sprites accumulate penalties

- **WHEN** multiple sprites are visible on the same scanline
- **THEN** each sprite SHALL contribute its own penalty to Mode 3 duration
- **AND** sprites at the same X position SHALL share penalty (only one fetch)
- **AND** the maximum 10 sprites per line limit SHALL be enforced

### Requirement: OAM Scan (Mode 2)

During Mode 2 (OAM Scan), the PPU SHALL search OAM for sprites visible on the current scanline.

#### Scenario: Sprite collection during OAM scan

- **WHEN** Mode 2 begins (first 80 T-cycles of a visible scanline)
- **THEN** the PPU SHALL scan all 40 OAM entries
- **AND** collect up to 10 sprites where: `sprite_y <= LY + 16 < sprite_y + sprite_height`
- **AND** sprite_height SHALL be 8 or 16 based on LCDC bit 2

#### Scenario: DMG sprite priority during collection

- **WHEN** running in DMG mode
- **AND** more than one sprite could occupy the same X position
- **THEN** sprites SHALL be prioritized by X position (lower X = higher priority)
- **AND** ties SHALL be broken by OAM index (lower index = higher priority)

#### Scenario: CGB sprite priority during collection

- **WHEN** running in CGB mode
- **AND** OPRI register is 0 (OAM priority mode)
- **THEN** sprites SHALL be prioritized by OAM index only
- **AND** X position SHALL NOT affect priority

### Requirement: Memory Access Blocking

The PPU SHALL block CPU access to OAM and VRAM during appropriate rendering phases.

#### Scenario: OAM blocked during Mode 2

- **WHEN** the PPU is in Mode 2 (OAM Scan)
- **THEN** CPU reads from OAM ($FE00-$FE9F) SHALL return $FF
- **AND** CPU writes to OAM SHALL be ignored

#### Scenario: OAM and VRAM blocked during Mode 3

- **WHEN** the PPU is in Mode 3 (Drawing)
- **THEN** CPU reads from OAM ($FE00-$FE9F) SHALL return $FF
- **AND** CPU reads from VRAM ($8000-$9FFF) SHALL return $FF
- **AND** CPU writes to OAM and VRAM SHALL be ignored

#### Scenario: Memory accessible during Mode 0 and Mode 1

- **WHEN** the PPU is in Mode 0 (HBlank) or Mode 1 (VBlank)
- **THEN** CPU reads from OAM and VRAM SHALL return actual values
- **AND** CPU writes to OAM and VRAM SHALL succeed

### Requirement: LCD Enable Timing

When the LCD is enabled via LCDC bit 7, the PPU SHALL follow specific timing for the first frame.

#### Scenario: First frame after LCD enable

- **WHEN** LCDC bit 7 transitions from 0 to 1
- **THEN** LY SHALL be set to 0
- **AND** the first scanline SHALL start in Mode 0 (not Mode 2)
- **AND** Mode 3 SHALL begin after approximately 76 T-cycles
- **AND** the first frame SHALL be blank (not rendered to display)

#### Scenario: LY=LYC comparison after LCD enable

- **WHEN** the LCD is enabled
- **AND** LYC equals 0
- **THEN** the LY=LYC coincidence flag SHALL be set
- **AND** a STAT interrupt SHALL fire if LYC interrupt is enabled

### Requirement: Mooneye PPU Timing Test Compliance

The emulator SHALL pass all Mooneye PPU timing tests that verify cycle-accurate behavior.

#### Scenario: intr_2_mode0_timing test

- **WHEN** running the `mooneye-test-suite/acceptance/ppu/intr_2_mode0_timing` test
- **THEN** Mode 0 interrupts SHALL fire at the exact T-cycle expected
- **AND** the test result SHALL be PASS

#### Scenario: intr_2_mode0_timing_sprites test

- **WHEN** running the `mooneye-test-suite/acceptance/ppu/intr_2_mode0_timing_sprites` test
- **THEN** Mode 0 timing SHALL correctly account for sprite penalties
- **AND** the test result SHALL be PASS

#### Scenario: intr_2_mode3_timing test

- **WHEN** running the `mooneye-test-suite/acceptance/ppu/intr_2_mode3_timing` test
- **THEN** Mode 3 duration SHALL match hardware behavior
- **AND** the test result SHALL be PASS

#### Scenario: intr_2_oam_ok_timing test

- **WHEN** running the `mooneye-test-suite/acceptance/ppu/intr_2_oam_ok_timing` test
- **THEN** OAM SHALL become accessible at the exact expected T-cycle
- **AND** the test result SHALL be PASS

#### Scenario: lcdon_timing_gs test

- **WHEN** running the `mooneye-test-suite/acceptance/ppu/lcdon_timing-GS` test
- **THEN** LCD enable timing SHALL match DMG/MGB/SGB hardware
- **AND** the test result SHALL be PASS

#### Scenario: lcdon_write_timing_gs test

- **WHEN** running the `mooneye-test-suite/acceptance/ppu/lcdon_write_timing-GS` test
- **THEN** register writes after LCD enable SHALL have correct timing
- **AND** the test result SHALL be PASS

#### Scenario: hblank_ly_scx_timing_gs test

- **WHEN** running the `mooneye-test-suite/acceptance/ppu/hblank_ly_scx_timing-GS` test
- **THEN** LY increment timing SHALL correctly depend on SCX value
- **AND** the test result SHALL be PASS
