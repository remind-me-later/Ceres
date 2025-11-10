# Change: Refactor Tracing and Disassembly

## Why

The previous attempt to add a disassembler and execution tracing (`add-sm83-disassembler`) was incomplete and rushed.
The tracing module was not integrated properly, the disassembler core was missing, and the trace format was not ideal
for debugging the emulator's internal logic, particularly for diagnosing failures in the test ROM suite.

This proposal aims to deliver a robust, well-designed, and complete tracing and disassembly solution. It addresses the
shortcomings of the previous implementation by focusing on the standard Rust tracing ecosystem which is well-tested,
performant, and familiar to Rust developers. The primary goal of this enhanced tracing capability is to facilitate
debugging of the emulator's internal logic, particularly for diagnosing failures in the test ROM suite, rather than
debugging the ROMs themselves.

## What Changes

This change will be broken down into two main capabilities:

### 1. Core Disassembler (`disassembler-core`)

- A `no_std` compatible disassembler will be created in `ceres-core/src/disasm.rs`.
- It will provide a function to decode an opcode and its operands from a byte slice into a structured `Instruction`
  representation, not just a `String`.
- A separate function will format the `Instruction` struct into a human-readable string. This separation improves
  testability and allows for different formatting strategies in the future.

### 2. Standard Rust Trace Logging (`trace-log-format`)

- The existing `ceres-core/src/trace.rs` will be refactored to use the standard Rust `tracing` crate.
- Instead of a custom JSON format, we will leverage the `tracing` crate ecosystem which provides:
  1. Structured logging with metadata
  2. Multiple output formats (JSON, plain text, etc.)
  3. Filtering capabilities
  4. Good performance characteristics
  5. Integration with existing Rust tooling
- The workflow will be as follows:
  1. The emulator will use the `tracing` crate to log trace events as structured data.
  2. The output can be configured via environment variables or programmatic configuration to go to files, stdout, or
     other sinks in different formats.
  3. Developers can use standard tracing tools and filters to analyze the execution logs.
- The `TraceEntry` will be updated to work with tracing's structured data model.
- The `Gb::run_cpu` method in `ceres-core/src/sm83.rs` will be modified to generate a trace event for each executed
  instruction when tracing is enabled.
- The `analyze_trace.py` script will be **removed** in favor of standard Rust tracing tools and filtering.

## Impact

- **Affected Specs**: `disassembler-core` (new), `trace-log-format` (new), `trace-collection` (modified).
- **Affected Code**:
  - `ceres-core/src/disasm.rs` (new file)
  - `ceres-core/src/trace.rs` (refactored)
  - `ceres-core/src/lib.rs` (updated to expose new features)
  - `ceres-core/src/sm83.rs` (integration of tracing)
  - `ceres-core/Cargo.toml` (add tracing dependency)
  - `ceres-test-runner/analyze_trace.py` (deleted)
- **Breaking Changes**: The trace format will change from a custom JSON to the Rust tracing ecosystem format. The
  `analyze_trace.py` script will be removed.
- `no_std` compatibility will be maintained in `ceres-core` using the `tracing` crate's `attributes` feature.
