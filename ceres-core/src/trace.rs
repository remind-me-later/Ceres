//! Execution trace collection for debugging and analysis.
//!
//! This module provides a circular buffer for capturing CPU execution history
//! with minimal performance overhead. Traces can be queried programmatically
//! for debugging MBC banking errors, timing issues, and other emulator behavior.
//!
//! # Performance
//!
//! Trace collection adds approximately 2-3% overhead when enabled. The buffer
//! uses a fixed-size heap allocation and does not grow during execution.
//!
//! # Example
//!
//! ```no_run
//! use ceres_core::{AudioCallback, Sample, GbBuilder};
//!
//! struct DummyAudio;
//! impl AudioCallback for DummyAudio {
//!     fn audio_sample(&self, _l: Sample, _r: Sample) {}
//! }
//!
//! let mut gb = GbBuilder::new(44100, DummyAudio).build();
//! gb.trace_enable();
//!
//! // Run emulation...
//! for _ in 0..1000 {
//!     gb.run_cpu();
//! }
//!
//! // Analyze last 100 instructions
//! for entry in gb.trace_last_n(100) {
//!     println!("{:04X}: {}", entry.pc, entry.instruction);
//! }
//! ```

use alloc::string::String;
use alloc::vec::Vec;

/// Default capacity for the trace buffer (number of entries).
pub const DEFAULT_TRACE_CAPACITY: usize = 1000;

/// Snapshot of CPU register state at a specific point in time.
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RegisterSnapshot {
    /// Accumulator register
    pub a: u8,
    /// Flags register (Z, N, H, C flags)
    pub f: u8,
    /// B register
    pub b: u8,
    /// C register
    pub c: u8,
    /// D register
    pub d: u8,
    /// E register
    pub e: u8,
    /// H register
    pub h: u8,
    /// L register
    pub l: u8,
    /// Stack pointer
    pub sp: u16,
}

/// A single entry in the execution trace buffer.
///
/// Captures the CPU state before an instruction was executed, including
/// the program counter, disassembled instruction, cycle count, and register values.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TraceEntry {
    /// Program counter at the start of instruction execution
    pub pc: u16,
    /// Disassembled instruction string (e.g., "LD A, $42")
    pub instruction: String,
    /// Number of CPU cycles consumed by this instruction
    pub cycles: u8,
    /// Register state before instruction execution
    pub registers: RegisterSnapshot,
}

/// Circular buffer for storing execution traces.
///
/// The buffer has a fixed capacity and automatically overwrites the oldest
/// entries when full. This ensures bounded memory usage while maintaining
/// recent execution history.
pub struct TraceBuffer {
    /// Ring buffer storage for trace entries
    entries: Vec<TraceEntry>,
    /// Write position in the circular buffer (0..capacity)
    head: usize,
    /// Current number of valid entries (0..capacity)
    size: usize,
    /// Maximum number of entries the buffer can hold
    capacity: usize,
    /// Whether trace collection is currently active
    enabled: bool,
}

impl TraceBuffer {
    /// Creates a new trace buffer with the specified capacity.
    ///
    /// The buffer is initially disabled and empty. Call [`enable`](Self::enable)
    /// to start collecting traces.
    ///
    /// # Arguments
    ///
    /// * `capacity` - Maximum number of trace entries to store
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::with_capacity(capacity),
            head: 0,
            size: 0,
            capacity,
            enabled: false,
        }
    }

    /// Adds a new trace entry to the buffer.
    ///
    /// If the buffer is full, this overwrites the oldest entry. If tracing
    /// is disabled, this method does nothing.
    ///
    /// # Arguments
    ///
    /// * `entry` - The trace entry to add
    pub fn push(&mut self, entry: TraceEntry) {
        if !self.enabled {
            return;
        }

        if self.entries.len() < self.capacity {
            // Buffer not yet full, just append
            self.entries.push(entry);
        } else {
            // Buffer full, overwrite oldest entry
            self.entries[self.head] = entry;
        }

        // Advance head pointer with wraparound
        self.head = (self.head + 1) % self.capacity;

        // Update size (saturates at capacity)
        if self.size < self.capacity {
            self.size += 1;
        }
    }

    /// Clears all entries from the buffer.
    ///
    /// This resets the buffer to an empty state but does not change the
    /// enabled/disabled status.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.head = 0;
        self.size = 0;
    }

    /// Enables trace collection.
    ///
    /// After calling this method, [`push`](Self::push) will add entries to the buffer.
    pub fn enable(&mut self) {
        self.enabled = true;
    }

    /// Disables trace collection.
    ///
    /// After calling this method, [`push`](Self::push) will be a no-op.
    /// Existing entries remain in the buffer.
    pub fn disable(&mut self) {
        self.enabled = false;
    }

    /// Returns whether trace collection is currently enabled.
    #[inline]
    pub const fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Returns the current number of entries in the buffer.
    #[inline]
    pub const fn len(&self) -> usize {
        self.size
    }

    /// Returns whether the buffer is empty.
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.size == 0
    }

    /// Returns the maximum number of entries the buffer can hold.
    #[inline]
    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    /// Returns all entries in the buffer as a Vec, from oldest to newest.
    pub fn iter(&self) -> impl Iterator<Item = &TraceEntry> {
        // Collect entries in correct order
        let mut result = Vec::with_capacity(self.size);

        if self.entries.len() < self.capacity {
            // Buffer not yet full, entries are in order
            result.extend(self.entries.iter());
        } else {
            // Buffer full, need to rotate to get correct order
            let (newer, older) = self.entries.split_at(self.head);
            result.extend(older.iter());
            result.extend(newer.iter());
        }

        result.into_iter()
    }

    /// Returns a slice of the last N entries, from oldest to newest.
    ///
    /// If N is greater than the number of entries in the buffer, returns
    /// all available entries.
    pub fn last_n(&self, n: usize) -> Vec<&TraceEntry> {
        let count = n.min(self.size);
        let all_entries: Vec<_> = self.iter().collect();
        let skip = all_entries.len().saturating_sub(count);
        all_entries.into_iter().skip(skip).collect()
    }
}

impl Default for TraceBuffer {
    fn default() -> Self {
        Self::new(DEFAULT_TRACE_CAPACITY)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(pc: u16, instruction: &str, cycles: u8) -> TraceEntry {
        TraceEntry {
            pc,
            instruction: instruction.into(),
            cycles,
            registers: RegisterSnapshot {
                a: 0,
                f: 0,
                b: 0,
                c: 0,
                d: 0,
                e: 0,
                h: 0,
                l: 0,
                sp: 0,
            },
        }
    }

    #[test]
    fn test_buffer_initially_disabled() {
        let buffer = TraceBuffer::new(10);
        assert!(!buffer.is_enabled());
        assert_eq!(buffer.len(), 0);
        assert!(buffer.is_empty());
    }

    #[test]
    fn test_buffer_enable_disable() {
        let mut buffer = TraceBuffer::new(10);
        buffer.enable();
        assert!(buffer.is_enabled());
        buffer.disable();
        assert!(!buffer.is_enabled());
    }

    #[test]
    fn test_buffer_push_when_disabled() {
        let mut buffer = TraceBuffer::new(10);
        buffer.push(make_entry(0x100, "NOP", 4));
        assert_eq!(buffer.len(), 0);
    }

    #[test]
    fn test_buffer_push_when_enabled() {
        let mut buffer = TraceBuffer::new(10);
        buffer.enable();
        buffer.push(make_entry(0x100, "NOP", 4));
        assert_eq!(buffer.len(), 1);
    }

    #[test]
    fn test_buffer_wraparound() {
        let mut buffer = TraceBuffer::new(3);
        buffer.enable();

        // Fill buffer
        buffer.push(make_entry(0x100, "NOP", 4));
        buffer.push(make_entry(0x101, "LD A, B", 4));
        buffer.push(make_entry(0x102, "INC A", 4));
        assert_eq!(buffer.len(), 3);

        // Overwrite first entry
        buffer.push(make_entry(0x103, "DEC A", 4));
        assert_eq!(buffer.len(), 3);

        // Check that oldest entry was overwritten
        let entries: Vec<_> = buffer.iter().collect();
        assert_eq!(entries[0].pc, 0x101);
        assert_eq!(entries[1].pc, 0x102);
        assert_eq!(entries[2].pc, 0x103);
    }

    #[test]
    fn test_buffer_clear() {
        let mut buffer = TraceBuffer::new(10);
        buffer.enable();
        buffer.push(make_entry(0x100, "NOP", 4));
        buffer.push(make_entry(0x101, "LD A, B", 4));
        assert_eq!(buffer.len(), 2);

        buffer.clear();
        assert_eq!(buffer.len(), 0);
        assert!(buffer.is_empty());
        assert!(buffer.is_enabled()); // Clear doesn't disable
    }

    #[test]
    fn test_buffer_iter_not_full() {
        let mut buffer = TraceBuffer::new(10);
        buffer.enable();
        buffer.push(make_entry(0x100, "NOP", 4));
        buffer.push(make_entry(0x101, "LD A, B", 4));

        let entries: Vec<_> = buffer.iter().collect();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].pc, 0x100);
        assert_eq!(entries[1].pc, 0x101);
    }

    #[test]
    fn test_buffer_iter_after_wraparound() {
        let mut buffer = TraceBuffer::new(3);
        buffer.enable();

        // Fill and wrap
        buffer.push(make_entry(0x100, "A", 4));
        buffer.push(make_entry(0x101, "B", 4));
        buffer.push(make_entry(0x102, "C", 4));
        buffer.push(make_entry(0x103, "D", 4));
        buffer.push(make_entry(0x104, "E", 4));

        let entries: Vec<_> = buffer.iter().collect();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].pc, 0x102);
        assert_eq!(entries[1].pc, 0x103);
        assert_eq!(entries[2].pc, 0x104);
    }

    #[test]
    fn test_buffer_last_n() {
        let mut buffer = TraceBuffer::new(10);
        buffer.enable();
        buffer.push(make_entry(0x100, "A", 4));
        buffer.push(make_entry(0x101, "B", 4));
        buffer.push(make_entry(0x102, "C", 4));
        buffer.push(make_entry(0x103, "D", 4));

        let last_2 = buffer.last_n(2);
        assert_eq!(last_2.len(), 2);
        assert_eq!(last_2[0].pc, 0x102);
        assert_eq!(last_2[1].pc, 0x103);

        let last_10 = buffer.last_n(10);
        assert_eq!(last_10.len(), 4); // Only 4 entries exist
    }

    #[test]
    fn test_buffer_last_n_after_wraparound() {
        let mut buffer = TraceBuffer::new(3);
        buffer.enable();

        buffer.push(make_entry(0x100, "A", 4));
        buffer.push(make_entry(0x101, "B", 4));
        buffer.push(make_entry(0x102, "C", 4));
        buffer.push(make_entry(0x103, "D", 4));

        let last_2 = buffer.last_n(2);
        assert_eq!(last_2.len(), 2);
        assert_eq!(last_2[0].pc, 0x102);
        assert_eq!(last_2[1].pc, 0x103);
    }
}
