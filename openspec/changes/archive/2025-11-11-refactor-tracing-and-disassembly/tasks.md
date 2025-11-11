## 1. Core Disassembler Implementation

- [ ] 1.1. Create `ceres-core/src/disasm.rs` with a `disassemble` function that takes a byte slice and returns a
      structured `Instruction` and the number of bytes read.
- [ ] 1.2. Implement decoding for all SM83 opcodes, including CB-prefixed ones.
- [ ] 1.3. Create a `Display` implementation for the `Instruction` struct to produce human-readable assembly.
- [ ] 1.4. Add unit tests for the disassembler, covering a wide range of instructions.

## 2. Trace Format and Integration

- [ ] 2.1. Add a dependency on the `tracing` crate with the `attributes` feature for `no_std` compatibility.
- [ ] 2.2. Refactor `ceres-core/src/trace.rs` to use the `tracing` crate for structured logging.
- [ ] 2.3. Update `Gb::run_cpu` in `ceres-core/src/sm83.rs` to call the disassembler and record a trace entry for each
      instruction when tracing is enabled via the tracing subscriber.
- [ ] 2.4. Add configuration options to control tracing output format (JSON, plain text, etc.).
- [ ] 2.5. Remove the `analyze_trace.py` script from `ceres-test-runner/`.
- [ ] 2.6. Update documentation in `AGENTS.md` and `README.md` to reflect the new tracing and analysis workflow using
      Rust tracing ecosystem.
