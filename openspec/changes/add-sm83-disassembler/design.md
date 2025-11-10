# Design: SM83 Disassembler

## Context

The Game Boy CPU (SM83) has a relatively simple instruction set derived from the Z80:

- 256 base opcodes (0x00-0xFF)
- 256 CB-prefixed opcodes (0xCB 0x00 through 0xCB 0xFF)
- Variable instruction lengths: 1, 2, or 3 bytes
- Immediate values are little-endian for 16-bit operands

The disassembler must work in `no_std` environments since `ceres-core` is platform-agnostic.

## Goals / Non-Goals

**Goals:**

- Decode all 512 opcodes into assembly mnemonics
- Support both standalone disassembly and runtime tracing
- Maintain `no_std` compatibility
- Zero-allocation design where practical
- Readable output format matching RGBDS/mgbdis syntax

**Non-Goals:**

- Interactive debugging UI (deferred to future proposal)
- Performance optimization (disassembly is for debugging only)
- Symbol resolution or label generation
- Multi-instruction analysis or control flow graphing

## Decisions

### 1. Module Structure

**Decision:** Create `ceres-core/src/disasm/mod.rs` as a new module

**Rationale:**

- Keeps disassembly logic isolated from emulation
- Easy to feature-gate if needed in the future
- Follows existing module pattern (apu/, ppu/, etc.)

### 2. API Design

**Public API:**

```rust
// In ceres-core/src/disasm/mod.rs
pub struct DisasmResult {
    pub mnemonic: heapless::String<32>,  // e.g., "LD A, B"
    pub length: u8,                      // instruction length in bytes (1-3)
}

pub fn disasm(bytes: &[u8], pc: u16) -> DisasmResult;
pub fn disasm_cb(opcode: u8) -> DisasmResult;

// In ceres-core/src/lib.rs (Gb impl)
impl<A: AudioCallback> Gb<A> {
    pub fn disasm_at(&mut self, addr: u16) -> DisasmResult { ... }
}
```

**Rationale:**

- `heapless::String` provides stack-allocated strings for `no_std`
- 32 bytes is sufficient for longest instruction: `CALL NZ, $1234` is 14 chars
- Separate `disasm` (takes bytes) and `disasm_at` (reads from emulator memory)
- Returns length so caller knows how many bytes to skip
- `pc` parameter allows showing jump/call targets as absolute addresses

**Alternative considered:** Returning `&'static str`

- Rejected: Would require compile-time string allocation or string interning
- Rejected: Less flexible for including immediate values in output

### 3. Output Format

**Decision:** Match RGBDS assembler syntax (v1.0.0 specification)

**Examples:**

```asm
NOP
LD A, B
LD A, [$FF44]
LD [HL], $42
JP $1234
BIT 7, (HL)
CALL NZ, $0150
LDH A, [$FF00]
JR NZ, $10
ADD HL, BC
```

**Rationale:**

- RGBDS is the standard toolchain for Game Boy development (rgbasm, rgblink)
- Source code for test ROMs uses RGBDS syntax
- Consistency aids debugging by matching available documentation
- Reference: https://rgbds.gbdev.io/docs/v1.0.0/gbz80.7

**Key RGBDS Syntax Rules:**

- Memory access uses brackets: `[HL]`, `[$FF44]`, `[BC]`
- Immediates use `$` prefix: `$FF`, `$1234`
- Registers: `A`, `B`, `C`, `D`, `E`, `H`, `L`, `BC`, `DE`, `HL`, `SP`, `AF`
- Conditions: `NZ`, `Z`, `NC`, `C`
- High-page loads: `LDH A, [$FF00]` or `LD A, [$FF00+C]`
- Relative jumps take signed 8-bit offset: `JR $10` (shown as address, not offset)
- CB-prefix instructions: `BIT u3, r8`, `SET u3, r8`, `RES u3, r8`, `SWAP r8`

### 4. Immediate Value Formatting

**Decision:** Use `$` prefix for hex values, uppercase hex digits

**Examples:**

- 8-bit: `$FF`, `$42`, `$00`
- 16-bit: `$1234`, `$ABCD`, `$0150`
- Addresses: `[$1234]`, `[$FF44]`
- High-page: `[$FF00]` (8-bit offset from $FF00)

**Rationale:**

- RGBDS convention uses `$` for hex (not `0x`)
- Uppercase hex digits are standard in Game Boy development
- Distinct from decimal values
- Easier to correlate with memory addresses
- Matches Pan Docs and other GB documentation

### 5. Instruction Length Calculation

**Decision:** Calculate length inline during disassembly

**Rationale:**

- Simple lookup table approach
- No separate length-only API needed
- Minimal code duplication

### 6. CB-Prefix Handling

**Decision:** Decode CB opcodes separately but expose unified API

**Rationale:**

- CB instructions have different decode logic (bit operations)
- Internal separation keeps code clean
- External API is unified for simplicity

### 7. Zero-Allocation Strategy

**Decision:** Use `heapless::String<32>` for output

**Rationale:**

- Stack-allocated, `no_std` compatible
- 32 bytes sufficient for all instructions + formatting
- Dependency already used in other parts of project (or add if needed)

**Alternative:** Write to `fmt::Write` trait

- Rejected: More complex API, caller must provide buffer
- Rejected: Current approach is simpler for both implementer and user

## Implementation Strategy

### Phase 1: Core Disassembler (add-disassembler-core)

1. Add `heapless` dependency to `ceres-core/Cargo.toml` if not present
2. Create `ceres-core/src/disasm/mod.rs`
3. Implement `disasm()` for base opcodes with match statements
4. Implement `disasm_cb()` for CB-prefixed opcodes
5. Add unit tests with known opcodes
6. Validate against test ROMs' expected disassembly

### Phase 2: CLI Integration (add-disassembler-cli)

1. Add `Gb::disasm_at()` method
2. Add `--trace` CLI flag to `ceres-std/src/cli.rs`
3. Modify `Gb::run_cpu()` to conditionally log instructions
4. Format output: `[PC:$1234] LD A, ($FF44) ; A=00 F=Z--- BC=0000 DE=0000 HL=0000 SP=FFFE`
5. Test with mbc3-tester ROM

## Testing Strategy

### Unit Tests

Test each category of instructions:

- Data transfer (LD)
- Arithmetic (ADD, SUB, INC, DEC)
- Logical (AND, OR, XOR, CP)
- Control flow (JP, JR, CALL, RET, RST)
- Bit operations (BIT, SET, RES)
- Rotate/shift (RLC, RRC, RL, RR, SLA, SRA, SRL, SWAP)

### Integration Tests

- Disassemble known sections of test ROMs
- Compare output with mgbdis or manual disassembly
- Verify all 512 opcodes are covered

## Performance Considerations

- Disassembly only runs when debugging is enabled
- Single branch check in hot path: `if self.debug_enabled { ... }`
- No measurable performance impact when disabled
- When enabled, expect ~10-20% slowdown (acceptable for debugging)

## Open Questions

None. Design is straightforward and well-understood.

## AI Agent Debugging Support

### Current Design Assessment

**Strengths for AI agents:**

- Structured trace format is parseable
- RGBDS syntax matches documentation
- Real-time execution visibility

**Gaps for autonomous debugging:**

- Output is stdout-only (not programmatically accessible)
- No trace history collection or replay
- Missing memory access tracking
- No query API for "what happened at address X?"

### Recommended Enhancements (Future Proposals)

To enable AI agents to autonomously debug issues, consider these extensions:

#### 1. Trace Collection API (`add-trace-collection`)

```rust
pub struct TraceEntry {
    pub pc: u16,
    pub bank: u8,
    pub opcode: u8,
    pub mnemonic: String,
    pub registers: CpuState,
    pub memory_reads: Vec<(u16, u8)>,
    pub memory_writes: Vec<(u16, u8)>,
    pub cycles: u64,
}

impl Gb {
    pub fn enable_trace_collection(&mut self, max_entries: usize);
    pub fn get_trace_buffer(&self) -> &[TraceEntry];
    pub fn export_trace_json(&self) -> String;
}
```

#### 2. Memory Access Tracking (`add-memory-tracking`)

- Track all reads/writes with source instruction PC
- Identify "when was address $FF44 last written?"
- Detect unexpected writes or access patterns

#### 3. Condition-Based Breakpoints (`add-conditional-breakpoints`)

```rust
pub enum BreakCondition {
    PcEquals(u16),
    MemoryWrite(u16),
    RegisterEquals(Register, u8),
    Custom(Box<dyn Fn(&CpuState) -> bool>),
}
```

#### 4. Execution Replay (`add-trace-replay`)

- Save trace to file for later analysis
- Replay execution from saved trace
- Compare executions between working/broken states

These enhancements would enable AI agents to:

- Analyze execution without parsing stdout
- Query specific events: "Show me all writes to $2000-$3FFF" (MBC bank switches)
- Compare traces: "What's different between this run and the reference?"
- Set targeted breakpoints: "Stop when A==$FF after PC==$0150"

## Future Extensions

Deferred to future proposals:

- **add-trace-collection** (created): Programmatic trace capture for AI analysis with circular buffer, query API, and
  JSON export
- **add-memory-tracking**: Track all memory accesses with instruction context
- **add-conditional-breakpoints**: Break on complex conditions
- **add-trace-replay**: Record and replay execution for comparison
- **add-interactive-debugger**: Interactive debugger with breakpoints
- Symbol table support for better output
- Saving/loading debug sessions
