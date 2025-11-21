# Change: Optimize Tracing Infrastructure

## Why

The current tracing infrastructure relies on `tracing-chrome` to write trace events directly to JSON files. While
effective for short captures, this approach has significant limitations:

1. **Performance Overhead**: Continuous disk I/O slows down emulation, making it difficult to debug timing-sensitive
   issues.
2. **File Size**: Trace files grow rapidly (hundreds of MBs for a few seconds), making them unmanageable for
   long-running tests.
3. **Analysis Paralysis**: Debugging a failure often requires only the last few frames of execution, but we currently
   capture everything from the start, creating noise.
4. **System Instability**: As seen in the `debug-call-cc-timing2` investigation, massive trace files can cause system
   instability and make tools like Perfetto unresponsive.

We need a more efficient way to capture relevant trace data without the massive overhead of full-session logging.

## What Changes

We will introduce a **Ring Buffer Tracing System** that keeps only the most recent events in memory and writes them to
disk only when needed (e.g., on test failure).

### Key Components

1. **In-Memory Ring Buffer Layer**:
   - A custom `tracing_subscriber::Layer` that stores events in a fixed-size circular buffer.
   - Configurable buffer size (e.g., keep last N events or last N MB of data).
   - `no_std` compatible core (if possible) or efficient `std` implementation.

2. **On-Demand Dump**:
   - Ability to flush the buffer to a file programmatically.
   - Integration with `ceres-test-runner` to dump traces automatically when a test fails.

3. **Advanced Filtering**:
   - **Trigger-based Tracing**: Start/stop tracing based on events (e.g., PC reaching a value, memory write to specific
     address).
   - **Cycle-based Filtering**: Trace only specific windows of time (e.g., cycles X to Y).

## Impact

- **`ceres-test-runner`**: Will be updated to use the ring buffer layer. Tests will run faster and only produce traces
  on failure.
- **`ceres-std`**: Will host the new tracing infrastructure code (or a new `ceres-trace` crate if it grows too large).
- **Developer Experience**: Debugging will become faster. "Run until fail" workflows will produce concise, relevant
  traces immediately.
