# Trace Analysis Results: call_cc_timing2 DMA Timing Issue

## Critical Finding

**The DMA startup delay is 3.1x too long!**

- **Expected**: 8 dots (2 M-cycles)
- **Actual**: 24.89 dots (6.22 M-cycles)
- **Error**: +16.89 dots (+4.22 M-cycles)

## Trace Evidence

From `debug_call_cc_timing2_minimal_1763832368.json`:

```
DMA Start Event:    427834.539 μs  (OAM DMA started, delay_dots=8, src_base=$8000)
First Transfer:     427840.473 μs  (OAM DMA transfer byte, oam_offset=$00, src=$8000, val=$81)
Measured Delay:     5.934 μs = 24.89 dots
```

## Transfer Rate Analysis

The transfer rate between bytes is also inconsistent:

| Transfer Interval  | Time (μs) | Dots  | Expected |
| ------------------ | --------- | ----- | -------- |
| Start → Transfer 0 | 5.934     | 24.89 | 8.0      |
| Transfer 0 → 1     | 2.657     | 11.14 | 4.0      |
| Transfer 1 → 2     | 2.236     | 9.38  | 4.0      |
| Transfer 2 → 3     | 1.813     | 7.60  | 4.0      |
| Transfer 3 → 4     | 1.986     | 8.33  | 4.0      |

The startup is ~3x too long, and subsequent transfers vary between 6-11 dots instead of exactly 4 dots.

## Root Cause Hypothesis

### Theory 1: `run_dma()` not called immediately after write

When CPU writes to FF46:

1. `dma.write(val)` is called → Sets `Starting(8)` state
2. BUT `run_dma()` is only called during next `advance_dots_no_timers()`
3. If CPU continues executing for several M-cycles before the next `advance_dots()`, the DMA appears to have a longer
   startup delay

**Code flow**:

```rust
// In sm83.rs - CPU execution
fn write_cpu(&mut self, addr: u16, val: u8) {
    self.tick_m_cycle();  // ← This calls advance_dots(4)
    self.write_mem(addr, val);  // ← FF46 write happens HERE
}
// After write_mem returns, DMA state is Starting(8)
// But run_dma() won't be called until NEXT advance_dots()!
```

### Theory 2: Accumulator management issue

The `advance_dots()` adds to accumulator, but `step()` only processes when `accumulator >= 4`:

```rust
pub fn step(&mut self) -> Option<(u16, u8)> {
    while self.accumulator >= 4 {
        self.accumulator -= 4;
        // Process state machine
    }
}
```

If `advance_dots()` is called with varying amounts (e.g., from different M-cycle operations), the accumulator might
build up before being processed.

### Theory 3: Trace event timing

The trace events might be recorded at the wrong time relative to when the actual transfer occurs in memory. However,
this is less likely since the pattern is consistent.

## Recommended Fixes

### Fix 1: Call `run_dma()` immediately after FF46 write (Preferred)

Modify the DMA write handler to immediately process one step:

```rust
pub fn write(&mut self, val: u8) {
    self.reg = val;
    self.base_addr = u16::from(val) << 8;
    self.state = DmaState::Starting(8);
    self.accumulator = 0;

    tracing::trace!(/* ... */);
}
```

Then in the memory write handler:

```rust
DMA => {
    self.dma.write(val);
    self.run_dma();  // ← Process immediately
}
```

### Fix 2: Adjust for CPU write timing

The CPU write that triggers DMA happens DURING a tick_m_cycle(). We need to account for where in that M-cycle the write
occurs:

```rust
// When DMA is written, we're already partway through an M-cycle
// Reduce startup delay accordingly
pub fn write(&mut self, val: u8) {
    // ...
    self.state = DmaState::Starting(4);  // ← Reduced from 8
    // ...
}
```

### Fix 3: Ensure deterministic advance_dots calls

Make sure `advance_dots()` is called at consistent intervals and `run_dma()` processes immediately after.

## Next Steps

1. Add detailed logging to track when `advance_dots()` and `run_dma()` are called relative to FF46 write
2. Verify CPU write timing - does `tick_m_cycle()` call `advance_dots()` before or after `write_mem()`?
3. Test Fix 1 by calling `run_dma()` immediately after `dma.write()`
4. Re-run trace to verify timing

## Expected Impact

If fixed correctly:

- `call_timing` and `call_cc_timing` should still pass (currently passing)
- `call_timing2` and `call_cc_timing2` should now pass (currently failing)
- DMA startup delay should measure exactly 8 dots in traces
- Transfer intervals should measure exactly 4 dots each
