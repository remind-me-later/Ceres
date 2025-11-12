# Trace Analysis Tools - Quick Reference

Quick reference for the three CLI tools provided in Phase 3.

## Tool Overview

| Tool             | Purpose                       | Primary Use Case                             |
| ---------------- | ----------------------------- | -------------------------------------------- |
| `trace-query`    | Search and extract trace data | Find specific PC values or instructions      |
| `trace-diff`     | Compare two traces            | Find where passing/failing tests diverge     |
| `trace-patterns` | Detect execution patterns     | Identify infinite loops and timing anomalies |

## trace-query

### Commands

```bash
# Build index
trace-query index --trace FILE.jsonl [--interval 1000] [--output FILE.index.json]

# Show statistics
trace-query stats FILE.jsonl

# Query by field
trace-query query --trace FILE.jsonl "pc:0x100" [--max-results 10]
trace-query query --trace FILE.jsonl "instruction:LD A, B"

# Find specific PC
trace-query find-pc --trace FILE.jsonl --pc 0x100

# Find specific instruction
trace-query find-instruction --trace FILE.jsonl --instruction "DEC C" [--context 2]

# Extract specific lines
trace-query extract --trace FILE.jsonl --lines "10,20,30"
trace-query extract --trace FILE.jsonl --lines "100-110"
```

### Common Options

- `--trace FILE.jsonl` - Path to trace file
- `--index FILE.index.json` - Path to index (auto-detected if omitted)
- `--max-results N` - Limit results (default: 10)
- `--context N` - Show N lines of context

## trace-diff

### Basic Usage

```bash
# Compare two traces
trace-diff --trace-a PASS.jsonl --trace-b FAIL.jsonl

# Find divergence point (stop at first diff)
trace-diff -a PASS.jsonl -b FAIL.jsonl --stop-at-first

# Compare specific fields
trace-diff -a PASS.jsonl -b FAIL.jsonl --fields pc,instruction

# Show more differences
trace-diff -a PASS.jsonl -b FAIL.jsonl --max-diffs 50
```

### Options

- `--trace-a FILE.jsonl` (or `-a`) - First trace
- `--trace-b FILE.jsonl` (or `-b`) - Second trace
- `--max-diffs N` (or `-n`) - Max differences to show (default: 20)
- `--context N` (or `-c`) - Lines of context (default: 2)
- `--fields LIST` (or `-f`) - Fields to compare: `pc`, `instruction`, `registers`, `all` (default: `all`)
- `--stop-at-first` (or `-s`) - Stop at first difference

### Field Options

- `pc` - Compare program counter only
- `instruction` - Compare instruction mnemonics
- `registers` - Compare all register values
- `all` - Compare everything (default)

## trace-patterns

### Basic Usage

```bash
# Detect patterns
trace-patterns FILE.jsonl

# Adjust detection thresholds
trace-patterns FILE.jsonl --min-loop-iterations 50 --tight-loop-threshold 5

# Show detailed information
trace-patterns FILE.jsonl --verbose
```

### Options

- `--min-loop-iterations N` (or `-l`) - Min iterations to report (default: 100)
- `--tight-loop-threshold N` (or `-t`) - Consecutive PCs for tight loop (default: 10)
- `--verbose` (or `-v`) - Show detailed loop information

### Output Sections

1. **Tight Loops** - Same instruction repeating consecutively
2. **Loop Patterns** - Repeating instruction sequences
3. **PC Distribution** - Most frequently executed code locations
4. **Instruction Frequency** - Most common instructions
5. **Summary** - Overall statistics

## Typical Workflows

### Workflow 1: Debug a Timeout

```bash
# 1. Run test (traces generated automatically on failure)
cargo test test_name

# 2. Detect what's happening
trace-patterns target/traces/test_name_*_trace.jsonl

# 3. If tight loop detected, find where it starts
trace-query find-instruction \
    --trace target/traces/test_name_*_trace.jsonl \
    --instruction "JR NZ, \$FD" \
    --context 5

# 4. Extract surrounding context
trace-query extract \
    --trace target/traces/test_name_*_trace.jsonl \
    --lines "0-20"
```

### Workflow 2: Compare Passing vs Failing

```bash
# 1. Get both traces (modify test config to save passing traces too)
# 2. Compare
trace-diff \
    --trace-a passing_trace.jsonl \
    --trace-b failing_trace.jsonl \
    --stop-at-first

# 3. Examine divergence point
trace-query extract \
    --trace failing_trace.jsonl \
    --lines "1230-1240"  # Line from diff output
```

### Workflow 3: Analyze Specific Code Region

```bash
# 1. Find all executions of a PC
trace-query find-pc --trace FILE.jsonl --pc 0x150

# 2. Build index for faster queries
trace-query index --trace FILE.jsonl

# 3. Query specific instruction
trace-query query --trace FILE.jsonl "instruction:CALL 0x2000"

# 4. Get statistics
trace-query stats FILE.jsonl
```

## Tips

### Using Wildcards

```bash
# Most recent trace for a test
trace-patterns target/traces/test_name_*_trace.jsonl

# All traces
trace-patterns target/traces/*_trace.jsonl
```

### Piping to Tools

```bash
# Count unique PCs
trace-query query --trace FILE.jsonl "pc:*" | grep "PC=" | sort | uniq | wc -l

# Extract and analyze with jq
trace-query extract --trace FILE.jsonl --lines "100-200" | jq .
```

### Performance

- Indexes are generated automatically (can disable with `generate_index: false`)
- Query tool uses indexes automatically if available
- For large traces (>100K entries), always use indexes
- Pattern detection works directly on JSONL (no index needed)

## Integration with Tests

```rust
use ceres_test_runner::test_runner::{TestConfig, TraceFormat};

let config = TestConfig {
    enable_trace: true,
    export_trace_on_failure: true,
    trace_format: TraceFormat::JsonLines,
    test_name: Some("my_test".to_string()),
    generate_index: true,  // Enable indexing
    checkpoint_interval: 1000,  // Checkpoint every 1000 instructions
    ..TestConfig::default()
};
```

## File Naming Convention

Traces follow the pattern: `<test_name>_<timestamp>_trace.<ext>`

Example files:

```
test_mbc3_tester_cgb_1762961797_trace.jsonl       # Trace data
test_mbc3_tester_cgb_1762961797_trace.meta.json   # Metadata
test_mbc3_tester_cgb_1762961797_trace.index.json  # Index
```

## Getting Help

```bash
# Tool help
trace-query --help
trace-query index --help
trace-diff --help
trace-patterns --help

# See full documentation
cat openspec/changes/integrate-tracing-with-test-runner/PHASE3_SUMMARY.md
```
