# Design: Debug and Fix Ignored Mooneye PPU Tests

## Context

Ceres implements a PPU (Pixel Processing Unit) that manages display modes, STAT interrupts, and screen rendering. The
current implementation uses a simplified state machine that doesn't fully match hardware-verified behaviors, causing 10
Mooneye PPU tests to fail.

### Stakeholders

- Emulator users expecting accurate game behavior
- Developers maintaining PPU accuracy

### Constraints

- Must remain `no_std` compatible
- Must not break existing passing tests
- SameBoy is the reference implementation for correct behavior

## Goals / Non-Goals

### Goals

- Pass all 12 Mooneye PPU tests (currently 2/12 passing)
- Implement accurate STAT interrupt edge-detection
- Implement accurate LCD enable timing behavior
- Implement accurate mode timing affected by sprites

### Non-Goals

- CGB-specific PPU differences (focus on DMG/SGB first)
- Performance optimization (accuracy first)
- PPU rendering accuracy (separate concern from timing)

## Decisions

### Decision 1: Implement STAT Interrupt Line Tracking

**What:** Add boolean fields to track the internal STAT interrupt line state.

**Why:** SameBoy implements edge-detection by tracking `stat_interrupt_line` and `previous_interrupt_line`. Interrupts
only fire on rising edges. This is required for `stat_irq_blocking` test.

**Alternative considered:** Tracking individual interrupt sources - rejected as too complex and doesn't match hardware
behavior where a single internal line is OR'd from multiple sources.

**Implementation:**

```rust
pub struct Ppu {
    // ... existing fields ...

    /// Internal STAT interrupt line (OR of all enabled STAT sources)
    stat_interrupt_line: bool,
}

fn update_stat_interrupt(&mut self, ints: &mut Interrupts) {
    let previous_line = self.stat_interrupt_line;

    // Compute new line state from all sources
    let mut new_line = false;

    // LY=LYC coincidence
    if (self.stat & STAT_IF_LYC_B != 0) && (self.ly_for_comparison == self.lyc) {
        new_line = true;
    }

    // Mode-based interrupts
    match self.mode() {
        Mode::HBlank if self.stat & STAT_IF_HBLANK_B != 0 => new_line = true,
        Mode::VBlank if self.stat & STAT_IF_VBLANK_B != 0 => new_line = true,
        Mode::OamScan if self.stat & STAT_IF_OAM_B != 0 => new_line = true,
        _ => {}
    }

    self.stat_interrupt_line = new_line;

    // Only fire on rising edge
    if new_line && !previous_line {
        ints.request_lcd();
    }
}
```

### Decision 2: Separate LY for Comparison vs Display

**What:** Add `ly_for_comparison: u8` field separate from `ly`.

**Why:** The LY value used for LYC comparison has different timing than the displayed LY value. When LCD is off,
`ly_for_comparison` is set to 0 but the coincidence flag may be retained.

**Implementation:**

```rust
pub struct Ppu {
    ly: u8,                    // Displayed LY value
    ly_for_comparison: u8,     // LY used for LYC comparison (may differ during mode transitions)
}
```

### Decision 3: LCD Enable First-Line Special Handling

**What:** When LCD is enabled, line 0 starts in Mode 0 (not Mode 2) with special timing.

**Why:** The Mooneye `lcdon_timing-GS` test verifies this behavior:

- Line 0 starts in Mode 0, goes straight to Mode 3
- PPU is "late" by 2 T-cycles on first line
- Lines 1+ have normal Mode 2 -> Mode 3 -> Mode 0 progression

**Implementation:**

```rust
pub fn write_lcdc(&mut self, val: u8, ints: &mut Interrupts) {
    // turn on
    if val & LCDC_ON_B != 0 && self.lcdc & LCDC_ON_B == 0 {
        self.ly = 0;
        self.ly_for_comparison = 0;

        // Line 0 starts in Mode 0, not Mode 2
        self.set_mode_stat(Mode::HBlank);

        // First line is "late" by 2 T-cycles - goes straight to Mode 3
        self.first_line_after_enable = true;
        self.enable_timer = 2;  // 2 T-cycle delay before Mode 3

        // Update coincidence (may trigger interrupt if LYC=0)
        self.update_stat_interrupt(ints);
    }
    // ...
}
```

### Decision 4: Mode 3 Duration Based on Sprite X Coordinates

**What:** Mode 3 duration varies based on sprite positions on the current line.

**Why:** The `intr_2_mode0_timing_sprites` test verifies this behavior:

- Each sprite adds extra cycles to Mode 3
- Sprite X coordinate affects how many extra cycles are added
- 10 sprites at X=0 adds 16 cycles; 10 sprites at X=168 adds 0 cycles

**Implementation approach:**

- During OAM scan, collect sprite X coordinates for current line
- Calculate Mode 3 duration based on sprite positions
- Use SameBoy's algorithm as reference

### Decision 5: VBlank Mode 2 STAT Interrupt at Line 144

**What:** When STAT bit 5 (Mode 2 interrupt) is enabled, also fire STAT interrupt at line 144.

**Why:** The `vblank_stat_intr-GS` test verifies that the Mode 2 OAM interrupt also triggers at VBlank start.

**Implementation:**

```rust
fn enter_mode(&mut self, mode: Mode, ints: &mut Interrupts) {
    // ... existing code ...

    match mode {
        Mode::VBlank => {
            ints.request_vblank();

            if self.stat & STAT_IF_VBLANK_B != 0 {
                ints.request_lcd();
            }

            // Mode 2 interrupt also fires at line 144
            if self.stat & STAT_IF_OAM_B != 0 {
                // This is handled by the STAT line update
                // since the internal line will go high
            }
        }
        // ...
    }
}
```

## Risks / Trade-offs

### Risk 1: Breaking Existing Games

**Risk:** More accurate PPU timing might break games that worked with the old inaccurate timing.

**Mitigation:** Run full test suite before and after changes. Test popular games that had issues.

### Risk 2: Performance Impact

**Risk:** Additional state tracking and per-cycle checks may slow emulation.

**Mitigation:** Profile after implementation. If needed, optimize hot paths. Accuracy is priority.

### Risk 3: CGB Differences

**Risk:** Some behaviors differ between DMG and CGB. Tests marked with `-GS` only pass on DMG/SGB.

**Mitigation:** Focus on DMG-compatible behavior first. Note CGB differences for future work.

## Migration Plan

This is a bug fix with no user-facing API changes. No migration needed.

## Open Questions

1. **Sprite penalty calculation:** What is the exact algorithm for calculating Mode 3 extension based on sprite X
   coordinates? Need to extract from SameBoy or Pan Docs.

2. **CGB differences:** How do CGB PPU timings differ? Tests like `lcdon_timing-GS` explicitly fail on CGB. Should we
   branch on model?

3. **Double-speed mode:** Does CGB double-speed mode affect these timings? Need to verify.

## References

- [SameBoy display.c](https://github.com/LIJI32/SameBoy/blob/master/Core/display.c)
- [Pan Docs - STAT Interrupts](https://gbdev.io/pandocs/Interrupt_Sources.html#stat-interrupt)
- [Pan Docs - LCD Position and Scrolling](https://gbdev.io/pandocs/Scrolling.html)
- [Mooneye Test Suite](https://github.com/Gekkio/mooneye-test-suite)
