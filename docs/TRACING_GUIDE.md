# Chrome Tracing Guide for Ceres

Complete guide to using Chrome Trace Event Format traces for debugging and performance analysis in the Ceres Game Boy emulator.

## Table of Contents

1. [Quick Start](#quick-start)
2. [Generating Traces](#generating-traces)
3. [Viewing Traces](#viewing-traces)
4. [SQL Analysis](#sql-analysis)
5. [Common Debugging Workflows](#common-debugging-workflows)
6. [Advanced Topics](#advanced-topics)

## Quick Start

### 1. Generate a Trace

```bash
cd ceres-test-runner
cargo test test_chrome_trace_export -- --ignored --nocapture
```

This creates `ceres-test-runner/target/traces/test_chrome_trace_export_<timestamp>.json` (~130MB for 100 frames).

### 2. Open in Perfetto

1. Go to [ui.perfetto.dev](https://ui.perfetto.dev)
2. Drag the trace file into the browser
3. Explore the timeline visually or run SQL queries

### 3. Run SQL Queries

```bash
# Find tight loops
trace_processor -q examples/sql/tight_loops.sql <trace_file>

# Analyze frame timing
trace_processor -q examples/sql/frame_timing.sql <trace_file>
```

## Generating Traces

### In Tests

Use the provided test as a template:

```rust
use tracing_subscriber::{layer::SubscriberExt, EnvFilter};
use ceres_test_runner::{load_test_rom, test_runner::{TestConfig, TestRunner}};

#[test]
#[ignore]
fn my_debug_trace() {
    // Configure trace output
    let trace_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target/traces/my_debug.json");
    std::fs::create_dir_all(trace_path.parent().unwrap()).unwrap();
    
    // Set up tracing
    let (chrome_layer, _guard) = tracing_chrome::ChromeLayerBuilder::new()
        .file(&trace_path)
        .include_args(true)
        .build();
    
    let filter = EnvFilter::new("trace,cpu_execution=info");
    let subscriber = tracing_subscriber::registry()
        .with(filter)
        .with(chrome_layer);
    let _sub_guard = tracing::subscriber::set_default(subscriber);
    
    // Run emulator with tracing
    let rom = load_test_rom("path/to/test.gb").unwrap();
    let config = TestConfig {
        timeout_frames: 100,
        ..Default::default()
    };
    
    let mut runner = TestRunner::new(rom, config).unwrap();
    runner.enable_tracing();
    runner.run();
    
    eprintln!("Trace: {}", trace_path.display());
}
```

### Trace Filtering

Control what gets traced using the `EnvFilter`:

```rust
// Trace everything at trace level
EnvFilter::new("trace")

// Trace hardware, but only info for CPU (less verbose)
EnvFilter::new("trace,cpu_execution=info")

// Only PPU and DMA events
EnvFilter::new("ppu=trace,dma=trace")

// Disable all tracing
EnvFilter::new("off")
```

### PC Range Filtering

Skip boot ROM execution and trace only your game code:

```rust
// Enable tracing and set PC range
runner.enable_tracing();
runner.set_trace_pc_range(0x0100, 0xFFFF);  // Start from game entry point

// Or trace only a specific function (e.g., 0x0150 to 0x0200)
runner.set_trace_pc_range(0x0150, 0x0200);
```

**Common ranges**:
- `0x0100` to `0xFFFF` - Skip boot ROM, trace all game code
- `0x0150` to `0x7FFF` - Skip header, trace ROM code only
- Custom ranges - Focus on specific functions/routines

This dramatically reduces trace size and makes analysis easier!

### Trace Size Management

| Filter Level | Trace Size (100 frames) | Use Case |
|--------------|-------------------------|----------|
| `trace` | ~150MB | Full debugging |
| `trace,cpu_execution=info` | ~130MB | Hardware focus |
| `ppu=trace,dma=trace,memory=trace` | ~20MB | PPU/memory only |
| `cpu_execution=info` | ~110MB | CPU only |

## Viewing Traces

### Perfetto UI (Recommended)

**Pros**: Advanced SQL queries, better performance, more features
**Cons**: Requires internet connection

1. Open [ui.perfetto.dev](https://ui.perfetto.dev)
2. Drag and drop trace file
3. Navigate with:
   - **W/S**: Zoom in/out
   - **A/D**: Pan left/right
   - **Mouse wheel**: Zoom
   - **Click and drag**: Select time range

**Key Features**:
- SQL query editor (left sidebar)
- Thread/process view
- Event details on click
- Time range selection
- Bookmarks

### Chrome Tracing

**Pros**: Works offline, integrated with Chrome DevTools
**Cons**: Limited analysis capabilities

1. Open `chrome://tracing` in Chrome
2. Click "Load"
3. Select trace file
4. Navigate with WASD keys

## SQL Analysis

### Available Queries

Located in `examples/sql/`:

| Query | Purpose | Speed |
|-------|---------|-------|
| `tight_loops.sql` | Find infinite loops, busy waits | Fast |
| `instruction_hotspots.sql` | Most executed instructions | Fast |
| `ppu_mode_timeline.sql` | PPU timing analysis | Medium |
| `frame_timing.sql` | Frame rate/timing issues | Fast |
| `dma_transfers.sql` | DMA upload tracking | Fast |
| `memory_hotspots.sql` | Hot memory addresses | Medium |
| `register_changes.sql` | Register value tracking | Slow |
| `execution_fingerprint.sql` | Execution comparison | Fast |

See `examples/sql/QUICK_REFERENCE.md` for quick reference.

### Running Queries

```bash
# Using trace_processor
trace_processor -q examples/sql/tight_loops.sql trace.json

# Save results to CSV
trace_processor -q examples/sql/frame_timing.sql trace.json > results.csv

# Interactive mode
trace_processor trace.json
# Then type SQL queries directly
```

### Custom Queries

The trace uses standard Perfetto/Chrome trace format. Key tables:

- `slice`: All trace events
- `args`: Event arguments/metadata
- `thread`: Thread information
- `process`: Process information

Example custom query:

```sql
-- Find all VRAM writes to specific address
SELECT
  s.ts / 1000000.0 AS time_ms,
  (SELECT string_value FROM args WHERE arg_set_id = s.arg_set_id AND key = 'args.addr') AS addr,
  (SELECT string_value FROM args WHERE arg_set_id = s.arg_set_id AND key = 'args.value') AS value
FROM slice s
WHERE s.cat = 'memory'
  AND (SELECT string_value FROM args WHERE arg_set_id = s.arg_set_id AND key = 'args.addr') = '"$8000"'
ORDER BY s.ts;
```

## Common Debugging Workflows

### Finding Infinite Loops

1. Generate trace
2. Run `tight_loops.sql`
3. Look for high `execution_count` at same PC
4. Check if instruction is `JR`, `JP`, or `HALT`

```bash
trace_processor -q examples/sql/tight_loops.sql trace.json | head -20
```

Expected output:
```
"537",""JR Z, $FC"",255812,1724,352,2077
```

This shows PC 537 executing 255,812 times = likely stuck in loop.

### Debugging Timing Issues

1. Run `frame_timing.sql`
2. Look for frames with abnormal duration
3. Check `timing_status` column

```bash
trace_processor -q examples/sql/frame_timing.sql trace.json
```

Normal: 16-17ms per frame (~60 FPS)
Slow: >17ms (indicates performance issue or waiting)
Fast: <16ms (emulator running too fast)

### PPU Rendering Issues

1. Run `ppu_mode_timeline.sql`
2. Verify mode transitions are correct:
   - OAM Scan: ~80 dots
   - Drawing: 172-289 dots (varies with scroll)
   - HBlank: 87-204 dots
3. Check scanline progression

```bash
trace_processor -q examples/sql/ppu_mode_timeline.sql trace.json | less
```

### Memory Access Patterns

1. Run `memory_hotspots.sql`
2. Look for addresses with high access counts
3. Check if pattern makes sense for your ROM

```bash
trace_processor -q examples/sql/memory_hotspots.sql trace.json
```

VRAM $9800-$9BFF = Background tilemap
VRAM $8000-$97FF = Tile data
OAM $FE00-$FE9F = Sprite attributes

## Advanced Topics

### Comparing Traces

To compare emulator behavior between versions:

1. Generate trace from version A
2. Generate trace from version B (same ROM, same frame count)
3. Run `execution_fingerprint.sql` on both
4. Export results and diff

```bash
trace_processor -q examples/sql/execution_fingerprint.sql v1_trace.json > v1.csv
trace_processor -q examples/sql/execution_fingerprint.sql v2_trace.json > v2.csv
diff v1.csv v2.csv
```

### Performance Profiling

Use `instruction_hotspots.sql` to find optimization targets:

```bash
trace_processor -q examples/sql/instruction_hotspots.sql trace.json
```

Focus on:
- Instructions with high `execution_count`
- Instructions at specific PCs (likely in loops)
- `percent_of_total` to find biggest contributors

### Trace Overhead

Tracing has measurable overhead:

| Measurement | No Trace | With Trace | Overhead |
|-------------|----------|------------|----------|
| Frame time | 16.7ms | ~19ms | +13% |
| File I/O | None | Continuous | Disk I/O |
| Memory | Normal | +50-100MB | Buffer |

For accurate timing measurements, compare traces (relative timing) rather than absolute times.

### Debugging Test Failures

When a test fails:

1. Add tracing to the test
2. Run until failure
3. Use `register_changes.sql` to track register values
4. Compare PC/instruction sequence with known-good trace

```rust
#[test]
fn debug_failing_test() {
    // Enable tracing
    // ... setup code ...
    
    let mut runner = TestRunner::new(rom, config).unwrap();
    runner.enable_tracing();
    
    match runner.run() {
        TestResult::Passed => {},
        other => {
            eprintln!("Test failed: {:?}", other);
            eprintln!("Check trace for details");
        }
    }
}
```

## Tips and Tricks

1. **Start small**: Trace 10-50 frames first, not 1000
2. **Use filters**: Enable only the events you need
3. **Time ranges**: In Perfetto, select time ranges to focus analysis
4. **Export results**: Save SQL query results to CSV for external analysis
5. **Bookmark locations**: In Perfetto, bookmark interesting time points
6. **Compare side-by-side**: Open multiple Perfetto tabs for comparison
7. **Check file size**: If trace is >500MB, reduce frame count or filter more

## Troubleshooting

### Trace file not created

- Check that `target/traces/` directory exists
- Ensure guards are held until after trace completes
- Add `std::thread::sleep(Duration::from_millis(500))` before test ends

### Trace loads but shows nothing

- Check filter level (may be filtering out all events)
- Verify `runner.enable_tracing()` was called
- Check that ROM actually executes (not stuck at boot)

### Perfetto UI won't load trace

- File may be corrupted (incomplete flush)
- File too large (>1GB) - try reducing frame count
- Browser may need more memory - close other tabs

### Queries return empty results

- Check event category names match (`cpu_execution`, `ppu`, `dma`, `memory`)
- Verify args keys match exactly (`args.pc`, `args.mode`, etc.)
- Check data types (some values are strings, not ints)

## See Also

- `examples/sql/README.md` - SQL query documentation
- `examples/sql/QUICK_REFERENCE.md` - Quick lookup guide
- `examples/sql/TEST_RESULTS.md` - Query test results
- [Perfetto Documentation](https://perfetto.dev/docs/)
- [Chrome Trace Event Format](https://docs.google.com/document/d/1CvAClvFfyA5R-PhYUmn5OOQtYMH4h6I0nSsKchNAySU)
