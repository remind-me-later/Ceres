# Proposal: Add Execution Trace Collection

**Status**: Draft  
**Created**: 2025-11-10  
**Dependencies**: `add-disassembler-cli`

## Problem

The current `--trace` flag outputs disassembly to stdout, which is useful for human debugging but has limitations for
programmatic analysis:

- **No programmatic access**: AI agents and tools must parse stdout text
- **No filtering**: Cannot selectively capture specific instruction types or address ranges
- **No post-execution analysis**: Cannot analyze patterns across multiple frames
- **Performance overhead**: Writing every instruction to stdout is slow
- **Memory constraints**: Cannot replay execution sequences or export traces

AI agents need structured trace data to autonomously debug issues like:

- MBC banking errors (tracking bank switches and memory accesses)
- Timing issues (analyzing instruction sequences and cycle counts)
- PPU synchronization (correlating CPU execution with video timing)
- Interrupt handling (tracking interrupt enable/disable patterns)

## Solution

Add a trace collection system that captures execution history in memory with programmatic query API, enabling AI agents
to analyze emulator behavior without parsing text output.

**Key Features**:

1. **Circular Trace Buffer**: Ring buffer storing last N instructions (configurable size)
2. **Programmatic API**: Query traces by address range, instruction type, or register values
3. **Structured Output**: Export traces as JSON for analysis tools
4. **Minimal Overhead**: Only active when explicitly enabled
5. **Integration Points**: Accessible from frontends and test runner

## Implementation Strategy

### Phase 1: Core Trace Buffer (ceres-core)

Add circular buffer infrastructure in `ceres-core` for no_std compatibility:

```rust
pub struct TraceEntry {
    pub pc: u16,
    pub instruction: heapless::String<32>,
    pub cycles: u8,
    pub registers: RegisterSnapshot,
}

pub struct TraceBuffer {
    entries: heapless::Vec<TraceEntry, CAPACITY>,
    enabled: bool,
}
```

### Phase 2: Collection Integration (ceres-core)

Modify `sm83.rs` to capture traces during instruction execution:

- Hook into `run_cpu()` after instruction execution
- Reuse disassembly from `disasm_at()` method
- Store minimal data (PC, instruction, cycles, registers)

### Phase 3: Query API (ceres-core)

Provide filtering and search capabilities:

```rust
impl Gb {
    pub fn trace_filter(&self, predicate: impl Fn(&TraceEntry) -> bool) -> Vec<&TraceEntry>;
    pub fn trace_range(&self, start_pc: u16, end_pc: u16) -> Vec<&TraceEntry>;
    pub fn trace_last_n(&self, n: usize) -> &[TraceEntry];
}
```

### Phase 4: Export Functionality (ceres-std)

Add JSON export for analysis tools (standard library only):

```rust
pub fn export_trace_json(gb: &Gb) -> String;
```

### Phase 5: CLI Integration (frontends)

Add CLI flags for trace control:

- `--trace-buffer-size N`: Set circular buffer size (default 1000)
- `--trace-export FILE.json`: Export trace on exit
- `--trace-filter EXPR`: Only capture matching instructions

### Phase 6: Test Runner Integration (ceres-test-runner)

Enable trace collection during test execution for debugging failures:

```rust
pub fn run_test_with_trace(rom: &[u8]) -> (TestResult, TraceBuffer);
```

## Success Criteria

1. Trace buffer captures last 1000 instructions with <5% performance overhead
2. AI agents can query traces programmatically without parsing text
3. JSON export enables analysis with external tools (Python, jq, etc.)
4. Test runner can export traces for failed tests automatically
5. Documentation includes example agent debugging workflows

## Open Questions

1. **Buffer size**: What default capacity balances memory usage vs. history depth?
2. **Register snapshots**: Capture full register state (8 bytes) or minimal delta?
3. **Memory access tracking**: Should traces include memory read/write operations?
4. **Conditional collection**: Support runtime predicates to filter during collection?
5. **Integration with save states**: Should traces be included in BESS format?

## Risks

- **Memory usage**: Circular buffer consumes heap/stack space (mitigated by configurable size)
- **Performance**: Recording every instruction adds overhead (mitigated by optional enable flag)
- **no_std compatibility**: JSON export requires alloc (mitigated by ceres-std separation)
- **API complexity**: Query interface may be over-engineered (mitigated by iterative design)

## Alternatives Considered

1. **Keep stdout-only tracing**: Simple but limits programmatic analysis
2. **Full instruction logging to file**: Too slow and generates huge files
3. **External debugger protocol (GDB stub)**: Complex integration, overkill for this use case
4. **Event-based callbacks**: Flexible but adds complexity for simple use cases

## References

- Parent Proposal: `add-sm83-disassembler` - Design document with trace integration notes
- Dependency: `add-disassembler-cli` - Provides disasm_at() method for trace generation
- Related: `add-mbc3-tester-test` - Primary use case for trace-based debugging
- External: GDB Remote Serial Protocol - Reference for debugger integration patterns
