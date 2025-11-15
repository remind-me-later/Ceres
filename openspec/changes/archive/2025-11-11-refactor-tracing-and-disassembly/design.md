## Debugging and Tracing Strategy: Rust Tracing vs. CTF vs. GDB Stub

This document outlines the decision-making process for choosing a tracing and debugging strategy for the Ceres emulator.
Three primary approaches were considered:

1. **Rust Tracing Ecosystem**: The currently proposed solution using the standard `tracing` crate.
2. **Offline Trace Analysis via Common Trace Format (CTF)**: The originally proposed solution.
3. **Interactive Debugging via GDB Remote Stub**: An alternative approach.

### 1. Rust Tracing Ecosystem Approach

This approach focuses on generating detailed, low-overhead execution logs using the standard Rust `tracing` crate during
emulation runs. These logs use a standardized, structured format that can be output in multiple formats (JSON, plain
text, etc.) for efficient post-mortem analysis.

**Pros:**

- **Standard Rust Ecosystem**: Uses the well-established and widely adopted `tracing` crate ecosystem which has
  excellent tooling and documentation.
- **Low Overhead**: The `tracing` crate is designed for performance with conditional compilation and filtering
  capabilities that add minimal performance impact to the emulation when disabled. This is crucial for running long
  tests or playing games without significant slowdown.
- **Post-Mortem Analysis**: It provides a complete history of execution, which is invaluable for debugging complex
  issues that are hard to reproduce, such as timing bugs or rare graphical glitches.
- **Flexible Output Formats**: Can output to JSON, plain text, or other formats as needed. Developers can use the same
  tracing data with different tools and filters.
- **Simpler Implementation**: Using the established `tracing` crate is significantly less complex than implementing a
  custom CTF writer or a GDB stub protocol.
- **no_std Compatibility**: Using the `tracing` crate with the `attributes` feature maintains `no_std` compatibility for
  the core emulator.

**Cons:**

- **Learning Curve**: While standard in Rust, team members unfamiliar with the `tracing` ecosystem may need to learn how
  to properly configure and filter logs.
- **Requires External Tooling**: Users may need to install and learn how to use tools like `tracing-chrome`,
  `tracing-subscriber`, or filters to inspect the trace output effectively.

### 2. Common Trace Format (CTF) Approach

This was the originally proposed approach, focusing on generating traces in the CTF format.

**Pros:**

- **Standardized Tooling**: By using CTF, we could leverage a mature ecosystem of analysis tools like `babeltrace`,
  which are far more powerful than a custom script.

**Cons:**

- **No Available Rust Implementation**: The main issue identified is that there are no available Rust crates for writing
  CTF format, making this approach infeasible.
- **High Implementation Complexity**: Implementing a CTF writer from scratch that works in a `no_std` environment would
  be very complex.

### 3. GDB Remote Stub Approach

This approach involves implementing a GDB server directly within the emulator. This would allow a standard GDB client
(or a compatible IDE like VS Code) to connect to the running emulator and debug it interactively.

**Pros:**

- **Powerful Interactive Debugging**: Provides a full suite of debugging features: step-by-step execution (`step`,
  `next`), breakpoints, watchpoints, memory inspection, and register modification.
- **Leverages Existing Ecosystem**: Developers can use the familiar and powerful GDB client and its many frontends. No
  need to learn a new set of tools.
- **Direct Feedback**: Problems can be investigated "live" as they occur, which can be more intuitive than sifting
  through a large trace file.

**Cons:**

- **High Implementation Complexity**: The GDB Remote Serial Protocol is non-trivial to implement correctly. It requires
  handling a variety of commands, managing state, and communicating over a socket.
- **Performance Impact**: An active GDB connection can introduce significant overhead, potentially altering the
  timing-sensitive behavior of the emulator and making certain bugs harder to reproduce.
- **Not Ideal for Long-Running Analysis**: While GDB can log commands, it is not designed for capturing comprehensive,
  high-performance traces of long execution runs. It is primarily a tool for interactive, "in-the-moment" debugging.

## Decision

The **Rust Tracing approach** was chosen for this proposal because it provides the best balance of feasibility,
ecosystem integration, and debugging capability. It addresses the original goal of debugging failing integration tests
(`test-roms`) while being practical to implement in the Rust ecosystem with `no_std` compatibility.

The Rust tracing ecosystem provides structured logging that can be formatted in multiple ways (including JSON for
compatibility with existing analysis workflows), has excellent performance characteristics with conditional compilation,
and is familiar to Rust developers. It allows for post-mortem analysis which is the primary use case while being much
more practical to implement than the CTF approach that lacks available Rust libraries.
