## 1. Core Logic

- [ ] 1.1 In `ceres-core/src/trace.rs`, add fields to `Tracer` to store start and end PC for tracing.
- [ ] 1.2 In `ceres-core/src/sm83.rs`, modify `run_cpu` to check the PC against the start/end range before recording a
      trace.

## 2. Test Runner Integration

- [ ] 2.1 In `ceres-test-runner/src/test_runner.rs`, add a method to the test builder to specify a PC range for tracing.
- [ ] 2.2 Update the `mbc3-tester` test in `ceres-test-runner/tests/mbc3_tester.rs` to use the new PC-based tracing
      feature.

## 3. Documentation

- [ ] 3.1 Update `openspec/specs/trace-collection/spec.md` to reflect the new PC-based triggering capability.
