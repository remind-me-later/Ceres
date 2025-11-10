# Change: Add Disassembler CLI Integration

## Why

This is **Part 2** of the `add-sm83-disassembler` parent proposal.

With the core disassembler complete (`add-disassembler-core`), we can now integrate it into the emulator for runtime
debugging. The MBC3 test ROM is failing, and we need to see what instructions execute to understand where bank switching
goes wrong.

This change adds execution tracing that logs each instruction as it runs, showing the program counter, mnemonic, and
register state.

## What Changes

- Add `Gb::disasm_at(addr: u16)` method to disassemble from emulator memory
- Add `--trace` CLI flag to all frontends (winit, gtk, egui)
- Modify `Gb::run_cpu()` to optionally log instructions
- Format trace output: `[PC:$1234] LD A, ($FF44) ; A=00 F=Z--- BC=0000 DE=0000 HL=0000 SP=FFFE`
- Performance guard: single branch check when tracing disabled
- Update CLI help text to document `--trace` flag

## Impact

- Affected specs: `disassembler-cli` (new capability)
- Affected code:
  - `ceres-core/src/lib.rs` (`Gb::disasm_at()` method)
  - `ceres-std/src/cli.rs` (add `--trace` flag)
  - `ceres-core/src/sm83.rs` (add trace logging in `run_cpu()`)
  - `ceres-winit/src/main.rs` (pass flag to emulator)
  - `ceres-gtk/src/main.rs` (pass flag to emulator)
  - `ceres-egui/src/main.rs` (pass flag to emulator)
- No breaking changes to existing API
- Minimal performance impact (<1% overhead when disabled)
- ~20-40% slowdown when tracing enabled (acceptable for debugging)

## Dependencies

This proposal depends on:

- **add-disassembler-core**: Must be complete before CLI integration can begin
- **add-disassembler-cli**: Must be complete before trace collection can be implemented

**Blocks**: `add-mbc3-tester-test` - The failing MBC3 test is waiting for this debugging capability

**Future Enhancement**: `add-trace-collection` - Proposed follow-up that adds programmatic trace access for AI agents

## AI Agent Debugging Considerations

This implementation provides **basic** AI agent debugging support through parseable stdout output. For more advanced
autonomous debugging, consider these future enhancements:

### Current Capabilities (This Proposal)

✅ AI agents can:

- Capture stdout traces to files for analysis
- Parse structured format: `[PC:$XXXX] INSTRUCTION ; REGISTERS`
- Search for specific instruction sequences or register patterns
- Correlate execution with source code (if available)

### Future Enhancements for Better AI Support

For true autonomous debugging, future proposals should add:

1. **Programmatic trace access** - API to get trace entries without parsing stdout
2. **Trace buffering** - Circular buffer of last N instructions for post-mortem analysis
3. **JSON export** - Structured format for ML analysis or comparison tools
4. **Memory access tracking** - Record what each instruction read/wrote
5. **Query interface** - "Show all writes to MBC bank register ($2000-$3FFF)"

Example workflow an AI agent could use **today**:

```bash
# Capture trace
cargo run -- --trace rom.gb 2>&1 | tee trace.log

# Agent analyzes with tools
grep "LD \[\$2" trace.log  # Find MBC bank switches
grep "PC:\$0150" trace.log # Find execution at specific address
diff -u expected_trace.log trace.log # Compare with known-good trace
```

With future enhancements, agents could query programmatically:

```rust
// Hypothetical future API
let trace = gb.get_last_n_instructions(1000);
let bank_switches = trace.iter()
    .filter(|e| e.memory_writes.iter().any(|(addr, _)| (0x2000..=0x3FFF).contains(addr)))
    .collect::<Vec<_>>();
```
