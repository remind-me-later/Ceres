# Change: Refine Trace Collection for MBC3 Test Debugging

## Why

The `mbc3-tester` integration test requires validation after 40 frames of execution. The current trace collection
implementation starts from the bootrom, generating a massive volume of trace data. This makes it impractical to analyze
the specific test case behavior. We need a way to scope trace collection to the relevant execution period.

## What Changes

- **ADD** a mechanism to enable and disable trace collection based on the program counter (PC).
- This will allow tests to start tracing only when the test ROM's code is executing, skipping the bootrom and other
  irrelevant setup phases.

## Impact

- **Affected specs**: `trace-collection`
- **Affected code**: `ceres-core/src/trace.rs`, `ceres-core/src/sm83.rs`, `ceres-test-runner/src/test_runner.rs`
