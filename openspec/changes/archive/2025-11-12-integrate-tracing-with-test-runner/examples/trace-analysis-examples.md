# Trace Analysis Examples for Humans and AI Agents

## Overview

This document provides practical examples of how to analyze emulator traces using both human-friendly and
machine-friendly approaches.

## Trace Format Examples

### JSON Lines Format (Machine-Friendly)

Each line is a complete JSON object representing a single trace entry:

```jsonl
{"target":"cpu_execution","level":"TRACE","timestamp":1762947510816,"pc":502,"instruction":"DEC C","a":2,"f":64,"b":203,"c":242,"d":152,"e":16,"h":50,"l":141,"sp":57343,"cycles":1}
{"target":"cpu_execution","level":"TRACE","timestamp":1762947510817,"pc":503,"instruction":"JR NZ, $01F7","a":2,"f":64,"b":203,"c":241,"d":152,"e":16,"h":50,"l":141,"sp":57343,"cycles":3}
{"target":"cpu_execution","level":"TRACE","timestamp":1762947510820,"pc":496,"instruction":"LD A, [HL+]","a":0,"f":64,"b":203,"c":241,"d":152,"e":16,"h":50,"l":141,"sp":57343,"cycles":2}
```

**Benefits for AI Agents**:

- Each line is independently parseable
- Easy to stream process large files
- Simple to filter with standard tools
- Minimal memory footprint for processing

### Structured JSON Format (Human-Friendly)

Complete trace with metadata wrapper:

```json
{
  "metadata": {
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
    "truncated": false
  },
  "entries": [
    {
      "target": "cpu_execution",
      "level": "TRACE",
      "timestamp": 1762947510816,
      "pc": 502,
      "instruction": "DEC C",
      "registers": {
        "a": 2,
        "f": 64,
        "b": 203,
        "c": 242,
        "d": 152,
        "e": 16,
        "h": 50,
        "l": 141,
        "sp": 57343
      },
      "cycles": 1
    }
  ]
}
```

## Common Analysis Patterns

### Pattern 1: Find First Divergence Between Traces

**Use Case**: Compare a passing and failing test to find where execution diverged.

**Shell Command** (using jq):

```bash
# Extract PC values from both traces
jq -r '.pc' passing.jsonl > passing_pcs.txt
jq -r '.pc' failing.jsonl > failing_pcs.txt

# Find first difference
diff -u passing_pcs.txt failing_pcs.txt | head -20
```

**Python Script**:

```python
import json

def find_divergence(passing_file, failing_file, context=10):
    with open(passing_file) as pf, open(failing_file) as ff:
        for i, (p_line, f_line) in enumerate(zip(pf, ff)):
            p_entry = json.loads(p_line)
            f_entry = json.loads(f_line)

            if p_entry['pc'] != f_entry['pc']:
                print(f"Divergence at instruction {i}:")
                print(f"  Passing: PC={p_entry['pc']:04X} {p_entry['instruction']}")
                print(f"  Failing: PC={f_entry['pc']:04X} {f_entry['instruction']}")
                return i
    return None
```

**AI Agent Prompt**:

```text
Analyze these two trace files and identify where execution diverged:
- passing_trace.jsonl (test passed)
- failing_trace.jsonl (test failed)

For each trace entry, you have: pc, instruction, registers (a,f,b,c,d,e,h,l,sp), cycles.
Find the first instruction where the PC differs and provide context about register states.
```

### Pattern 2: Detect Infinite Loops

**Use Case**: Test times out, need to identify if it's stuck in a loop.

**Shell Command**:

```bash
# Find most frequently executed PC values
jq -r '.pc' trace.jsonl | sort | uniq -c | sort -rn | head -10
```

**Python Script**:

```python
import json
from collections import defaultdict

def detect_loop(trace_file, threshold=100):
    pc_sequence = []
    pc_counts = defaultdict(int)

    with open(trace_file) as f:
        for line in f:
            entry = json.loads(line)
            pc = entry['pc']
            pc_sequence.append(pc)
            pc_counts[pc] += 1

    # Find PCs executed more than threshold times
    hot_pcs = [(pc, count) for pc, count in pc_counts.items() if count > threshold]

    if hot_pcs:
        print("Potential infinite loop detected:")
        for pc, count in sorted(hot_pcs, key=lambda x: x[1], reverse=True)[:5]:
            print(f"  PC 0x{pc:04X} executed {count} times")

            # Find the loop region
            loop_pcs = set()
            in_loop = False
            for p in pc_sequence:
                if p == pc:
                    in_loop = True
                elif in_loop:
                    loop_pcs.add(p)
                    if p == pc:
                        break

            print(f"    Loop region: {[f'0x{p:04X}' for p in sorted(loop_pcs)]}")
```

**AI Agent Prompt**:

```text
This trace is from a test that timed out. Analyze the trace to determine if the emulator
is stuck in an infinite loop.

Look for:
1. PC values that are executed repeatedly (>100 times)
2. Sequences of instructions that repeat
3. Register values that aren't changing during the loop

Provide the PC range of the loop and suggest what the code might be waiting for.
```

### Pattern 3: Track Memory Write Patterns

**Use Case**: Debugging PPU rendering or memory corruption issues.

**Shell Command**:

```bash
# Find all writes to VRAM region (0x8000-0x9FFF)
# Assuming we enhance traces to include memory operations
jq -r 'select(.memory_write and .address >= 32768 and .address < 40960) |
       "\(.timestamp) Write to 0x\(.address|tostring|ascii_upcase) = 0x\(.value|tostring|ascii_upcase)"' \
       trace.jsonl
```

**Python Script**:

```python
import json

def analyze_memory_pattern(trace_file, start_addr, end_addr):
    """Analyze memory writes to a specific address range."""
    writes = []

    with open(trace_file) as f:
        for line in f:
            entry = json.loads(line)
            # This assumes enhanced trace format with memory operations
            if 'memory_write' in entry:
                addr = entry['address']
                if start_addr <= addr < end_addr:
                    writes.append({
                        'timestamp': entry['timestamp'],
                        'pc': entry['pc'],
                        'address': addr,
                        'value': entry['value'],
                        'instruction': entry.get('instruction', 'unknown')
                    })

    # Analyze patterns
    print(f"Memory writes to range 0x{start_addr:04X}-0x{end_addr:04X}:")
    print(f"  Total writes: {len(writes)}")

    # Group by instruction type
    by_instruction = defaultdict(int)
    for w in writes:
        by_instruction[w['instruction']] += 1

    print("\n  Most common instructions:")
    for inst, count in sorted(by_instruction.items(), key=lambda x: x[1], reverse=True)[:5]:
        print(f"    {inst}: {count} times")
```

### Pattern 4: Register State Timeline

**Use Case**: Track how a specific register changes over time.

**Shell Command**:

```bash
# Track register A changes
jq -r '"\(.pc|tostring|ascii_downcase) \(.instruction) A=\(.a|tostring|ascii_downcase)"' trace.jsonl | head -20
```

**Python Script**:

```python
import json

def track_register(trace_file, register='a', show_changes_only=True):
    """Track a specific register's value over time."""
    prev_value = None

    with open(trace_file) as f:
        for i, line in enumerate(f):
            entry = json.loads(line)
            value = entry.get(register.lower())

            if value is None:
                continue

            if show_changes_only:
                if value != prev_value:
                    print(f"[{i:6d}] PC 0x{entry['pc']:04X}: {entry['instruction']:20s} "
                          f"{register.upper()}={prev_value or 0:02X} -> {value:02X}")
                    prev_value = value
            else:
                print(f"[{i:6d}] PC 0x{entry['pc']:04X}: {register.upper()}={value:02X}")
```

### Pattern 5: Instruction Frequency Analysis

**Use Case**: Understand what the ROM is doing most frequently.

**Shell Command**:

```bash
# Top 10 most executed instructions
jq -r '.instruction' trace.jsonl | sort | uniq -c | sort -rn | head -10
```

**Python Script**:

```python
import json
from collections import Counter

def instruction_histogram(trace_file, top_n=20):
    """Generate histogram of instruction execution."""
    instructions = Counter()

    with open(trace_file) as f:
        for line in f:
            entry = json.loads(line)
            instructions[entry['instruction']] += 1

    print(f"Top {top_n} most executed instructions:")
    for inst, count in instructions.most_common(top_n):
        print(f"  {count:6d}x  {inst}")

    return instructions
```

## AI Agent Integration Examples

### Example 1: LLM-Based Trace Analysis

**Prompt Template**:

```markdown
I have a Game Boy emulator trace from a failing test. Here's a sample of the trace data:

[Provide first 50 lines of JSONL trace]

The test failed because: [failure reason]

Please analyze this trace and:

1. Identify any suspicious patterns (infinite loops, unusual register states, etc.)
2. Suggest what the emulator might be doing wrong
3. Recommend specific areas to investigate

Trace schema:

- pc: Program counter (u16)
- instruction: Disassembled instruction (string)
- a,f,b,c,d,e,h,l: CPU registers (u8)
- sp: Stack pointer (u16)
- cycles: Instruction cycle count (u8)
```

### Example 2: Automated Pattern Detection

**Script for AI Agent**:

```python
import json
import sys

def analyze_trace_for_agent(trace_file):
    """
    Automated analysis for AI agent consumption.
    Returns structured findings in JSON format.
    """
    findings = {
        "loops": [],
        "anomalies": [],
        "statistics": {},
        "recommendations": []
    }

    pc_frequency = Counter()
    instruction_frequency = Counter()
    register_changes = {reg: 0 for reg in 'afbcdehl'}
    prev_registers = None

    with open(trace_file) as f:
        for i, line in enumerate(f):
            entry = json.loads(line)
            pc_frequency[entry['pc']] += 1
            instruction_frequency[entry['instruction']] += 1

            # Track register changes
            if prev_registers:
                for reg in 'afbcdehl':
                    if entry[reg] != prev_registers[reg]:
                        register_changes[reg] += 1

            prev_registers = entry

    # Detect loops (PC executed >100 times)
    for pc, count in pc_frequency.items():
        if count > 100:
            findings["loops"].append({
                "pc": f"0x{pc:04X}",
                "execution_count": count
            })

    # Statistics
    findings["statistics"] = {
        "total_instructions": sum(instruction_frequency.values()),
        "unique_pcs": len(pc_frequency),
        "most_common_instruction": instruction_frequency.most_common(1)[0][0],
        "register_volatility": register_changes
    }

    # Recommendations based on findings
    if findings["loops"]:
        findings["recommendations"].append(
            "Infinite loop detected - check interrupt handling or wait loop conditions"
        )

    if register_changes['a'] < 10:
        findings["recommendations"].append(
            "Register A barely changes - possible stuck in initialization or wait loop"
        )

    return json.dumps(findings, indent=2)

if __name__ == "__main__":
    if len(sys.argv) != 2:
        print("Usage: python analyze_for_agent.py <trace.jsonl>")
        sys.exit(1)

    result = analyze_trace_for_agent(sys.argv[1])
    print(result)
```

### Example 3: Trace Diff for AI Agents

**Output Format**:

```json
{
  "divergence_point": {
    "instruction_index": 1523,
    "passing": {
      "pc": "0x0156",
      "instruction": "LD A, [HL]",
      "registers": { "a": 0, "f": 128, "h": 195, "l": 0 }
    },
    "failing": {
      "pc": "0x015A",
      "instruction": "JP 0x0200",
      "registers": { "a": 0, "f": 128, "h": 195, "l": 4 }
    }
  },
  "context_before": [
    { "pc": "0x0150", "instruction": "LD HL, $C300" },
    { "pc": "0x0153", "instruction": "LD B, $10" }
  ],
  "analysis": {
    "likely_cause": "Register L has different value (0 vs 4), causing different memory read",
    "affected_registers": ["l"],
    "severity": "high"
  }
}
```

## Performance Considerations

### Streaming Analysis for Large Traces

```python
def stream_analyze(trace_file, analysis_func):
    """
    Analyze trace file in streaming fashion to handle large files.
    analysis_func receives one entry at a time.
    """
    with open(trace_file) as f:
        for i, line in enumerate(f):
            entry = json.loads(line)
            analysis_func(i, entry)

            # Progress indicator every 10k entries
            if i % 10000 == 0:
                print(f"Processed {i} entries...", file=sys.stderr)
```

### Compressed Traces

```bash
# Traces can be gzip compressed for storage
gzip trace.jsonl  # Creates trace.jsonl.gz

# Analysis still works with compression
zcat trace.jsonl.gz | jq -r '.pc' | sort | uniq -c | sort -rn | head -10
```

## Schema Documentation for AI Agents

### Trace Entry Schema

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "Ceres Emulator Trace Entry",
  "type": "object",
  "required": ["target", "level", "timestamp", "pc", "instruction"],
  "properties": {
    "target": {
      "type": "string",
      "description": "Trace event target (e.g., 'cpu_execution', 'ppu', 'apu')",
      "examples": ["cpu_execution", "ceres_core::ppu"]
    },
    "level": {
      "type": "string",
      "enum": ["TRACE", "DEBUG", "INFO", "WARN", "ERROR"],
      "description": "Logging level of the event"
    },
    "timestamp": {
      "type": "integer",
      "description": "Unix timestamp in milliseconds",
      "minimum": 0
    },
    "pc": {
      "type": "integer",
      "description": "Program counter (0x0000-0xFFFF)",
      "minimum": 0,
      "maximum": 65535
    },
    "instruction": {
      "type": "string",
      "description": "Disassembled instruction",
      "examples": ["LD A, B", "JP 0x0150", "CALL 0x2000"]
    },
    "a": { "type": "integer", "minimum": 0, "maximum": 255 },
    "f": { "type": "integer", "minimum": 0, "maximum": 255 },
    "b": { "type": "integer", "minimum": 0, "maximum": 255 },
    "c": { "type": "integer", "minimum": 0, "maximum": 255 },
    "d": { "type": "integer", "minimum": 0, "maximum": 255 },
    "e": { "type": "integer", "minimum": 0, "maximum": 255 },
    "h": { "type": "integer", "minimum": 0, "maximum": 255 },
    "l": { "type": "integer", "minimum": 0, "maximum": 255 },
    "sp": {
      "type": "integer",
      "description": "Stack pointer",
      "minimum": 0,
      "maximum": 65535
    },
    "cycles": {
      "type": "integer",
      "description": "Number of cycles the instruction took",
      "minimum": 1,
      "maximum": 24
    }
  }
}
```

This schema enables AI agents to:

- Validate trace data structure
- Generate type-safe parsers
- Understand value ranges for anomaly detection
- Build automated analysis tools
