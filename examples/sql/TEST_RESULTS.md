# SQL Query Test Results

Tested on: `test_chrome_trace_export_1763417337.json` (137MB, 100 frames) Test ROM:
blargg/cpu_instrs/individual/01-special.gb

## Query Status

| Query                     | Status      | Notes                               |
| ------------------------- | ----------- | ----------------------------------- |
| tight_loops.sql           | ✅ Working  | Successfully identifies tight loops |
| instruction_hotspots.sql  | ⚠️ Partial  | Works but cycles data is NULL       |
| ppu_mode_timeline.sql     | ✅ Working  | Shows PPU mode transitions          |
| frame_timing.sql          | ✅ Working  | Analyzes frame timing               |
| memory_hotspots.sql       | ✅ Working  | Shows VRAM access patterns          |
| dma_transfers.sql         | ⚠️ Untested | No DMA in test ROM                  |
| register_changes.sql      | ⚠️ Complex  | Very slow on large traces           |
| execution_fingerprint.sql | ⚠️ Untested | Needs comparison workflow           |

## Sample Results

### Tight Loops (tight_loops.sql)

Found instruction at PC 537 ("JR Z, $FC") executed 255,812 times over 1.7 seconds - clear busy loop.

### Instruction Hotspots (instruction_hotspots.sql)

- PC 537: "JR Z, $FC" - 76% of all instructions (255,812 executions)
- PC 516: "LD [HL+], A" - 6% of instructions (20,480 executions)

### PPU Mode Timeline (ppu_mode_timeline.sql)

Shows proper PPU state machine:

- Scanline 0-143: OAM Scan → Drawing → HBlank (normal operation)
- Each mode appears ~90-91 times (91 frames in trace)

### Frame Timing (frame_timing.sql)

Frame rate: 50-58 FPS (should be 59.73 Hz) Status: Most frames marked "Slow" (17-19ms vs 16.74ms target) Issue: Trace
overhead or ROM waiting in loops

### Memory Hotspots (memory_hotspots.sql)

VRAM addresses $98C2-$9903 accessed 19 times each Pattern: Background tilemap updates (sequential addresses)

## Known Issues

1. **Cycles data missing**: CPU execution events don't store cycle counts
2. **Register tracking slow**: Full register change tracking on 137MB trace is very slow
3. **No DMA events**: Test ROM doesn't use DMA, so dma_transfers.sql untested

## Recommendations

1. Queries work well for moderate trace sizes (<200MB)
2. Use time-based filtering in Perfetto UI for large traces
3. Focus on hotspot queries first for quick insights
4. Full register tracking best used on small, targeted traces
