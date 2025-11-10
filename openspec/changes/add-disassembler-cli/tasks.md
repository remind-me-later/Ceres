# Implementation Tasks

## 1. Core Integration

- [ ] 1.1 Add `disasm_at(&mut self, addr: u16) -> DisasmResult` method to `ceres-core/src/lib.rs` (Gb impl)
- [ ] 1.2 Implement memory reading for disassembly (read up to 3 bytes from address)
- [ ] 1.3 Handle CB-prefix correctly (read 2 bytes for CB instructions)
- [ ] 1.4 Call `crate::disasm::disasm()` with bytes from memory

## 2. Trace Flag Infrastructure

- [ ] 2.1 Add `trace_enabled: bool` field to `Gb` struct in `ceres-core/src/lib.rs`
- [ ] 2.2 Add parameter to `Gb::new()` or create `Gb::with_trace()` constructor
- [ ] 2.3 Initialize trace flag to false by default

## 3. Execution Tracing

- [ ] 3.1 Modify `Gb::run_cpu()` in `ceres-core/src/sm83.rs`
- [ ] 3.2 Add trace check: `if self.trace_enabled { self.trace_instruction(); }`
- [ ] 3.3 Implement `trace_instruction()` method that:
  - Calls `self.disasm_at(self.cpu.pc)`
  - Formats output: `[PC:$XXXX] MNEMONIC ; REGISTERS`
  - Prints to stdout (or logs)

## 4. Register Formatting

- [ ] 4.1 Implement `format_flags()` helper: converts F register to "Z---", "ZNH-", etc.
- [ ] 4.2 Format all registers in trace output
- [ ] 4.3 Output format: `A=XX F=ZNHC BC=XXXX DE=XXXX HL=XXXX SP=XXXX`

## 5. CLI Flag Addition

- [ ] 5.1 Add `trace: bool` field to `ceres_std::cli::Args` struct
- [ ] 5.2 Add `#[arg(long, help = "Enable instruction-level execution tracing")]` attribute
- [ ] 5.3 Update CLI help text

## 6. Frontend Integration - Winit

- [ ] 6.1 Pass `trace` flag from CLI args to `Gb::new()` or equivalent
- [ ] 6.2 Test: run `cargo run --package ceres-winit -- --trace test-roms/blargg/cpu_instrs/cpu_instrs.gb`
- [ ] 6.3 Verify trace output appears on stdout

## 7. Frontend Integration - GTK

- [ ] 7.1 Pass `trace` flag from CLI args to `Gb::new()` or equivalent
- [ ] 7.2 Test: run `cargo run --package ceres-gtk -- --trace test-roms/blargg/cpu_instrs/cpu_instrs.gb`
- [ ] 7.3 Verify trace output appears on stdout

## 8. Frontend Integration - Egui

- [ ] 8.1 Pass `trace` flag from CLI args to `Gb::new()` or equivalent
- [ ] 8.2 Test: run `cargo run --package ceres-egui -- --trace test-roms/blargg/cpu_instrs/cpu_instrs.gb`
- [ ] 8.3 Verify trace output appears on stdout

## 9. Performance Testing

- [ ] 9.1 Benchmark emulator speed with `--trace` disabled (should be unchanged)
- [ ] 9.2 Benchmark emulator speed with `--trace` enabled (expect 20-40% slowdown)
- [ ] 9.3 Verify no heap allocations in hot path when tracing disabled

## 10. MBC3 Debugging

- [ ] 10.1 Run mbc3-tester ROM with `--trace` flag
- [ ] 10.2 Capture first ~10,000 instructions to file
- [ ] 10.3 Analyze where bank switching occurs
- [ ] 10.4 Compare with expected behavior from test ROM source
- [ ] 10.5 Identify MBC3 bug (if present)

## 11. Validation

- [ ] 11.1 Run `cargo test --package ceres-core --package ceres-test-runner` - all tests pass
- [ ] 11.2 Run `openspec validate add-disassembler-cli --strict` - validation passes
- [ ] 11.3 Verify `--help` output includes `--trace` flag
- [ ] 11.4 Test trace output is readable and accurate

## Dependencies

- **Requires:** `add-disassembler-core` must be completed first
- No other blocking dependencies

## Estimated Time

- Core integration: 1 hour
- Tracing implementation: 2 hours
- CLI integration (all frontends): 1 hour
- Testing and validation: 1 hour
- MBC3 debugging: 2-4 hours (analysis time)
- **Total: ~8 hours**
