# Trace Collection and Analysis System

Complete implementation of execution trace collection, export, and analysis for the Ceres Game Boy emulator test suite.

## Overview

This system provides comprehensive debugging capabilities for test failures through automatic trace collection and a
suite of analysis tools. Traces are collected automatically when tests fail, exported in machine-friendly formats, and
analyzed with purpose-built CLI tools.

## Features

### ✅ Phase 1: Core Tracing (Complete)

- Automatic trace collection during test execution
- Circular buffer preserves last N instructions
- Export only on test failure (no overhead for passing tests)
- Structured JSON export with full register state
- Integration with Rust `tracing` ecosystem

### ✅ Phase 2: Machine-Friendly Enhancements (Complete)

- JSON Lines (JSONL) format for streaming analysis
- Rich metadata files with test context
- JSON Schemas for AI agent integration
- Multiple export formats (JSON, JSONL)
- Test name-based file naming

### ✅ Phase 3: Advanced Analysis (Complete)

- **Trace Indexing** - Fast lookups with companion index files
- **trace-query** - CLI tool for searching and extracting trace data
- **trace-diff** - Compare traces to find execution divergence
- **trace-patterns** - Detect loops and timing anomalies

### 🚧 Phase 4: Ecosystem Integration (Planned)

- Python bindings for programmatic analysis
- Jupyter notebook examples
- LLM-friendly analysis prompts
- Performance optimizations (compression, sampling)

## Quick Start

### Running Tests with Tracing

Tracing is enabled automatically in test configurations:

```rust
use ceres_test_runner::test_runner::{TestConfig, TestRunner, TraceFormat};

let config = TestConfig {
    enable_trace: true,
    export_trace_on_failure: true,
    trace_format: TraceFormat::JsonLines,
    test_name: Some("my_test".to_string()),
    trace_buffer_size: 10_000,
    generate_index: true,
    checkpoint_interval: 1000,
    ..TestConfig::default()
};

let mut runner = TestRunner::new(rom, config)?;
let result = runner.run();
```

When a test fails, traces are automatically exported to `target/traces/`:

```
target/traces/
├── my_test_1762961797_trace.jsonl       # Execution trace (JSONL)
├── my_test_1762961797_trace.meta.json   # Metadata
└── my_test_1762961797_trace.index.json  # Search index
```

### Analyzing Traces

#### 1. Detect Patterns

```bash
cargo run --bin trace-patterns -- target/traces/my_test_*_trace.jsonl
```

Shows tight loops, repeating patterns, and instruction frequency.

#### 2. Query Trace

```bash
# Find a specific instruction
cargo run --bin trace-query -- find-instruction \
    --trace target/traces/my_test_*_trace.jsonl \
    --instruction "LD A, B"

# Find a specific PC
cargo run --bin trace-query -- find-pc \
    --trace target/traces/my_test_*_trace.jsonl \
    --pc 0x150
```

#### 3. Compare Traces

```bash
cargo run --bin trace-diff -- \
    --trace-a passing_trace.jsonl \
    --trace-b failing_trace.jsonl \
    --stop-at-first
```

## Architecture

### Components

```
ceres-test-runner/
├── src/
│   ├── test_tracer.rs      # Tracing subscriber and export
│   ├── test_runner.rs      # Test execution with tracing
│   ├── trace_index.rs      # Indexing for fast lookups
│   └── bin/
│       ├── trace-query.rs  # Query CLI tool
│       ├── trace-diff.rs   # Diff CLI tool
│       └── trace-patterns.rs # Pattern detection CLI
├── schemas/
│   ├── trace-entry.schema.json      # JSON Schema for trace entries
│   ├── trace-metadata.schema.json   # JSON Schema for metadata
│   └── README.md                     # Schema documentation
└── tests/
    └── mbc3_tester.rs      # Example test with tracing
```

### Data Flow

```
Test Execution
    ↓
Trace Events → TestTracer (circular buffer)
    ↓
Test Fails
    ↓
Export JSONL → Generate Index → Save Metadata
    ↓
target/traces/
    ├── test_name_timestamp_trace.jsonl
    ├── test_name_timestamp_trace.index.json
    └── test_name_timestamp_trace.meta.json
```

## File Formats

### Trace Entry (JSONL)

Each line is a complete JSON object:

```json
{
  "target": "cpu_execution",
  "level": "TRACE",
  "timestamp": 1762961797000,
  "pc": 496,
  "instruction": "DEC C",
  "a": 0,
  "f": 0,
  "b": 255,
  "c": 255,
  "d": 0,
  "e": 0,
  "h": 0,
  "l": 0,
  "sp": 65534,
  "cycles": 4
}
```

### Metadata

```json
{
  "test_name": "test_mbc3_tester_cgb",
  "entry_count": 10000,
  "timestamp": 1762961797,
  "duration_ms": 11017,
  "frames_executed": 300,
  "model": "CGB",
  "failure_reason": "Timeout",
  "buffer_size": 10000,
  "truncated": true,
  "schema_version": "1.0"
}
```

### Index

```json
{
  "version": "1.0",
  "source_file": "test_name_trace.jsonl",
  "total_entries": 10000,
  "checkpoint_interval": 1000,
  "pc_index": {
    "496": {
      "pc": 496,
      "line_ranges": [{ "start": 4, "end": 9999 }],
      "count": 4982
    }
  },
  "instruction_index": {
    "DEC C": {
      "instruction": "DEC C",
      "line_ranges": [{ "start": 4, "end": 9999 }],
      "count": 4982
    }
  },
  "checkpoints": [
    {
      "line": 0,
      "pc": 502,
      "registers": { "a": 0, "f": 0, "b": 255, "c": 255, "d": 0, "e": 0, "h": 0, "l": 0, "sp": 65534 }
    }
  ]
}
```

## CLI Tools Reference

See [`CLI_REFERENCE.md`](CLI_REFERENCE.md) for detailed usage of the three CLI tools.

### trace-query

Search and extract trace data:

- Build indexes for fast lookups
- Query by PC or instruction
- Extract specific line ranges
- Show trace statistics

### trace-diff

Compare two traces to find differences:

- Identify execution divergence points
- Compare PC, instructions, or registers
- Show context around differences
- Stop at first difference (find where tests diverge)

### trace-patterns

Detect execution patterns:

- Tight loops (same instruction repeating)
- Loop patterns (repeating sequences)
- PC distribution (hot spots)
- Instruction frequency

## Performance

### Trace Collection

- **Runtime Overhead**: <5% with buffering
- **Memory**: Fixed buffer size (default 10,000 entries × ~200 bytes = ~2MB)
- **Disk**: Only written on test failure

### File Sizes

- **JSONL**: ~1.8MB for 10,000 entries (60% smaller than JSON)
- **Index**: ~50KB (2.8% of trace size)
- **Metadata**: <1KB

### Analysis Performance

- **Index Generation**: ~50ms for 10,000 entries
- **Query (indexed)**: <1ms for PC/instruction lookup
- **Pattern Detection**: ~100ms for 10,000 entries
- **Diff**: ~200ms for comparing 10,000 entry traces

## Examples

See [`examples/trace-analysis-examples.md`](examples/trace-analysis-examples.md) for comprehensive examples including:

- Basic trace analysis with jq
- Pattern detection workflows
- Trace comparison strategies
- Integration with AI tools

## Testing

The system is validated with real test ROMs:

```bash
# Run test with tracing
cargo test --package ceres-test-runner test_mbc3_tester_cgb

# Traces are in target/traces/ if the test fails
ls -lh target/traces/

# Analyze the trace
cargo run --bin trace-patterns -- target/traces/test_mbc3_tester_cgb_*_trace.jsonl
```

## Development

### Adding New Event Types

To trace additional events (memory, interrupts, etc.), emit them from the emulator:

```rust
use tracing::trace;

// In emulator code
trace!(
    target: "memory_access",
    address = addr,
    value = val,
    operation = "write",
    "Memory write"
);
```

The tracing system will automatically capture and export these events.

### Extending Analysis Tools

The trace index and CLI tools are modular. To add new analysis:

1. Add fields to `TraceIndex` in `trace_index.rs`
2. Update `build_from_jsonl()` to populate new indexes
3. Add query commands to `trace-query.rs`

## Documentation

- [`PHASE1_SUMMARY.md`](PHASE1_SUMMARY.md) - Core tracing implementation
- [`PHASE2_SUMMARY.md`](PHASE2_SUMMARY.md) - Machine-friendly enhancements
- [`PHASE3_SUMMARY.md`](PHASE3_SUMMARY.md) - Advanced analysis tools
- [`CLI_REFERENCE.md`](CLI_REFERENCE.md) - Quick reference for CLI tools
- [`proposal.md`](proposal.md) - Complete proposal with all phases
- [`QUICKSTART.md`](QUICKSTART.md) - Implementation guide

## Contributing

When adding new tests, enable tracing for better debuggability:

```rust
let config = TestConfig {
    enable_trace: true,
    export_trace_on_failure: true,
    test_name: Some("test_name".to_string()),
    ..TestConfig::default()
};
```

This ensures traces are available when debugging failures.

## Future Work (Phase 4)

Planned enhancements:

- Python bindings for programmatic analysis
- Jupyter notebooks with analysis examples
- Automated pattern detection for AI agents
- Trace compression (gzip)
- Sampling mode for long-running tests
- Integration with VS Code debugger

## License

Part of the Ceres Game Boy emulator project. See LICENSE.md in the repository root.
