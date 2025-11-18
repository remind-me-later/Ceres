# Perfetto SQL Queries for Game Boy Debugging

This directory contains SQL queries for analyzing Chrome Trace Event Format traces in
[Perfetto UI](https://ui.perfetto.dev).

## How to Use

1. **Generate a trace file** by running a test with tracing enabled (see `ceres-test-runner/tests/trace_export_test.rs`)
2. **Open Perfetto UI** at https://ui.perfetto.dev
3. **Load your trace file** by dragging it into the browser
4. **Open the SQL query editor** (click the query icon in the left sidebar)
5. **Copy and paste** one of the queries below
6. **Execute** and analyze the results

## Available Queries

### Performance Analysis

- `tight_loops.sql` - Detect tight loops (same PC executing repeatedly)
- `instruction_hotspots.sql` - Find most frequently executed instructions
- `frame_timing.sql` - Analyze frame timing and identify slow frames

### Hardware Debugging

- `ppu_mode_timeline.sql` - Visualize PPU mode transitions per scanline
- `dma_transfers.sql` - Track all DMA transfer operations
- `memory_hotspots.sql` - Find most frequently accessed memory regions

### Comparative Analysis

- `compare_traces.sql` - Compare two trace files to find behavioral differences
- `register_changes.sql` - Track register value changes over time

## Tips

- Traces can be large (100-200MB for 100 frames). Use shorter test runs for faster analysis.
- Use Perfetto's built-in filtering to focus on specific time ranges.
- The SQL queries can be modified to suit your specific debugging needs.
- Bookmark useful queries in Perfetto for quick access.

## Trace Event Structure

Our traces include these event types:

### CPU Execution (`cpu_execution` target)

- **name**: Disassembled instruction
- **args.pc**: Program counter
- **args.a, args.f, args.b, args.c, args.d, args.e, args.h, args.l**: Register values
- **args.sp**: Stack pointer
- **args.cycles**: Instruction cycle count

### PPU Mode Changes (`ppu` target)

- **name**: "PPU mode change"
- **args.mode**: HBlank, VBlank, "OAM Scan", or Drawing
- **args.ly**: Current scanline
- **args.dots**: Remaining dots in mode

### DMA Transfers (`dma` target)

- **name**: "OAM DMA transfer" or "HDMA transfer"
- **args.src**: Source address
- **args.dst**: Destination address
- **args.bytes**: Transfer size
- **args.transfer_type**: (HDMA only) "HBlank" or "General"

### Memory Access (`memory` target)

- **name**: "Memory write"
- **args.addr**: Memory address
- **args.value**: Written value
- **args.region**: "VRAM" or "OAM"
