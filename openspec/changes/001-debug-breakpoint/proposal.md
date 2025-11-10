# Proposal: Debug Breakpoint Instruction

- **Author**: Gemini
- **Status**: Proposed
- **Created**: 2025-11-10
- **Last Updated**: 2025-11-10

## Abstract

This proposal introduces a mechanism to notify the emulator frontend when a specific instruction (`ld b, b`) is
executed. This will serve as a debug breakpoint, primarily for detecting test completion in automated test ROMs, and can
be extended for more advanced debugging features in the future.

## Motivation

Several widely-used Game Boy test ROMs, including `cgb-acid2`, `dmg-acid2`, and others in the `gameboy-test-roms` suite,
use the `ld b, b` instruction (opcode `0x40`) as a signal to indicate that a test has completed its execution. At this
point, the video frame is ready for inspection.

Currently, the emulator has no way to recognize this signal. Test runners must rely on fixed frame counts or timeouts,
which are unreliable and inefficient. For `ceres-test-runner` to automatically and accurately verify the results of
these tests, it needs a way to stop emulation precisely when the test signals completion.

Implementing a breakpoint on `ld b, b` provides a clean, reliable solution for this problem. It also establishes a
foundation for more sophisticated debugging tools, such as user-defined breakpoints.

## Proposal

1. **Define a `BreakpointCallback` Trait**: A new public trait named `BreakpointCallback` will be defined in
   `ceres-core`.

   ```rust
   // In ceres-core/src/lib.rs or a new module
   pub trait BreakpointCallback {
       fn on_breakpoint(&mut self);
   }
   ```

2. **Integrate Callback into `GameBoy`**: The main `GameBoy` struct in `ceres-core` will be modified to accept a generic
   type parameter that implements `BreakpointCallback`. It will hold an instance of this callback.

   ```rust
   // In ceres-core/src/lib.rs
   pub struct GameBoy<T: BreakpointCallback> {
       // ... existing fields
       pub breakpoint_callback: T,
   }
   ```

   A `NoopBreakpointCallback` struct will be provided as a default for frontends that do not need to handle breakpoints.

3. **Trigger Callback in CPU Execution**: In the CPU instruction execution loop within `ceres-core/src/sm83.rs`, when
   the `ld b, b` instruction (opcode `0x40`) is fetched and decoded, the `on_breakpoint` method of the registered
   callback will be invoked.

   ```rust
   // In ceres-core/src/sm83.rs, inside the instruction execution loop
   // ...
   let opcode = self.read_byte_from_pc();
   match opcode {
       // ... other opcodes
       0x40 => { // ld b, b
           // The instruction does nothing CPU-wise
           gameboy.breakpoint_callback.on_breakpoint();
       }
       // ... other opcodes
   }
   // ...
   ```

4. **Implement the Callback in `ceres-test-runner`**: The `ceres-test-runner` will implement the `BreakpointCallback`
   trait. The `on_breakpoint` method will set a flag to gracefully terminate the test's execution loop.

   ```rust
   // In ceres-test-runner/src/test_runner.rs
   struct TestRunner {
       // ...
       hit_breakpoint: bool,
   }

   impl BreakpointCallback for TestRunner {
       fn on_breakpoint(&mut self) {
           self.hit_breakpoint = true;
       }
   }

   // In the test execution loop
   while !runner.hit_breakpoint {
       gameboy.run_frame();
   }
   ```

5. **Update Frontends**: All other frontends (`ceres-winit`, `ceres-egui`, `ceres-gtk`) will be updated to use the
   `NoopBreakpointCallback` to satisfy the new generic requirement on `GameBoy`. This ensures they continue to function
   without any changes to their behavior.

## Rationale

- **Trait-based Approach**: Using a trait provides a clean, decoupled architecture. The core emulation logic doesn't
  need to know about the specific implementation details of any frontend.
- **Minimal Performance Impact**: The check is a single `match` arm on the opcode, which is already being decoded. The
  overhead for non-breakpoint instructions is zero. For the breakpoint instruction, the cost is a single method call.
- **Extensibility**: This design can be easily extended. For example, the `on_breakpoint` method could take the current
  CPU state as an argument, or the trait could be expanded with other methods for different types of breakpoints.

## Backwards Compatibility

This is a breaking change for all consumers of `ceres-core` because it adds a generic parameter to the `GameBoy` struct.
All instantiations of `GameBoy` will need to be updated. However, providing a `NoopBreakpointCallback` makes this a
straightforward mechanical change for frontends that don't require breakpoint functionality.

## Test Plan

1. A new integration test will be added to `ceres-test-runner`.
2. This test will run a simple ROM that executes `ld b, b` and then enters an infinite loop.
3. The test will pass if the `on_breakpoint` callback is triggered and the test terminates successfully.
4. Existing tests, particularly `cgb-acid2`, will be updated to use this new mechanism instead of a fixed frame count,
   verifying its effectiveness in a real-world scenario.

## Alternatives Considered

1. **Hardcoded Flag in `GameBoy` Struct**:

   - A `bool` flag could be added to the `GameBoy` struct, e.g., `hit_breakpoint`.
   - The CPU would set this flag, and the frontend would have to poll it after every frame.
   - **Downside**: This is less clean, couples the core to a specific polling mechanism, and is less extensible than a
     trait-based callback system.

2. **Returning a Value from `run_frame`**:
   - The `run_frame` or a CPU step function could return an enum, e.g., `ExecutionState::Running` or
     `ExecutionState::BreakpointHit`.
   - **Downside**: This complicates the return signature of core emulation functions and forces the caller to inspect
     the return value on every single call, which can be less ergonomic. The callback approach is more direct and
     event-driven.
