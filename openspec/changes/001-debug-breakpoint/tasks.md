## 1. Core Logic Implementation

- [ ] 1.1 Define `BreakpointCallback` trait in `ceres-core`.
- [ ] 1.2 Add a `NoopBreakpointCallback` default implementation.
- [ ] 1.3 Add generic `<T: BreakpointCallback>` to `GameBoy` struct and store the callback.
- [ ] 1.4 In `sm83.rs`, detect `ld b, b` (opcode `0x40`) and call the `on_breakpoint` callback.

## 2. Test Runner Integration

- [ ] 2.1 Implement `BreakpointCallback` for the test runner in `ceres-test-runner`.
- [ ] 2.2 Use a flag in the test runner to detect when the breakpoint is hit and stop emulation.
- [ ] 2.3 Update `cgb-acid2` and `dmg-acid2` tests to use the breakpoint for completion instead of frame counting.
- [ ] 2.4 Add a new test ROM that specifically tests the `ld b, b` breakpoint.

## 3. Frontend Updates

- [ ] 3.1 Update `ceres-winit` to use `NoopBreakpointCallback`.
- [ ] 3.2 Update `ceres-egui` to use `NoopBreakpointCallback`.
- [ ] 3.3 Update `ceres-gtk` to use `NoopBreakpointCallback`.
