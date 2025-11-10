# Disassembler Core Capability

## ADDED Requirements

### Requirement: SM83 Instruction Decoding

The system SHALL decode SM83 CPU instructions from binary opcodes into human-readable assembly mnemonics following RGBDS
assembler syntax.

#### Scenario: Decode base opcode

- **WHEN** `disasm()` is called with bytes `[0x3E, 0xFF]` at PC $0150
- **THEN** the result mnemonic is `"LD A, $FF"`
- **AND** the result length is 2 bytes

#### Scenario: Decode CB-prefixed opcode

- **WHEN** `disasm()` is called with bytes `[0xCB, 0x7E]` at any PC
- **THEN** the result mnemonic is `"BIT 7, (HL)"`
- **AND** the result length is 2 bytes

#### Scenario: Decode 16-bit immediate

- **WHEN** `disasm()` is called with bytes `[0xCD, 0x50, 0x01]` at PC $0000
- **THEN** the result mnemonic is `"CALL $0150"`
- **AND** the result length is 3 bytes
- **AND** the immediate value is formatted as little-endian ($0150, not $5001)

#### Scenario: Decode all 256 base opcodes

- **WHEN** every opcode from $00 to $FF is disassembled
- **THEN** each returns a valid mnemonic string
- **AND** no opcode panics or returns empty string
- **AND** illegal opcodes are formatted as `"ILLEGAL $XX"`

#### Scenario: Decode all 256 CB opcodes

- **WHEN** every CB-prefixed opcode from $CB00 to $CBFF is disassembled
- **THEN** each returns a valid mnemonic string
- **AND** no opcode panics or returns empty string

### Requirement: No-Std Compatibility

The disassembler SHALL work in `no_std` environments without heap allocation.

#### Scenario: Zero heap allocations

- **WHEN** any disassembly function is called
- **THEN** no heap allocations occur
- **AND** all strings are stack-allocated using `heapless::String<32>`

#### Scenario: Works without std library

- **WHEN** `ceres-core` is compiled with `#![no_std]`
- **THEN** the disasm module compiles without errors
- **AND** all functionality remains available

### Requirement: RGBDS Syntax Compatibility

The disassembler SHALL format instructions using RGBDS assembler syntax conventions.

#### Scenario: Hex values use dollar-sign prefix

- **WHEN** an instruction with immediate values is disassembled
- **THEN** hex values use `$` prefix (e.g., `$FF`, `$1234`)
- **AND** values are uppercase hex digits (e.g., `$FF`, not `$ff`)

#### Scenario: Memory access uses square brackets

- **WHEN** an instruction accesses memory indirectly is disassembled
- **THEN** the address register is wrapped in square brackets per RGBDS syntax (e.g., `"LD A, [HL]"`, `"LD [BC], A"`)

#### Scenario: High-page access format

- **WHEN** instructions $E0, $E2, $F0, $F2 are disassembled
- **THEN** they format as `"LDH [$FFxx], A"` or `"LDH A, [$FFxx]"` per RGBDS v1.0.0 syntax
- **AND** the high-page offset is shown as 8-bit value
- **AND** $E2 formats as `"LD [$FF00+C], A"`and $F2 as`"LD A, [$FF00+C]"`

#### Scenario: Conditional suffixes

- **WHEN** conditional instructions (JP, JR, CALL, RET) are disassembled
- **THEN** conditions are formatted as two-letter suffixes: `NZ`, `Z`, `NC`, `C`
- **AND** format is `"JP NZ, $1234"`, `"CALL C, $0150"`, etc.

### Requirement: Instruction Length Reporting

The disassembler SHALL accurately report the byte length of each instruction.

#### Scenario: Single-byte instruction length

- **WHEN** a single-byte instruction like `NOP` is disassembled
- **THEN** the length is reported as 1

#### Scenario: Two-byte instruction length

- **WHEN** a two-byte instruction like `LD A, $FF` or `JR $10` is disassembled
- **THEN** the length is reported as 2

#### Scenario: Three-byte instruction length

- **WHEN** a three-byte instruction like `JP $1234` or `LD ($1234), A` is disassembled
- **THEN** the length is reported as 3

#### Scenario: CB-prefixed instruction length

- **WHEN** any CB-prefixed instruction is disassembled
- **THEN** the length is reported as 2 (CB prefix + opcode)

### Requirement: Public API

The disassembler SHALL expose a simple, ergonomic API for instruction decoding.

#### Scenario: Standalone disassembly

- **WHEN** `disasm(bytes, pc)` is called with instruction bytes and program counter
- **THEN** a `DisasmResult` is returned containing mnemonic and length
- **AND** no memory access or emulator state is required

#### Scenario: Result structure

- **WHEN** any disassembly function returns a result
- **THEN** the result contains a `mnemonic` field with the instruction string
- **AND** the result contains a `length` field with the instruction byte count
- **AND** both fields are directly accessible without unwrapping

### Requirement: Error Handling

The disassembler SHALL handle invalid or partial instruction bytes gracefully.

#### Scenario: Insufficient bytes for immediate

- **WHEN** `disasm()` is called with `[0xCD]` (CALL requires 3 bytes but only 1 provided)
- **THEN** the disassembly succeeds with available information
- **AND** missing bytes are shown as `$??` (e.g., `"CALL $????)"`)

#### Scenario: Illegal opcodes

- **WHEN** an undefined opcode is disassembled
- **THEN** the mnemonic is `"ILLEGAL $XX"` where XX is the opcode hex value
- **AND** the length is 1 byte
- **AND** the function does not panic
