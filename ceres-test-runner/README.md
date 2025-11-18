# Ceres Test Runner

Integration test runner for the Ceres Game Boy emulator using actual Game Boy test ROMs.

## Overview

The test suite validates emulator accuracy using pixel-perfect screenshot comparison against reference images. Tests
cover:

- **CPU Instructions**: Complete SM83 instruction set validation
- **Timing**: Instruction timing, memory timing, and interrupt timing
- **PPU Rendering**: CGB and DMG display accuracy tests

### Test Completion Detection

The test runner uses multiple mechanisms to detect when tests complete:

- **Breakpoint detection**: Test ROMs like cgb-acid2 and dmg-acid2 use the `ld b, b` instruction (opcode 0x40) as a
  debug breakpoint to signal completion. When detected, tests complete immediately after the screenshot matches.
- **Screenshot comparison**: Tests pass when the emulator output matches reference images pixel-for-pixel.
- **Timeout safety**: All tests have timeout values to prevent infinite loops in broken or incomplete test ROMs.

This approach allows Acid2 tests to complete in ~0.4 seconds instead of waiting for the full timeout (~20+ seconds).

## Setup

Test ROMs are **automatically downloaded** when you build or test this crate. No manual setup required!

The build script downloads the test ROM collection (172MB) from the
[c-sp/gameboy-test-roms](https://github.com/c-sp/gameboy-test-roms) repository on first build and caches it in the
`test-roms/` directory.

## Running Tests

### Run All Tests

```bash
cargo test --package ceres-test-runner
```

This will run all integration tests (~2-3 seconds total):

1. Download test ROMs automatically (if not already present)
2. Run CPU instruction tests
3. Run timing validation tests
4. Run PPU accuracy tests
5. Run unit tests for the test infrastructure

### Run Specific Test

```bash
# Run a specific test
cargo test --package ceres-test-runner test_blargg_cpu_instrs

# Run all dmg-acid2 tests
cargo test --package ceres-test-runner test_dmg_acid2

# Run ignored tests (known failures)
cargo test --package ceres-test-runner -- --ignored
```

### CI/CD Usage

Test ROMs are automatically downloaded in CI environments. For optimal performance, cache the `test-roms/` directory:

```yaml
# Example GitHub Actions workflow
- name: Cache test ROMs
  uses: actions/cache@v4
  with:
    path: test-roms
    key: test-roms-v7.0

- name: Run tests
  run: cargo test --package ceres-test-runner
```

## Test Suite

The tests are organized into logical modules:

### Blargg Test Suite (`tests/blargg_tests.rs`)

CPU instructions, timing, and hardware bug tests from Blargg's test suite:

- **`test_blargg_cpu_instrs`** - All 11 CPU instruction categories in one comprehensive test
- **`test_blargg_instr_timing`** - Validates instruction cycle timing
- **`test_blargg_mem_timing`** - Tests memory access timing
- **`test_blargg_mem_timing_2`** - Advanced memory timing scenarios
- **`test_blargg_interrupt_time`** - Interrupt handling timing
- **`test_blargg_halt_bug`** - HALT instruction edge cases

### PPU Accuracy Tests (`tests/ppu_tests.rs`)

Visual accuracy tests for the Pixel Processing Unit. These tests use breakpoint detection for fast completion:

- **`test_cgb_acid2`** - CGB PPU rendering accuracy
- **`test_dmg_acid2_cgb`** - DMG Acid2 test running in CGB mode
- **`test_dmg_acid2_dmg`** - DMG Acid2 test running in DMG mode

The Acid2 tests complete in ~0.4 seconds total thanks to breakpoint detection, compared to ~20 seconds if relying on
timeouts alone.

### Serial Output Tests (`tests/serial_test.rs`)

Serial communication functionality tests

## Known Issues

- **DMG PPU rendering**: The `test_dmg_acid2_dmg` test is currently failing due to differences in DMG mode PPU
  rendering. The CGB mode version passes, indicating the issue is specific to DMG palette handling or rendering
  behavior.

## Writing New Tests

To add a new test ROM:

1. Ensure the ROM is available in the `test-roms/` collection
2. Use the `load_test_rom()` helper to load it
3. Create a `TestRunner` with appropriate configuration
4. Run the test and assert on the result

Example:

```rust
use ceres_test_runner::{
    load_test_rom,
    test_runner::{TestConfig, TestResult, TestRunner, timeouts},
};

#[test]
fn test_my_rom() {
    let rom = load_test_rom("path/to/rom.gb").expect("Failed to load ROM");

    let config = TestConfig {
        timeout_frames: 300,
        expected_screenshot: ceres_test_runner::expected_screenshot_path(
            "path/to/rom.gb",
            ceres_core::Model::Cgb,
        ),
        ..TestConfig::default()
    };

    let mut runner = TestRunner::new(rom, config).expect("Failed to create runner");
    let result = runner.run();

    assert_eq!(result, TestResult::Passed, "Test failed");
}
```

### Breakpoint Detection

If your test ROM uses the `ld b, b` instruction (opcode 0x40) as a completion signal:

1. Configure an `expected_screenshot` in the `TestConfig`
2. The test runner will automatically detect the breakpoint and complete immediately when the screenshot matches
3. Set an appropriate timeout as a safety net (the test will use whichever comes first: breakpoint match or timeout)

The breakpoint detection allows tests to complete as soon as they signal completion, rather than waiting for arbitrary
timeouts.

## Execution Tracing

The test runner supports Chrome Trace Event Format export for detailed emulator debugging and performance analysis.

### Quick Start

Generate a trace file:

```bash
# Run the trace export test (creates ~130MB trace for 100 frames)
cargo test --package ceres-test-runner test_chrome_trace_export -- --ignored --nocapture
```

This creates a trace file in `ceres-test-runner/target/traces/test_chrome_trace_export_<timestamp>.json`.

### Viewing Traces

**Option 1: Perfetto UI (Recommended)**

1. Open [ui.perfetto.dev](https://ui.perfetto.dev) in your browser
2. Drag and drop the trace file
3. Explore the timeline and use SQL queries

**Option 2: Chrome Tracing**

1. Open Chrome and navigate to `chrome://tracing`
2. Click "Load" and select the trace file
3. Use WASD keys to navigate the timeline

### Using SQL Queries

The `examples/sql/` directory contains ready-to-use SQL queries for common debugging scenarios:

```bash
# Analyze tight loops
trace_processor -q examples/sql/tight_loops.sql <trace_file.json>

# View instruction hotspots
trace_processor -q examples/sql/instruction_hotspots.sql <trace_file.json>

# Check PPU timing
trace_processor -q examples/sql/ppu_mode_timeline.sql <trace_file.json>

# Analyze frame timing
trace_processor -q examples/sql/frame_timing.sql <trace_file.json>
```

See `examples/sql/README.md` for complete documentation and `examples/sql/QUICK_REFERENCE.md` for a quick lookup guide.

### Enabling Tracing in Your Tests

To enable tracing in custom tests, set up the Chrome tracing layer before creating the test runner:

```rust
use tracing_subscriber::{layer::SubscriberExt, EnvFilter};

#[test]
#[ignore]
fn my_traced_test() {
    // Set up Chrome tracing
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let trace_dir = manifest_dir.join("target/traces");
    std::fs::create_dir_all(&trace_dir).unwrap();
    let trace_path = trace_dir.join("my_test.json");
    
    let (chrome_layer, _guard) = tracing_chrome::ChromeLayerBuilder::new()
        .file(&trace_path)
        .include_args(true)
        .build();
    
    // Enable trace level for hardware events, info for CPU
    let filter = EnvFilter::new("trace,cpu_execution=info");
    let subscriber = tracing_subscriber::registry()
        .with(filter)
        .with(chrome_layer);
    
    let _subscriber_guard = tracing::subscriber::set_default(subscriber);
    
    // Create and run test
    let rom = load_test_rom("path/to/rom.gb").unwrap();
    let config = TestConfig {
        timeout_frames: 100,
        ..TestConfig::default()
    };
    
    let mut runner = TestRunner::new(rom, config).unwrap();
    runner.enable_tracing();  // Enable emulator tracing
    
    // Optional: Skip boot ROM and trace only game code (0x0100 onwards)
    runner.set_trace_pc_range(0x0100, 0xFFFF);
    
    let result = runner.run();
    
    // Cleanup to flush traces
    drop(runner);
    drop(_subscriber_guard);
    drop(_guard);
    
    eprintln!("Trace written to: {}", trace_path.display());
}
```

### PC Range Filtering

To reduce trace size and focus on specific code sections, you can filter by program counter (PC) range:

```rust
// Skip boot ROM execution - only trace game code starting at 0x0100
runner.set_trace_pc_range(0x0100, 0xFFFF);

// Trace only a specific function (e.g., addresses 0x0150-0x0200)
runner.set_trace_pc_range(0x0150, 0x0200);
```

**Common use cases**:

- **Skip boot ROM**: `set_trace_pc_range(0x0100, 0xFFFF)` - Start tracing when the game code begins
- **Skip header**: `set_trace_pc_range(0x0150, 0x7FFF)` - Skip ROM header and trace only code
- **Specific routine**: `set_trace_pc_range(0x1234, 0x1256)` - Debug a specific function

This can reduce trace size by 50-80% when you don't need boot ROM execution data!

### Trace Event Types

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

### Performance Impact

Tracing is designed to have minimal performance impact when disabled:

- **Tracing disabled**: Zero overhead (events are not generated)
- **Tracing enabled**: ~10-15% slowdown depending on trace verbosity
- Trace files: ~1-2 MB per frame of emulation
