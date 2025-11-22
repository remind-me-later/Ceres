# Trace Analysis: call_cc_timing2 Failure

## Generated Traces

Two trace files have been generated for debugging the `call_cc_timing2` test failure:

### 1. Minimal Trace (DMA-only)

- **File**: `ceres-test-runner/target/traces/debug_call_cc_timing2_minimal_1763832368.json`
- **Size**: 172 KB
- **Filter**: `dma=trace,oam=trace`
- **Content**: Only DMA transfer events and OAM access events
- **Use for**: Quick overview of DMA timing and OAM blocking behavior

### 2. Detailed Trace (DMA + Memory + CPU)

- **File**: `ceres-test-runner/target/traces/debug_call_cc_timing2_1763832379.json`
- **Size**: 291 MB
- **Filter**: `dma=trace,memory=trace,cpu_execution=info`
- **Content**: DMA events, memory access, and CPU instruction execution
- **Use for**: Detailed analysis of CPU/DMA interaction and timing

## How to Analyze

### Option 1: Perfetto UI (Recommended)

1. Open https://ui.perfetto.dev
2. Drag and drop the trace file
3. Use the timeline view to visualize events
4. Look for:
   - DMA start/end events
   - OAM write attempts during DMA
   - CPU CALL instruction execution
   - PPU mode changes (if available)

### Option 2: Trace Processor CLI

```bash
# Install trace_processor if needed
# Download from: https://perfetto.dev/docs/analysis/trace-processor

# Query DMA events
trace_processor -q "SELECT ts, name, dur FROM slice WHERE cat='dma'" trace.json

# Query OAM writes
trace_processor -q "SELECT ts, name, dur FROM slice WHERE cat='oam' OR name LIKE '%OAM%'" trace.json

# Query CALL instructions
trace_processor -q "SELECT ts, name, dur FROM slice WHERE name LIKE '%CALL%'" trace.json
```

## Key Areas to Investigate

### 1. DMA Startup Timing

**Current Implementation**: 8 dots (2 M-cycles) startup delay **Check**: When does the first DMA transfer occur relative
to the DMA write?

### 2. OAM Blocking Windows

**Current Implementation** (from `ceres-core/src/ppu/oam.rs`):

- **Read blocking**: After first 4 dots of Mode 2 (remaining_dots <= 76)
- **Write blocking**: After first 8 dots of Mode 2 (remaining_dots <= 72)

**Check**:

- Are CPU writes to OAM being blocked at the correct times?
- Does the blocking window align with DMA transfer timing?

### 3. PPU Mode 2 Timing

**Expected**: Mode 2 (OAM Scan) should be 80 dots **Check**:

- Is Mode 2 consistently 80 dots long?
- Are mode transitions happening at the right times relative to DMA?

### 4. CPU/DMA Bus Contention

**Current Implementation**: DMA always wins (CPU blocked when `dma_active`) **Check**:

- When CPU tries to write to OAM during DMA, is it always blocked?
- Should there be partial M-cycle contention?

## Expected vs. Actual Behavior

### Test ROM Behavior (call_cc_timing2.s)

The test uses three rounds with different NOP counts to test timing:

1. **Round 1 (nops 1)**: OAM accessible at M=6 → both bytes corrupted by DMA
2. **Round 2 (nops 2)**: OAM accessible at M=5 → high byte corrupted, low byte correct
3. **Round 3 (nops 3)**: OAM accessible at M=4 → both bytes correct

### CALL cc, nn Timing

```
M = 0: instruction decoding
M = 1: nn read: memory access for low byte
M = 2: nn read: memory access for high byte
M = 3: internal delay
M = 4: PC push: memory access for high byte
M = 5: PC push: memory access for low byte
```

### Current Result

All registers = 0x42 → Test completely fails (all rounds fail)

## Trace Analysis Checklist

- [ ] Find when DMA is started (FF46 write)
- [ ] Measure time from DMA start to first byte transfer
- [ ] Check if startup delay is exactly 8 dots
- [ ] Find CALL instructions in the trace
- [ ] Check timing of PUSH operations (M=4 and M=5)
- [ ] Verify OAM write attempts during PUSH
- [ ] Check if writes are blocked/allowed correctly
- [ ] Compare DMA transfer timing with expected schedule (1 byte per 4 dots)
- [ ] Check PPU mode at the time of critical writes
- [ ] Look for any unexpected blocking/unblocking

## Hypothesis to Test

1. **Startup delay off-by-one**: Maybe startup should be 7 or 9 dots instead of 8?
2. **Blocking window inverted**: Maybe the comparison operators are backwards?
3. **PPU mode tracking**: Maybe `remaining_dots_in_mode` isn't updated correctly?
4. **CGB-specific behavior**: Test runs in CGB mode, might need different rules?

## Next Steps After Analysis

Based on trace findings, consider:

1. Adjusting DMA startup delay
2. Refining OAM blocking window calculations
3. Implementing CGB-specific blocking exceptions
4. Adding sub-M-cycle timing if needed
