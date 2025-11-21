# Design: Ring Buffer Tracing

## Problem

The current `tracing-chrome` implementation writes every event to disk immediately. This causes:

1. High I/O overhead.
2. Massive file sizes.
3. Difficulty in capturing "just the crash" without filling the disk.

## Solution

Implement a `RingBufferLayer` that stores trace events in memory in a circular buffer.

### Architecture

```rust
struct RingBufferLayer {
    buffer: Arc<Mutex<Vec<StoredEvent>>>,
    position: usize,
    size: usize,
    wrapped: bool,
}

struct StoredEvent {
    timestamp: u64,
    level: Level,
    target: String,
    name: String,
    fields: BTreeMap<String, serde_json::Value>,
    thread_id: ThreadId,
}
```

### Event Storage

Since `tracing::Event` holds references, we must serialize/clone the data immediately when `on_event` is called.

- **Timestamp**: Capture `Instant::now()` or emulator cycle count if available (via field).
- **Fields**: Use a `Visit` implementation to extract fields into `serde_json::Value`.

### Flushing

When `flush` is called:

1. Lock the buffer.
2. Iterate from `position` (if wrapped) to end, then 0 to `position`.
3. Serialize each event to JSON.
4. Wrap in the Chrome Trace Event Format array `[ ... ]`.

### Integration

The `TestRunner` will hold a reference to the `RingBufferLayer` (or a handle to flush it).
On test failure, it calls `handle.flush(path)`.

### Performance Considerations

- **Allocation**: Storing events involves allocation (String, BTreeMap). This is still faster than Disk I/O but not
  zero-cost.
- **Locking**: Mutex contention should be low for single-threaded emulation, but we must be careful.

### Alternatives Considered

- **Mmap**: Memory-mapped files could work, but still involve I/O and file management.
- **Conditional Tracing**: Only enabling tracing near the crash. Hard to predict *when* the crash happens without
  running it first.
