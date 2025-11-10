# Change: Add Core Disassembler

## Why

This is **Part 1** of the `add-sm83-disassembler` parent proposal.

The MBC3 bank switching needs debugging, but we lack tools to see what instructions are executing. A disassembler
converts binary opcodes to readable assembly, enabling developers to understand ROM behavior and correlate it with
source code.

This change adds the foundational disassembly capability to `ceres-core` that will later be used for CLI tracing.

## What Changes

- Add `heapless` dependency to `ceres-core` for stack-allocated strings
- New `ceres-core/src/disasm/mod.rs` module with `no_std` compatible disassembler
- Decode all 256 base SM83 opcodes (0x00-0xFF)
- Decode all 256 CB-prefixed opcodes (0xCB 0x00-0xCB 0xFF)
- Format output using RGBDS assembler syntax
- Return instruction mnemonic and byte length
- Zero heap allocation design
- Unit tests for all instruction categories

## Impact

- Affected specs: `disassembler-core` (new capability)
- Affected code:
  - `ceres-core/Cargo.toml` (add heapless dependency)
  - `ceres-core/src/disasm/mod.rs` (new module)
  - `ceres-core/src/lib.rs` (expose disasm module)
- No breaking changes
- No impact on emulation performance (feature is independent)
- Maintains `no_std` compatibility
