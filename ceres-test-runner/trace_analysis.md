# Trace Analysis Guide

This guide explains how to analyze execution traces exported by the test runner when tests fail.

## Understanding Trace Files

When a test fails or times out, the test runner exports a trace file to `target/traces/<timestamp>_trace.json` containing detailed execution information.

The trace file has the following structure:
```json
{
  "metadata": {
    "entry_count": 500,
    "timestamp": 1234567890
  },
  "entries": [
    {
      "target": "cpu_execution",
      "level": "TRACE",
      "name": "EXECUTE_INSTRUCTION",
      "timestamp": 1234567890123,
      "fields": {
        "pc": 1234,
        "instruction": "LD A, $42",
        "a": 66,
        "f": 128,
        "sp": 65534,
        "...": "..."
      }
    }
  ]
}
```

## Analyzing Execution Traces

The trace contains detailed information about each executed instruction, including:
- Program counter (PC) at execution time
- Disassembled instruction
- Register state before execution
- Timing information

To analyze:
1. Look for patterns in the instruction execution sequence
2. Identify where the execution diverges from expected behavior
3. Examine register states to understand emulator state
4. Track program flow to identify loop conditions or branching issues

## Using with Standard Tools

You can process trace files using standard JSON tools:

```bash
# Count total trace entries
jq '.entries | length' trace.json

# Extract only instruction execution events
jq '.entries[] | select(.name == "EXECUTE_INSTRUCTION")' trace.json

# Find specific instructions (e.g., jumps)
jq '.entries[] | select(.fields.instruction | contains("JP") or contains("JR"))' trace.json

# Filter by specific PC addresses
jq '.entries[] | select(.fields.pc == 43690)' trace.json
```

## Common Debugging Workflows

### Identifying Infinite Loops
Look for repeating instruction sequences or program counters:

```bash
jq '.entries[].fields.pc' trace.json | uniq -c | sort -nr | head -20
```

### Memory Access Issues
Filter for memory read/write operations (addresses in specific ranges like 0xFF00-0xFFFF):

```bash
jq '.entries[] | select(.fields.pc >= 65280 and .fields.pc <= 65535)' trace.json
```

### Register State Changes
Monitor specific register changes over time:

```bash
jq '.entries[] | {"timestamp": .timestamp, "pc": .fields.pc, "a": .fields.a, "f": .fields.f}' trace.json
```

## Performance Considerations

- Trace files can become large, especially for long-running tests
- Only traces for failing tests are preserved to maintain performance
- Use trace filtering to focus on relevant execution periods