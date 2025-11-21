# Tasks: Optimize Tracing Infrastructure

## 1. Ring Buffer Implementation

- [x] Create `RingBufferLayer` struct implementing `tracing_subscriber::Layer`
  - [x] Define event storage format (optimized for memory usage)
  - [x] Implement circular buffer logic (overwrite oldest events)
  - [x] Add thread-safe access (Mutex/RwLock)
- [x] Implement `flush_to_file` method
  - [x] Serialize stored events to Chrome Trace Event Format (JSON)
  - [x] Handle file I/O efficiently

## 2. Integration with Test Runner

- [x] Update `ceres-test-runner` dependencies
  - [x] Add `RingBufferLayer` (from `ceres-std` or local module)
- [x] Modify `TestRunner` to support ring buffer mode
  - [x] Add configuration option for buffer size (e.g., number of events)
  - [x] Initialize `RingBufferLayer` instead of `ChromeLayer` when enabled
- [x] Implement "Dump on Failure"
  - [x] Detect test failure in `TestRunner`
  - [x] Trigger `flush_to_file` only when test fails
  - [x] Ensure unique filenames (timestamp/test name)

## 3. Advanced Filtering (Optional/Phase 2)

- [ ] Implement Trigger-based filtering
  - [ ] Add `TriggerLayer` that wraps another layer
  - [ ] Define triggers (PC address, cycle count, memory access)
  - [ ] Enable/disable underlying layer based on triggers

## 4. Validation

- [x] Verify trace output format is compatible with Perfetto
- [x] Benchmark performance overhead of Ring Buffer vs Direct File I/O
- [x] Verify "Dump on Failure" works correctly with existing tests
