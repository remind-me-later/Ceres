## Context

The Mooneye `oam_dma_start` test is failing because Ceres incorrectly implements the timing of when OAM becomes
inaccessible after a DMA transfer is initiated. This is a critical hardware behavior that affects games and demos that
execute code from OAM or manipulate DMA transfers.

### Constraints

- Must maintain compatibility with other passing DMA tests (`oam_dma_restart`, `oam_dma_timing`, `oam_dma/basic`, etc.)
- Should minimize performance impact (DMA is checked on every OAM access)
- Must align with SameBoy's behavior where documented, but recognize test expectations may differ slightly
- Changes to DMA state machine must not affect HDMA (CGB-specific feature)

### Stakeholders

- Test suite maintainers (ensuring Mooneye tests pass)
- Game compatibility (some games may rely on precise DMA timing)
- Performance (DMA checks are on hot path)

## Goals / Non-Goals

### Goals

- Pass the `mooneye-test-suite/acceptance/oam_dma_start.gb` test
- Correctly implement the 1 M-cycle window where OAM remains accessible after DMA write
- Maintain or improve clarity of DMA state machine code
- Document the timing behavior clearly in code comments

### Non-Goals

- Fixing other unrelated DMA behaviors (e.g., source address limitations, mode 2/3 access)
- Optimizing DMA performance beyond maintaining current speed
- Implementing edge cases not covered by test suite (defer to future work)
- Modifying HDMA (CGB-specific DMA) behavior

## Decisions

### Decision 1: Separate "DMA Active" from "OAM Blocked"

**What**: Introduce a distinction between "DMA is active" (state != Inactive) and "OAM is blocked" (state is
transferring or finishing).

**Why**: The current `is_enabled()` method returns true for all non-Inactive states, including `Starting`. This causes
OAM to be blocked immediately, but the test expects a 1 M-cycle delay.

**Implementation**: Add a new method `blocks_oam()` that returns false during the first 4 dots (1 M-cycle) of the
`Starting` state.

**Alternatives considered**:

- **Alternative A**: Split `Starting` into `Starting1` and `Starting2` states - more complex, harder to maintain
- **Alternative B**: Use a separate boolean flag - adds unnecessary state duplication
- **Alternative C**: Modify `is_enabled()` semantics - breaks existing code that checks if DMA is active

### Decision 2: Track Restart Behavior in DMA State

**What**: When a DMA is restarted (write to $FF46 while DMA active), the `Starting` state should account for whether OAM
was already blocked.

**Why**: The test's second round expects OAM to remain inaccessible throughout a restart.

**Implementation**: In the `Starting` state, if transitioning from an already-active DMA, set the delay to ensure
immediate OAM blocking. The `write()` method should check current state.

**Alternatives considered**:

- **Alternative A**: Add a `restarting` boolean flag - adds state complexity
- **Alternative B**: Use different startup delays (4 vs 8 dots) - cleaner, chosen approach

### Decision 3: Align Startup Delay with Test Expectations

**What**: The startup delay should be 8 dots (2 M-cycles), but OAM blocking should start at 4 dots (1 M-cycle).

**Why**: This matches the test's expectations:

- M=0: Write happens
- M=1: OAM accessible (first 4 dots of Starting)
- M=2: OAM inaccessible, transfer begins (second 4 dots + Transferring)

**Implementation**: `blocks_oam()` returns `dots_until_start <= 4` for `Starting(dots_until_start)`.

**Alternatives considered**:

- **Alternative A**: Change startup delay to 4 dots total - would break timing of actual transfer start
- **Alternative B**: Complex cycle tracking - over-engineered for this specific issue

## Technical Approach

### Current Implementation

```rust
// dma.rs
enum DmaState {
    Inactive,
    Starting(u8),     // Startup delay in dots
    Transferring(u8), // Offset being transferred
    Finishing,        // Post-transfer cleanup
}

pub const fn is_enabled(&self) -> bool {
    !matches!(self.state, DmaState::Inactive)
}

pub fn write(&mut self, val: u8) {
    // ...
    self.state = DmaState::Starting(8); // 2 M-cycles
}
```

```rust
// oam.rs
pub const fn read_oam(&self, addr: u16, dma_on: bool) -> u8 {
    if dma_on {  // Blocks immediately
        return 0xFF;
    }
    // ...
}
```

### Proposed Implementation

```rust
// dma.rs
pub const fn blocks_oam(&self) -> bool {
    match self.state {
        DmaState::Inactive => false,
        DmaState::Starting(dots) => dots <= 4, // Block in 2nd M-cycle
        DmaState::Transferring(_) | DmaState::Finishing => true,
    }
}

pub fn write(&mut self, val: u8) {
    // ...
    let was_active = !matches!(self.state, DmaState::Inactive);
    self.state = if was_active {
        DmaState::Starting(8) // Restart: OAM already blocked
    } else {
        DmaState::Starting(8) // Fresh start: 1 M-cycle before blocking
    };
}
```

```rust
// oam.rs
pub const fn read_oam(&self, addr: u16, dma_blocks: bool) -> u8 {
    if dma_blocks {  // Use blocks_oam() instead of is_enabled()
        return 0xFF;
    }
    // ...
}
```

**Wait, this doesn't work**: If both fresh and restart use `Starting(8)`, `blocks_oam()` will return the same value for
both. We need different delays.

### Revised Approach

Actually, re-reading the test comments more carefully:

```asm
; Expected timing (fresh DMA):
; M = 0: write to $FF46 happens
; M = 1: nothing (OAM still accessible)
; M = 2: new DMA starts, OAM reads will return $FF

; Expected timing (restarted DMA):
; M = 0: write to $FF46 happens. Previous DMA is running (OAM *not* accessible)
; M = 1: previous DMA is running (OAM *not* accessible)
; M = 2: new DMA starts, OAM reads will return $FF
```

The key insight: in a restart, OAM is **already** inaccessible because the previous DMA is running. So the behavior
difference is automatic if we check the state properly.

**Correct implementation**:

```rust
pub fn write(&mut self, val: u8) {
    // Don't need to check previous state - the Starting delay handles timing
    self.state = DmaState::Starting(8);
}

pub const fn blocks_oam(&self) -> bool {
    match self.state {
        DmaState::Inactive => false,
        DmaState::Starting(dots) if dots > 4 => false, // First M-cycle
        DmaState::Starting(_) => true,                 // Second M-cycle
        DmaState::Transferring(_) | DmaState::Finishing => true,
    }
}
```

This works because:

- Fresh DMA: Inactive → Starting(8), OAM accessible for first 4 dots
- Restart: Transferring → Starting(8), but OAM was already blocked, stays blocked

Wait, that's still wrong. If we're in `Transferring` and write DMA, we'll be in `Starting(8)` which will return
`blocks_oam() = false` for the first 4 dots, making OAM accessible during a restart!

**Actually correct implementation**:

We need to track whether the previous state was active:

```rust
pub fn write(&mut self, val: u8) {
    self.reg = val;
    self.base_addr = u16::from(val) << 8;

    // If restarting, skip the accessible window
    let delay = if matches!(self.state, DmaState::Inactive) {
        8  // Fresh: 1 M-cycle accessible, 1 M-cycle delay
    } else {
        4  // Restart: already blocked, go straight to transfer
    };

    self.state = DmaState::Starting(delay);
    self.accumulator = 0;
}

pub const fn blocks_oam(&self) -> bool {
    match self.state {
        DmaState::Inactive => false,
        DmaState::Starting(dots) if dots > 4 => false, // Only in fresh start
        DmaState::Starting(_) => true,
        DmaState::Transferring(_) | DmaState::Finishing => true,
    }
}
```

Now:

- Fresh DMA: Starting(8) → blocks_oam() false for 4 dots, then true
- Restart: Starting(4) → blocks_oam() true immediately

## Risks / Trade-offs

### Risk: Breaking Other DMA Tests

**Mitigation**: Run full test suite, especially `oam_dma_restart`, `oam_dma_timing`, and `oam_dma/basic`.

### Risk: Performance Impact of New Method

**Mitigation**: New method is const and inline, should compile to same code as `is_enabled()`.

### Risk: Misunderstanding Test Expectations

**Mitigation**: Cross-reference with SameBoy implementation and Pan Docs. Run test with detailed tracing.

### Trade-off: Code Complexity vs Correctness

Adding `blocks_oam()` increases API surface. However, the distinction between "DMA active" and "OAM blocked" is a real
hardware behavior, so the added complexity reflects real hardware complexity.

## Migration Plan

### Implementation Steps

1. Add `blocks_oam()` method to DMA struct
2. Update `write()` to use different startup delays for fresh vs restart
3. Change `read_oam()` and `write_oam()` to use `blocks_oam()` instead of `is_enabled()`
4. Add tracing to verify timing behavior
5. Test with oam_dma_start and other DMA tests

### Rollback

If issues arise, revert to using `is_enabled()` everywhere. The change is isolated to DMA module.

### Testing Strategy

- Unit test the `blocks_oam()` method with different states
- Integration test with Mooneye suite
- Regression test with blargg and gbmicro

## Open Questions

1. **Q**: Does this match SameBoy's implementation exactly?  
   **A**: SameBoy blocks OAM immediately. The test may expect slightly different behavior, or there's a subtlety in how
   the state is checked relative to instruction execution. Defer to test expectations.

2. **Q**: Should `is_enabled()` be renamed to avoid confusion?  
   **A**: Keep `is_enabled()` for now (may be used for other purposes like preventing concurrent DMAs). Add clear
   documentation.

3. **Q**: What happens if code jumps to OAM during the accessible window?  
   **A**: The CPU should be able to fetch instructions from OAM during M=1. This is what the test validates. Ceres's
   instruction fetch goes through the same `read_mem()` path, so this should work automatically.

4. **Q**: Does this affect save state compatibility?  
   **A**: DMA state is already serialized. The enum structure doesn't change, only the timing logic. No compatibility
   impact expected.
