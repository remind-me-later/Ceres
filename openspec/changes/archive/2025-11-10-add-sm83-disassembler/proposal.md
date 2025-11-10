# Change: Add SM83 Disassembler

## Why

The `add-mbc3-tester-test` integration test proposal is blocked because the MBC3 emulation appears to have issues. The
test ROM source code is available in disassembled form, which would allow manual inspection of what the ROM is doing.
However, without a disassembler, debugging the MBC3 bank switching behavior is difficult because we cannot correlate the
ROM's expected behavior with what the emulator is actually executing.

A disassembler would enable:

- Step-by-step debugging of ROM execution by showing what instructions are being run
- Comparison between expected ROM behavior (from source) and actual execution
- Better understanding of where MBC bank switching fails
- General debugging capability for future issues

This change introduces a new `disassembler` capability that can decode SM83/Game Boy CPU instructions from binary into
human-readable assembly mnemonics.

## What Changes

This is a multi-part change divided into sub-proposals for better tracking:

### Part 1: Core Disassembler (add-disassembler-core)

- New `ceres-core::disasm` module with `no_std` compatible disassembler
- Instruction decoding for all 256 base opcodes and 256 CB-prefixed opcodes
- Format instructions as assembly strings (e.g., "LD A, B", "JP $1234", "BIT 7, (HL)")
- Support for immediate values (8-bit and 16-bit)
- Zero-allocation design using stack buffers for `no_std` compatibility

### Part 2: CLI Integration (add-disassembler-cli)

- New `ceres-core::Gb::disasm_at()` method to disassemble at any address
- CLI flag to enable instruction logging during emulation
- Format: `[PC:$1234] LD A, ($FF44) ; A=00 F=Z--- BC=0000 DE=0000 HL=0000 SP=FFFE`
- Helps debug by showing execution flow and register state

### Part 3: Interactive Debugger (add-interactive-debugger) - Optional/Future

- Step-by-step execution
- Breakpoints
- Memory inspection
- Register modification

**This proposal focuses on Parts 1 and 2**, which are sufficient for debugging the MBC3 issue. Part 3 is deferred as
it's not immediately needed.

## Impact

- Affected specs: `disassembler-core` (new), `disassembler-cli` (new)
- Affected code:
  - `ceres-core/src/disasm/` (new module)
  - `ceres-core/src/lib.rs` (expose disasm module, add `Gb::disasm_at()` method)
  - `ceres-std/src/cli.rs` (new flag `--disasm` or `--trace`)
  - `ceres-winit/src/main.rs` (integrate CLI flag)
  - `ceres-gtk/src/main.rs` (integrate CLI flag)
  - `ceres-egui/src/main.rs` (integrate CLI flag)
- No breaking changes to existing API
- `no_std` compatibility maintained in `ceres-core`
- Minimal performance impact when disabled (single branch check)

## Sub-Proposals

This change is intentionally split into two focused sub-proposals:

1. **add-disassembler-core**: Core disassembly capability (independent, `no_std`)
2. **add-disassembler-cli**: CLI integration for execution tracing (depends on #1)

Each sub-proposal has its own tasks, specs, and can be implemented and tested independently.
