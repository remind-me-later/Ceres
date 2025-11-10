# trace-collection Specification

## Purpose

TBD - created by archiving change add-trace-collection. Update Purpose after archive.

## Requirements

### Requirement: Circular Trace Buffer (REQ-1)

**Priority**: MUST  
**Component**: ceres-core

The system MUST provide a circular buffer that stores the last N instruction executions:

- Buffer MUST use `heapless::Vec` for no_std compatibility
- Buffer MUST wrap around when full (FIFO behavior)
- Buffer MUST be disabled by default (zero overhead when not in use)
- Buffer capacity MUST be configurable at emulator initialization
- Buffer MUST support capacity from 1 to 100,000 entries

#### Scenario: Basic Storage

Execute 10 instructions, verify all 10 captured in buffer.

#### Scenario: Wraparound Behavior

Execute 1500 instructions with 1000 capacity buffer, verify last 1000 remain and older entries discarded.

#### Scenario: Disabled State

Execute instructions with disabled buffer, verify zero entries captured and no overhead.

#### Scenario: Clear Operation

Fill buffer with entries, call clear(), verify buffer is empty and ready for new entries.

### Requirement: Trace Entry Structure (REQ-2)

**Priority**: MUST  
**Component**: ceres-core

Each trace entry MUST capture complete instruction execution state:

- Program counter (PC) at instruction start
- Disassembled instruction string (max 32 bytes)
- Cycle count consumed by instruction
- Full CPU register state (A, F, B, C, D, E, H, L, SP)

**Rationale**: Complete state enables post-execution analysis without re-running emulation.

#### Scenario: Register Capture

Execute `LD A, $42`, verify trace entry shows A=42 in register snapshot after execution.

#### Scenario: Cycle Accuracy

Execute `NOP` instruction, verify trace entry shows cycles=4 (correct cycle count for NOP).

#### Scenario: PC Accuracy

Execute instruction at address 0x0150, verify trace entry shows pc=0x0150.

#### Scenario: CB-Prefixed Instructions

Execute CB-prefixed opcode (e.g., `BIT 7, H`), verify trace entry contains correct disassembly string.

### Requirement: Collection Performance (REQ-3)

**Priority**: MUST  
**Component**: ceres-core

Trace collection MUST have minimal performance impact:

- Overhead MUST be <5% when enabled (compared to no tracing)
- No overhead when disabled (conditional check only)
- Collection MUST NOT allocate heap memory in hot path
- Collection MUST NOT cause observable frame rate drops

#### Scenario: Disabled Overhead

Benchmark 1M instructions with disabled tracing, verify <1% overhead compared to no trace system.

#### Scenario: Enabled Overhead

Benchmark 1M instructions with enabled tracing, verify <5% slowdown compared to disabled tracing.

#### Scenario: Frame Time Maintenance

Run full frame (70224 cycles) with tracing enabled, verify 16.7ms budget maintained for 60 FPS.

### Requirement: Query API (REQ-4)

**Priority**: MUST  
**Component**: ceres-core

The system MUST provide programmatic query interface:

- `trace_entries() -> &[TraceEntry]` - Get all entries
- `trace_last_n(n: usize) -> &[TraceEntry]` - Get last N entries
- `trace_range(start: u16, end: u16) -> Vec<&TraceEntry>` - Filter by PC range
- `trace_filter(predicate: impl Fn(&TraceEntry) -> bool) -> Vec<&TraceEntry>` - Custom filter

#### Scenario: Range Query

Execute mixed instructions, query PC range [0x100..0x110], verify only entries within range returned.

#### Scenario: Last N Query

Execute 100 instructions, query last 10 entries, verify correct chronological subset returned.

#### Scenario: Custom Filter

Query all `LD` instructions using filter predicate, verify only load instructions returned.

#### Scenario: Empty Buffer Query

Query empty buffer with any API method, verify empty slice/vec returned without errors.

### Requirement: JSON Export (REQ-5)

**Priority**: MUST  
**Component**: ceres-std

The system MUST export traces as structured JSON:

- Export format MUST be valid JSON
- Export MUST include metadata: timestamp, entry count, buffer capacity
- Each entry MUST include all fields from TraceEntry
- Export MUST be callable from CLI and test runner
- Export file size MUST be reasonable (<100MB for 100k entries)

#### Scenario: Valid JSON Output

Export trace, parse result with `serde_json::from_str()`, verify no parse errors.

#### Scenario: Field Completeness

Export trace with single entry, verify all TraceEntry fields present in JSON (pc, instruction, cycles, registers).

#### Scenario: Metadata Section

Export trace, verify metadata section exists with correct timestamp, entry_count, and buffer_capacity.

#### Scenario: File I/O

Export trace to file path, verify file created and contains valid parseable JSON.

**Example JSON Format**:

```json
{
  "metadata": {
    "timestamp": "2025-11-10T12:34:56Z",
    "buffer_capacity": 1000,
    "entry_count": 156
  },
  "entries": [
    {
      "pc": "0x0100",
      "instruction": "NOP",
      "cycles": 4,
      "registers": {
        "a": 1,
        "f": 176,
        "b": 0,
        "c": 19,
        "d": 0,
        "e": 216,
        "h": 1,
        "l": 77,
        "sp": 65534
      }
    }
  ]
}
```

### Requirement: CLI Integration (REQ-6)

**Priority**: MUST  
**Component**: ceres-winit, ceres-gtk, ceres-egui

The CLI MUST support trace collection flags:

- `--trace-buffer-size N` - Set buffer capacity (default: 1000)
- `--trace-export FILE` - Export trace to JSON file on exit
- `--trace-enable` - Enable tracing from start
- Flags MUST be consistent across all frontends

#### Scenario: Buffer Size Configuration

Launch emulator with `--trace-buffer-size 500`, verify buffer capacity set to 500 entries.

#### Scenario: Export on Exit

Launch with `--trace-export out.json`, run emulation, exit, verify JSON file created at specified path.

#### Scenario: Auto-Enable Tracing

Launch with `--trace-enable`, verify tracing starts immediately without additional API calls.

#### Scenario: Invalid Size Error Handling

Provide invalid buffer size (0 or negative), verify graceful error message displayed and emulator doesn't crash.

### Requirement: Test Runner Integration (REQ-7)

**Priority**: MUST  
**Component**: ceres-test-runner

The test runner MUST export traces on test failure:

- Failed tests MUST automatically export trace to `target/traces/<test_name>.json`
- Test output MUST log trace file path
- Traces MUST be optional for passing tests (disabled by default)

#### Scenario: Automatic Failure Export

Run failing integration test, verify trace JSON automatically created in `target/traces/<test_name>.json`.

#### Scenario: Path Logging

Run failing test, check console output, verify trace file path printed to stdout for easy access.

#### Scenario: Passing Tests No Export

Run passing integration test, verify no trace file exported by default (only failures export traces).
