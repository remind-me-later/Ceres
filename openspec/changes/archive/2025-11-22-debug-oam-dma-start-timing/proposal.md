# Change: Debug OAM DMA Start Timing

## Why

The `oam_dma_start` test from the Mooneye test suite is currently failing. This test validates the precise timing
behavior of OAM DMA initialization, specifically testing when OAM becomes inaccessible after writing to the DMA register
($FF46).

According to the test ROM source and SameBoy's implementation:

- **M = 0**: Write to $FF46 happens
- **M = 1**: OAM is still accessible (1 M-cycle delay)
- **M = 2**: New DMA starts, OAM reads return $FF (inaccessible)

When a DMA is restarted while a previous one is running:

- **M = 0**: Write to $FF46 happens, previous DMA is running (OAM _not_ accessible)
- **M = 1**: Previous DMA is still running (OAM _not_ accessible)
- **M = 2**: New DMA starts

The test executes code from OAM memory ($FE00-$FE9F) immediately after triggering DMA, verifying that:

1. In the first test round, OAM is accessible for 1 M-cycle after DMA start
2. In the second test round, when DMA is restarted, OAM remains inaccessible throughout

Current test result shows all registers set to `0x42`, indicating both test rounds are failing.

## What Changes

- Document the expected OAM DMA start timing behavior based on Mooneye test and SameBoy
- Identify discrepancies between current Ceres implementation and expected behavior
- Provide implementation guidance for fixing the timing issue

## Impact

- **Affected specs**: `ppu-dma` (new capability)
- **Affected code**:
  - `ceres-core/src/memory/dma.rs` - DMA state machine and timing
  - `ceres-core/src/ppu/oam.rs` - OAM accessibility during DMA (`read_oam`, `write_oam`)
  - `ceres-core/src/memory/mod.rs` - DMA register write handling
- **Affected tests**: `ceres-test-runner/tests/mooneye_tests.rs::test_mooneye_oam_dma_start`

## Current Implementation Analysis

### Ceres DMA Implementation (`dma.rs`)

```rust
DmaState::Starting(8)  // 8 dots = 2 M-cycles startup delay
```

The `Starting` state delays the first byte transfer but doesn't control when OAM becomes inaccessible.

### OAM Access Control (`oam.rs`)

```rust
pub const fn read_oam(&self, addr: u16, dma_on: bool) -> u8 {
    if dma_on {  // dma_on = dma.is_enabled()
        return 0xFF;
    }
    // ... PPU mode checks ...
}
```

**Issue**: `dma.is_enabled()` returns `true` as soon as the DMA register is written, even during the `Starting` state.
This makes OAM immediately inaccessible, not after 1 M-cycle.

### SameBoy Behavior

SameBoy sets `dma_current_dest = 0xFF` immediately on DMA write, making OAM inaccessible right away. However, the
Mooneye test expects a 1 M-cycle window where OAM remains accessible.

**Resolution**: The test likely executes the instruction that wrote to DMA during M=0, and the following instruction(s)
execute at M=1 when OAM should still be accessible. The timing window depends on when the DMA state is checked relative
to instruction execution.

## Key Behavioral Requirements

1. **Fresh DMA Start**: After writing to $FF46, OAM must remain accessible for 1 M-cycle
2. **DMA Restart**: When restarting an active DMA, OAM must remain inaccessible throughout
3. **Timing Precision**: The DMA state machine must distinguish between "initialized but not blocking" vs "active and
   blocking"

## Open Questions

1. Should `is_enabled()` return a different value during the `Starting` state?
2. Do we need a separate state to track "DMA requested but OAM still accessible"?
3. How does this interact with CPU instruction timing and memory access cycles?
4. What happens if OAM is accessed from code executing in OAM itself during the transition?

## References

- Test ROM: `mooneye-test-suite/acceptance/oam_dma_start.s`
- SameBoy implementation: `Core/memory.c` and `Core/dma.c`
- Pan Docs: [OAM DMA Transfer](https://gbdev.io/pandocs/OAM_DMA_Transfer.html)
- Test result: Failing with all registers = `0x42` (expected: `B=$D7, C=$01, D=$D7, E=$00`)
