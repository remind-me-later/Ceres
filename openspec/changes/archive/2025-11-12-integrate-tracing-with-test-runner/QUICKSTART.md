# Quick Start: JSON Lines Implementation

This guide provides a straightforward implementation path for Phase 2 (Machine-Friendly Enhancements).

## Goal

Convert trace export from structured JSON to JSON Lines (JSONL) format for better machine processing.

## Why JSON Lines?

1. **Streaming**: Process one entry at a time without loading entire file
2. **Unix-friendly**: Works with grep, awk, jq, and other standard tools
3. **Memory efficient**: No need to hold entire trace in memory
4. **AI-friendly**: LLMs can process line-by-line
5. **Appendable**: Can write entries as they're generated

## Implementation Steps

### Step 1: Add JSONL Export to TestTracer

**File**: `ceres-test-runner/src/test_tracer.rs`

Add a new export method:

```rust
impl TestTracer {
    /// Export traces in JSON Lines format (one JSON object per line)
    pub fn export_jsonl(&self, path: &std::path::Path) -> std::io::Result<()> {
        use std::io::Write;
        
        let file = std::fs::File::create(path)?;
        let mut writer = std::io::BufWriter::new(file);
        
        let traces = self.get_traces();
        for entry in traces {
            // Flatten the entry structure for easier querying
            let flat_entry = serde_json::json!({
                "target": entry.target,
                "level": entry.level,
                "timestamp": entry.timestamp,
                "pc": entry.fields.get("pc"),
                "instruction": entry.fields.get("instruction"),
                "a": entry.fields.get("a"),
                "f": entry.fields.get("f"),
                "b": entry.fields.get("b"),
                "c": entry.fields.get("c"),
                "d": entry.fields.get("d"),
                "e": entry.fields.get("e"),
                "h": entry.fields.get("h"),
                "l": entry.fields.get("l"),
                "sp": entry.fields.get("sp"),
                "cycles": entry.fields.get("cycles"),
            });
            
            serde_json::to_writer(&mut writer, &flat_entry)?;
            writeln!(writer)?;
        }
        
        writer.flush()?;
        Ok(())
    }
}
```

### Step 2: Update TestConfig

**File**: `ceres-test-runner/src/test_runner.rs`

Add format option to `TestConfig`:

```rust
pub enum TraceFormat {
    Json,      // Structured JSON with metadata wrapper
    JsonLines, // One JSON object per line (default)
}

impl Default for TraceFormat {
    fn default() -> Self {
        TraceFormat::JsonLines
    }
}

pub struct TestConfig {
    // ... existing fields ...
    
    /// Format for trace export
    pub trace_format: TraceFormat,
}
```

### Step 3: Update Export Logic

**File**: `ceres-test-runner/src/test_runner.rs`

Modify `export_trace_if_enabled`:

```rust
fn export_trace_if_enabled(&self) {
    if !self.config.enable_trace {
        return;
    }

    if let Some(ref tracer) = self.tracer {
        if tracer.is_empty() {
            eprintln!("No trace data collected for export.");
            return;
        }

        // Create traces directory
        let trace_dir = std::path::PathBuf::from("target/traces");
        if let Err(e) = std::fs::create_dir_all(&trace_dir) {
            eprintln!("Failed to create trace directory: {e}");
            return;
        }

        // Generate trace filename with timestamp
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        // Choose extension based on format
        let extension = match self.config.trace_format {
            TraceFormat::Json => "json",
            TraceFormat::JsonLines => "jsonl",
        };
        
        let trace_path = trace_dir.join(format!("{timestamp}_trace.{extension}"));

        // Export based on format
        match self.config.trace_format {
            TraceFormat::JsonLines => {
                if let Err(e) = tracer.export_jsonl(&trace_path) {
                    eprintln!("Failed to export trace: {e}");
                } else {
                    println!("Trace exported to: {}", trace_path.display());
                }
            }
            TraceFormat::Json => {
                // Existing JSON export code...
                // (keep current implementation)
            }
        }
    }
}
```

### Step 4: Update Tests to Use JSONL

**File**: `ceres-test-runner/tests/mbc3_tester.rs`

```rust
let config = TestConfig {
    model,
    timeout_frames: 300,
    expected_screenshot: Some(test_roms_dir().join(format!("mbc3-tester/{screenshot_name}"))),
    enable_trace: true,
    export_trace_on_failure: true,
    trace_buffer_size: 10_000,
    trace_format: TraceFormat::JsonLines, // Use JSONL format
    ..TestConfig::default()
};
```

## Usage Examples

### Analyze with jq

```bash
# Count total instructions
wc -l trace.jsonl

# Show first 10 instructions
head -10 trace.jsonl | jq .

# Find all JP instructions
jq 'select(.instruction | contains("JP"))' trace.jsonl

# Track PC values
jq -r '.pc' trace.jsonl | head -20

# Find when register A equals 0xFF
jq 'select(.a == 255)' trace.jsonl

# Get unique instruction types
jq -r '.instruction' trace.jsonl | sort -u

# Find most common instructions
jq -r '.instruction' trace.jsonl | sort | uniq -c | sort -rn | head -10
```

### Analyze with Python

```python
import json

# Stream process large trace file
def analyze_trace(filename):
    with open(filename) as f:
        for i, line in enumerate(f):
            entry = json.loads(line)
            
            # Do analysis on each entry
            if entry['pc'] == 0x100:
                print(f"Instruction {i}: Started at ROM entry point")
            
            # Early exit if needed
            if i > 10000:
                break

analyze_trace('trace.jsonl')
```

### Analyze with grep/awk

```bash
# Find all instructions at PC 0x150
grep '"pc":336' trace.jsonl

# Count instructions by type
awk -F'"instruction":"' '{print $2}' trace.jsonl | awk -F'"' '{print $1}' | sort | uniq -c
```

## Enhanced Metadata

Create a separate metadata file alongside the trace:

**File**: `<timestamp>_trace.meta.json`

```json
{
  "test_name": "test_mbc3_tester_cgb",
  "test_file": "ceres-test-runner/tests/mbc3_tester.rs",
  "timestamp": 1762947510,
  "duration_ms": 920,
  "frames_executed": 300,
  "entry_count": 10000,
  "model": "CGB",
  "rom_name": "mbc3-tester.gb",
  "failure_reason": "Timeout",
  "buffer_size": 10000,
  "truncated": false,
  "trace_file": "1762947510_trace.jsonl",
  "format": "jsonl",
  "schema_version": "1.0"
}
```

This allows AI agents to understand context without parsing the entire trace file.

## Next Steps

1. Implement JSONL export (Steps 1-3)
2. Test with mbc3-tester
3. Verify file size reduction compared to JSON
4. Create analysis examples
5. Document schema for AI agents

## Expected Benefits

- **File size**: ~30% smaller than pretty-printed JSON
- **Processing speed**: 5-10x faster for streaming analysis
- **Memory usage**: Constant memory usage regardless of trace size
- **Tool compatibility**: Works with standard Unix tools
- **AI-friendly**: Easy for LLMs to process line-by-line
