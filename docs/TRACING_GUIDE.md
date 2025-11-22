# Chrome Tracing Guide

Ceres uses the Chrome Trace Event Format for debugging and performance analysis.

## Quick Start

1. **Generate Trace**:

   ```bash
   cd ceres-test-runner
   cargo test test_chrome_trace_export -- --ignored --nocapture
   ```

   Output: `ceres-test-runner/target/traces/test_chrome_trace_export_<timestamp>.json`

2. **Analyze**:
   - **UI**: Open [ui.perfetto.dev](https://ui.perfetto.dev) and drag in the trace file.
   - **CLI**: Use `trace_processor` with provided SQL queries.

     ```bash
     trace_processor -q docs/sql/tight_loops.sql <trace_file>
     ```

## Generating Traces

Enable tracing in your tests using `tracing_chrome`.

```rust
use tracing_subscriber::{layer::SubscriberExt, EnvFilter};

// ... inside your test ...
let (chrome_layer, _guard) = tracing_chrome::ChromeLayerBuilder::new()
    .file("trace.json")
    .include_args(true)
    .build();

let subscriber = tracing_subscriber::registry()
    .with(EnvFilter::new("trace,cpu_execution=info"))
    .with(chrome_layer);

tracing::subscriber::set_global_default(subscriber).unwrap();

// Optional: Filter by PC range to reduce size
runner.set_trace_pc_range(0x0100, 0xFFFF);
```

**Filtering Options**:

- `trace`: All events (large files, ~150MB/100 frames)
- `trace,cpu_execution=info`: Hardware focus (recommended, ~130MB)
- `ppu=trace,dma=trace`: Subsystem specific (~20MB)

## Trace Event Types

Traces include these event categories:

- **CPU Execution** (`cpu_execution` target)
  - Program counter, instruction, registers, flags, cycles
  - Use for: Finding hot code paths, debugging algorithms
- **PPU Mode Changes** (`ppu` target)
  - Mode transitions (OAM Scan, Drawing, HBlank, VBlank)
  - Scanline number, timing information
  - Use for: Debugging rendering issues, timing problems
- **DMA Transfers** (`dma` target)
  - Source/destination addresses, byte count, transfer type
  - Use for: Tracking sprite/tile uploads
- **Memory Access** (`memory` target)
  - VRAM and OAM write operations
  - Address, value, region
  - Use for: Finding memory hotspots, tracking data flow

## Performance Impact

Tracing is designed to have minimal performance impact when disabled:

- **Tracing disabled**: Zero overhead (events are not generated)
- **Tracing enabled**: ~10-15% slowdown depending on trace verbosity
- **Trace files**: ~1-2 MB per frame of emulation

## SQL Analysis Library

We provide a library of SQL queries in `docs/sql/` for common analysis tasks.

| Query | Purpose |
|-------|---------|
| `tight_loops.sql` | Find infinite loops and busy waits |
| `frame_timing.sql` | Analyze frame duration and FPS stability |
| `instruction_hotspots.sql` | Identify most executed instructions |
| `memory_hotspots.sql` | Find frequently accessed memory addresses |
| `ppu_mode_timeline.sql` | Debug PPU mode transitions and timing |
| `register_changes.sql` | Track CPU register state changes |
| `dma_transfers.sql` | Analyze OAM and HDMA transfers |
| `execution_fingerprint.sql` | Compare execution flow between runs |

## Troubleshooting

- **No trace file?** Ensure `_guard` is held until the test completes.
- **Empty trace?** Check `EnvFilter` settings and ensure `runner.enable_tracing()` is called.
- **Slow queries?** Some queries (like `register_changes.sql`) process millions of events; filter your trace or use a
  shorter run.

## Resources

- [Perfetto UI](https://ui.perfetto.dev)
- [Trace Processor Documentation](https://perfetto.dev/docs/analysis/trace-processor)
