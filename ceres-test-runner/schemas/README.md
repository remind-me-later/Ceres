# Ceres Trace Format Schemas

This directory contains JSON Schema definitions for the Ceres emulator trace formats. These schemas enable AI agents,
validation tools, and developers to understand and process trace data.

## Schema Files

### `trace-entry.schema.json`

Defines the structure of a single trace entry in JSON Lines format. Each line in a `.jsonl` trace file conforms to this
schema.

**Key Fields:**

- `pc`: Program counter (instruction address)
- `instruction`: Disassembled SM83 instruction
- `a,f,b,c,d,e,h,l`: CPU register values
- `sp`: Stack pointer
- `cycles`: Instruction cycle count
- `timestamp`: Event timestamp in milliseconds

**Usage:**

```bash
# Validate a trace entry
echo '{"target":"cpu_execution","level":"TRACE","timestamp":1234567890,"pc":502,"instruction":"DEC C","a":2,"f":64,"b":203,"c":242,"d":152,"e":16,"h":50,"l":141,"sp":57343,"cycles":1}' | \
  jq --schema trace-entry.schema.json
```

### `trace-metadata.schema.json`

Defines the structure of trace metadata files (`.meta.json`). Metadata provides context about the trace collection
session.

**Key Fields:**

- `test_name`: Name of the test that generated the trace
- `entry_count`: Total number of trace entries
- `duration_ms`: Test execution duration
- `frames_executed`: Number of emulator frames run
- `model`: Game Boy model (DMG, CGB, etc.)
- `failure_reason`: Why the test failed (if applicable)
- `truncated`: Whether the buffer filled before test completed

**Usage:**

```bash
# Validate metadata
jq --schema trace-metadata.schema.json test_name_timestamp_trace.meta.json
```

## For AI Agents

These schemas enable AI agents to:

1. **Validate trace data** - Ensure traces conform to expected format
2. **Generate type-safe parsers** - Auto-generate code for trace processing
3. **Understand value ranges** - Detect anomalies (e.g., PC > 0xFFFF is invalid)
4. **Build analysis tools** - Create automated debugging utilities

### Example AI Agent Usage

```python
import json
import jsonschema

# Load schema
with open('schemas/trace-entry.schema.json') as f:
    schema = json.load(f)

# Validate trace entry
with open('trace.jsonl') as f:
    for line in f:
        entry = json.loads(line)
        jsonschema.validate(entry, schema)
        # Process validated entry...
```

## Schema Version

Current version: **1.0**

The `schema_version` field in metadata indicates the format version. Future changes will increment this version to
maintain backward compatibility.

## See Also

- `/openspec/changes/integrate-tracing-with-test-runner/examples/trace-analysis-examples.md` - Analysis examples
- `/openspec/changes/integrate-tracing-with-test-runner/QUICKSTART.md` - Implementation guide
- `/ceres-test-runner/README.md` - Test runner documentation
