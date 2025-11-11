# Change: Integrate Tracing with Test Runner for Debugging

## Why

The refactor-tracing-and-disassembly change successfully replaced the legacy trace buffer system with the standard Rust
tracing ecosystem and created a structured disassembler. However, the test runner currently has limited debugging
capabilities when tests fail.

Currently, the test runner can collect traces in a basic way, but doesn't fully leverage the new tracing infrastructure
for detailed post-mortem analysis of failing tests. This makes it difficult to diagnose complex issues in the test ROM
suite, particularly timing-sensitive failures or subtle behavioral differences.

This proposal aims to enhance the test runner's debugging capabilities by integrating the new Rust tracing system. This
will provide detailed execution traces when tests fail, enabling developers to analyze exactly what went wrong in the
emulator's internal logic.

## What Changes

This change will enhance the test runner with comprehensive tracing integration:

### 1. Enhanced Test Tracing (`test-debugging`)

- The test runner will enable detailed tracing for all tests but only preserve traces for failing tests
- Tracing will capture comprehensive system events (CPU, APU, PPU, etc.) for complete debugging context
- Trace output will be formatted specifically for test failure analysis
- Automatic trace preservation will be enabled for failure and timeout scenarios, with traces discarded for passing
  tests

### 2. Test Runner Integration (`test-runner`)

- The test runner will configure tracing subscribers that capture execution data during all tests
- When tests fail, the system will export the complete captured trace information to structured JSON files
- Integration with the existing test timeout and failure detection mechanisms
- Efficient trace management that only saves traces for failing tests to maintain performance

## Impact

- **Affected Specs**: `test-debugging` (new), `test-runner` (modified)
- **Affected Code**:
  - `ceres-test-runner/src/test_runner.rs` (enhanced tracing integration)
  - `ceres-test-runner/Cargo.toml` (add tracing dependencies if needed)
  - New module in test runner for trace collection during tests
- **Breaking Changes**: None - this is an enhancement to existing functionality
- **Performance Impact**: Traces will be captured for all tests but only saved for failing ones, maintaining performance
  for passing tests
- **Validation**: The implementation will be validated using the currently failing mbc3-tester ROM to demonstrate
  debugging capabilities
