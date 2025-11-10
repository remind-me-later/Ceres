## Context

The `ld b, b` instruction (opcode 0x40) is used by test ROMs like cgb-acid2 and dmg-acid2 as a debug breakpoint to
signal test completion. The instruction is functionally a NOP but has special meaning in the testing context.

The current architecture has the CPU implementation (`Sm83`) as a separate struct within the `Gb` struct. The CPU
executes instructions through methods in `impl<A: AudioCallback> Gb<A>` that delegate to CPU-specific logic. This means
the CPU execution context already has access to the `Gb` struct's mutable state.

## Goals / Non-Goals

**Goals:**

- Provide a simple, reliable way to detect `ld b, b` execution
- Maintain backward compatibility with existing API
- Preserve `no_std` compatibility
- Use minimal code changes

**Non-Goals:**

- Generic breakpoint system with configurable opcodes
- Callback-based notification system
- Debug stepping or instruction tracing
- Performance profiling or timing analysis

## Decisions

### Decision: Flag stored in Gb struct, not Sm83

**Rationale:** The `Gb` struct is the public interface users interact with. Storing the flag there means the public
method can be added directly to `Gb` without exposing internal CPU state. The CPU execution methods already run in the
context of `&mut Gb<A>`, so setting the flag is straightforward.

**Alternatives considered:**

1. Store flag in `Sm83` - Would require exposing CPU internals through a public getter
2. Use a callback - Adds complexity and complicates `no_std` usage
3. Use serial output detection - Already available but requires parsing, less reliable

### Decision: Check-and-reset pattern

**Rationale:** Automatically resetting the flag when checked prevents users from needing to manually clear it and
ensures they don't miss breakpoint events between checks. The pattern is simple: check returns true once per `ld b, b`
execution.

**Implementation:** The method signature is `pub fn check_and_reset_ld_b_b_breakpoint(&mut self) -> bool`. It reads the
flag value, sets it to false, then returns the read value.

### Decision: Set flag in ld_b_b method

**Rationale:** The `ld_b_b` method in `sm83.rs` currently just calls `self.nop()`. Since this method runs in the context
of `&mut Gb<A>`, we can directly set `self.ld_b_b_breakpoint = true` before calling `self.nop()`.

**Implementation note:** The CPU execution methods are implemented as `impl<A: AudioCallback> Gb<A>`, so `self` refers
to the `Gb` instance, not the `Sm83` instance. This makes accessing the flag trivial.

## Implementation Details

### Files to modify

1. **`ceres-core/src/lib.rs`**:
   - Add `ld_b_b_breakpoint: bool` field to `Gb` struct
   - Initialize to `false` in `new` method (line ~155)
   - Reset to `false` in `soft_reset` method (line ~240)
   - Add public method `check_and_reset_ld_b_b_breakpoint`

2. **`ceres-core/src/sm83.rs`**:
   - Modify `ld_b_b` method (line ~823) to set flag before calling `nop()`

### Code changes

```rust
// In lib.rs, Gb struct (around line 51):
pub struct Gb<A: AudioCallback> {
    // ... existing fields ...
    ld_b_b_breakpoint: bool,
}

// In lib.rs, impl Gb (add new method):
#[inline]
pub fn check_and_reset_ld_b_b_breakpoint(&mut self) -> bool {
    let was_set = self.ld_b_b_breakpoint;
    self.ld_b_b_breakpoint = false;
    was_set
}

// In sm83.rs, ld_b_b method (line ~823):
const fn ld_b_b(&mut self) {
    self.ld_b_b_breakpoint = true;
    self.nop();
}
```

## Risks / Trade-offs

**Risk:** Additional state in `Gb` struct increases memory footprint. **Mitigation:** A single boolean adds only 1 byte.
The struct is already large (contains PPU, APU, etc.), so impact is negligible.

**Risk:** Performance overhead from setting flag on every `ld b, b`. **Mitigation:** Setting a boolean is a single store
instruction with zero computational cost. The instruction is also rare in normal Game Boy programs (only test ROMs use
it intentionally).

**Risk:** Users might forget to check the flag and miss breakpoints. **Mitigation:** Check-and-reset pattern ensures the
flag is automatically cleared. Documentation clearly explains the intended usage pattern.

## Migration Plan

No migration needed - this is a purely additive change. Existing code continues to work unchanged. Users who want
breakpoint detection can opt in by calling the new method.

## Open Questions

None - implementation is straightforward given the existing architecture.
