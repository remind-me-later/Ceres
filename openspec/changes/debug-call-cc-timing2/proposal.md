# Change: Debug Mooneye test_mooneye_call_cc_timing2 failure

## Why

The Mooneye tests `test_mooneye_call_cc_timing2` and `test_mooneye_call_timing2` are currently failing with all CPU
registers containing 0x42 (Mooneye failure code). These tests use OAM DMA to verify exact M-cycle timing of CALL
instructions by detecting when memory operations occur relative to DMA transfers.

Investigation revealed that the basic CALL instruction timing is **correct** (as proven by passing `call_timing` and
`call_cc_timing` tests), but the timing2 tests fail because they rely on precise OAM DMA behavior that doesn't match
hardware. The tests set SP to OAM memory and use DMA writes as timing sensors to detect the exact M-cycle when PUSH
operations occur.

## Root Cause Analysis

### Test Methodology

The timing2 tests use a clever technique:

1. Start OAM DMA from $8000 to $FE00-$FE9F (160 bytes, 1 byte per M-cycle)
2. Set SP to OAM+$20 ($FE20) where CALL will push return address
3. Carefully time the CALL to execute while DMA is writing to specific addresses
4. Pop the pushed values to check which bytes were corrupted by DMA vs written by CPU

From test source (call_cc_timing2.s):

```text
; CALL cc, nn is expected to have the following timing:
; M = 0: instruction decoding
; M = 1: nn read: memory access for low byte
; M = 2: nn read: memory access for high byte
; M = 3: internal delay
; M = 4: PC push: memory access for high byte
; M = 5: PC push: memory access for low byte
```

The test verifies:

- Round 1 (nops 1): OAM accessible at M=6 → both bytes corrupted by DMA
- Round 2 (nops 2): OAM accessible at M=5 → high byte corrupted, low byte correct
- Round 3 (nops 3): OAM accessible at M=4 → both bytes correct

### Investigation Findings

1. **CPU Timing Verified Correct**:

   - Implemented proper M-cycle timing for CALL: M=1,2 (read nn), M=3 (internal delay), M=4,5 (push PC)
   - Result: `call_timing` and `call_cc_timing` tests PASS

2. **OAM DMA Implementation Issue**:

   - Current implementation uses `remaining_dots` to track DMA progress.
   - `run_dma` consumes dots and transfers bytes.
   - **Issue 1: Blocking Duration**: `is_active` depends on `remaining_dots > 0`. At the end of an M-cycle,
     `remaining_dots` becomes 0, causing `is_active` to be false during the subsequent CPU memory access in the same
     M-cycle. This allows CPU to access OAM when it should be blocked.
   - **Issue 2: End Delay**: SameBoy implementation shows DMA remains active for 1 extra M-cycle after the last byte
     transfer (state 0xA0). `ceres-core` disables DMA immediately after the last byte.
   - **Issue 3: State Tracking**: The current `Dma` struct lacks explicit state tracking (Startup, Transfer, End),
     making it hard to implement precise timing and blocking behavior.

3. **SameBoy Comparison**:
   - SameBoy uses a state machine based on `dma_current_dest` (0-159 for transfer, 160 for end delay).
   - DMA blocks OAM access whenever `dma_current_dest != 0xA1` (Inactive).
   - DMA transfers 1 byte per M-cycle (4 T-cycles).

## Proposed Changes

### 1. Refactor `Dma` Struct

Introduce a state machine to track DMA progress explicitly, similar to SameBoy.

```rust
enum DmaState {
    Inactive,
    Starting(i32), // Startup delay (dots)
    Transferring(u16), // Current offset (0-159)
    Finishing, // Extra cycle after transfer
}

pub struct Dma {
    state: DmaState,
    src_base: u16,
    reg: u8,
    // ...
}
```

### 2. Implement Cycle-Accurate State Machine

Update `run_dma` (or `advance`) to step the state machine based on cycles/dots.

- **Startup**: Wait for 2 M-cycles (8 dots).
- **Transfer**: Transfer 1 byte every M-cycle (4 dots). Increment offset.
- **Finishing**: Wait for 1 extra M-cycle (4 dots) after offset 159.
- **Inactive**: State becomes Inactive after Finishing.

### 3. Fix Blocking Logic

Update `is_active` (or `is_enabled`) to return `true` for all states except `Inactive`. This ensures OAM is blocked
during startup, transfer, and the end delay.

```rust
impl Dma {
    pub fn is_active(&self) -> bool {
        !matches!(self.state, DmaState::Inactive)
    }
}
```

### 4. Update `run_dma` Usage

Ensure `run_dma` is called correctly in `advance_dots_no_timers` to advance the state machine. The logic should handle
`dots` increments and transition states accordingly.

## Plan

1. **Modify `ceres-core/src/memory/dma.rs`**:

   - Define `DmaState` enum.
   - Update `Dma` struct to use `DmaState`.
   - Rewrite `write` (start DMA) to initialize `Starting` state.
   - Rewrite `run_dma` (or `advance`) to handle state transitions and byte transfers.
   - Update `is_active` / `is_enabled`.

2. **Verify OAM Blocking**:

   - Ensure `ceres-core/src/ppu/oam.rs` uses the updated `is_active` method to block CPU writes.

3. **Test**:
   - Run `test_mooneye_call_cc_timing2` and `test_mooneye_call_timing2`.
   - Verify other DMA tests still pass.
   - **Regression Testing**: Ensure all currently passing integration tests (42 Mooneye tests, Blargg tests, etc.)
     continue to pass.

## Impact

- **Affected code**:
  - `ceres-core/src/memory/dma.rs`: Major refactor.
  - `ceres-core/src/ppu/oam.rs`: Minor update to usage if needed.
- **Risk**: Medium. Changing DMA timing can affect many games. Regression testing with other ROMs is recommended.
