## ADDED Requirements

### Requirement: Decode SM83 Instructions

The disassembler SHALL decode a byte stream into a structured representation of an SM83 instruction.

#### Scenario: Decode a simple instruction

- **GIVEN** a byte slice `[0x06, 0x42]`
- **WHEN** `disassemble` is called on the slice
- **THEN** it SHOULD return an `Instruction` struct representing `LD B, $42` and indicate that 2 bytes were consumed.

#### Scenario: Decode a CB-prefixed instruction

- **GIVEN** a byte slice `[0xCB, 0x7C]`
- **WHEN** `disassemble` is called on the slice
- **THEN** it SHOULD return an `Instruction` struct representing `BIT 7, H` and indicate that 2 bytes were consumed.

### Requirement: Format Instructions as Strings

The disassembler SHALL provide a way to format the structured `Instruction` into a human-readable string.

#### Scenario: Format a simple instruction

- **GIVEN** an `Instruction` struct for `LD A, $FF`
- **WHEN** it is formatted as a string
- **THEN** the output string SHOULD be `"LD A, $FF"`.
