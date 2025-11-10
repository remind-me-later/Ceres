# Implementation Tasks

## 1. Dependencies

- [ ] 1.1 Add `heapless = "0.8"` to `ceres-core/Cargo.toml` dependencies
- [ ] 1.2 Verify `no_std` compatibility with heapless

## 2. Module Scaffolding

- [ ] 2.1 Create `ceres-core/src/disasm/mod.rs` file
- [ ] 2.2 Define `DisasmResult` struct with `mnemonic: heapless::String<32>` and `length: u8`
- [ ] 2.3 Add module declaration to `ceres-core/src/lib.rs`
- [ ] 2.4 Export `disasm` module publicly

## 3. Base Opcode Disassembly

- [ ] 3.1 Implement `pub fn disasm(bytes: &[u8], pc: u16) -> DisasmResult`
- [ ] 3.2 Add match statement for all 256 opcodes (0x00-0xFF)
- [ ] 3.3 Handle 1-byte instructions (NOP, LD A,B, ADD A,B, etc.)
- [ ] 3.4 Handle 2-byte instructions with 8-bit immediate (LD A,$FF, JR $10, etc.)
- [ ] 3.5 Handle 3-byte instructions with 16-bit immediate (JP $1234, LD ($1234),A, etc.)
- [ ] 3.6 Format illegal opcodes as `"ILLEGAL $XX"`

## 4. CB-Prefix Opcode Disassembly

- [ ] 4.1 Implement `fn disasm_cb(opcode: u8) -> DisasmResult`
- [ ] 4.2 Decode bit operations: BIT, SET, RES (opcodes 0x40-0xFF)
- [ ] 4.3 Decode rotates/shifts: RLC, RRC, RL, RR, SLA, SRA, SRL, SWAP (opcodes 0x00-0x3F)
- [ ] 4.4 Handle all 8 register operands (B, C, D, E, H, L, (HL), A)

## 5. Formatting

- [ ] 5.1 Implement hex formatting with `$` prefix
- [ ] 5.2 Format 16-bit immediates as little-endian (e.g., bytes [0x50, 0x01] → `$0150`)
- [ ] 5.3 Format memory indirect with parentheses (e.g., `(HL)`, `(BC)`)
- [ ] 5.4 Format conditional suffixes (NZ, Z, NC, C)
- [ ] 5.5 Format high-page instructions (LDH)
- [ ] 5.6 Ensure all output fits in 32-byte limit

## 6. Unit Tests

- [ ] 6.1 Create `ceres-core/src/disasm/tests.rs` (or use inline `#[cfg(test)]` module)
- [ ] 6.2 Test data transfer instructions (LD)
- [ ] 6.3 Test arithmetic instructions (ADD, SUB, INC, DEC, ADC, SBC)
- [ ] 6.4 Test logical instructions (AND, OR, XOR, CP)
- [ ] 6.5 Test control flow (JP, JR, CALL, RET, RST)
- [ ] 6.6 Test stack operations (PUSH, POP)
- [ ] 6.7 Test CB-prefix instructions (BIT, SET, RES, rotates, shifts, SWAP)
- [ ] 6.8 Test edge cases (illegal opcodes, insufficient bytes)

## 7. Validation

- [ ] 7.1 Run `cargo test --package ceres-core` - all tests pass
- [ ] 7.2 Run `openspec validate add-disassembler-core --strict` - validation passes
- [ ] 7.3 Verify `no_std` compatibility: `cargo build --package ceres-core --target thumbv7em-none-eabihf` (or similar)
- [ ] 7.4 Check code coverage for disasm module

## Dependencies

- Requires `heapless` crate
- No dependencies on other proposals
- Independent of `add-disassembler-cli`

## Estimated Time

- Module scaffolding: 30 minutes
- Base opcodes: 2-3 hours
- CB opcodes: 1 hour
- Unit tests: 2 hours
- **Total: ~6 hours**
