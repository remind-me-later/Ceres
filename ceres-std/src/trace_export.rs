//! Trace export functionality for analysis tools.
//!
//! This module provides JSON export for execution traces, enabling
//! external analysis tools (Python scripts, jq, etc.) to process
//! emulator execution history.

use ceres_core::{AudioCallback, Gb, trace::TraceEntry};
use serde::Serialize;

/// Metadata about the trace export
#[derive(Debug, Serialize)]
pub struct TraceMetadata {
    /// Total number of entries in the buffer
    pub entry_count: usize,
    /// Maximum capacity of the buffer
    pub buffer_capacity: usize,
    /// Timestamp of export (seconds since Unix epoch)
    pub timestamp: u64,
}

/// Complete trace export structure
#[derive(Debug, Serialize)]
pub struct TraceExport<'a> {
    /// Export metadata
    pub metadata: TraceMetadata,
    /// Trace entries
    pub entries: Vec<&'a TraceEntry>,
}

/// Export trace buffer as formatted JSON string.
///
/// Creates a JSON object containing metadata and all trace entries.
/// The output is pretty-printed for human readability.
///
/// # Arguments
///
/// * `gb` - The Game Boy emulator instance
///
/// # Returns
///
/// A formatted JSON string containing the trace data
///
/// # Errors
///
/// Returns an error if JSON serialization fails (unlikely in practice)
///
/// # Example
///
/// ```no_run
/// use ceres_core::{AudioCallback, Sample, GbBuilder};
/// use ceres_std::trace_export::export_trace_json;
///
/// struct DummyAudio;
/// impl AudioCallback for DummyAudio {
///     fn audio_sample(&self, _l: Sample, _r: Sample) {}
/// }
///
/// let gb = GbBuilder::new(44100, DummyAudio).build();
/// let json = export_trace_json(&gb).unwrap();
/// std::fs::write("trace.json", json).unwrap();
/// ```
pub fn export_trace_json<A: AudioCallback>(gb: &Gb<A>) -> Result<String, serde_json::Error> {
    let entries: Vec<_> = gb.trace_entries().collect();

    let export = TraceExport {
        metadata: TraceMetadata {
            entry_count: gb.trace_count(),
            buffer_capacity: gb.trace_capacity(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        },
        entries,
    };

    serde_json::to_string_pretty(&export)
}

/// Export trace buffer as compact JSON string.
///
/// Similar to [`export_trace_json`] but without pretty-printing,
/// resulting in smaller file sizes.
///
/// # Arguments
///
/// * `gb` - The Game Boy emulator instance
///
/// # Returns
///
/// A compact JSON string containing the trace data
///
/// # Errors
///
/// Returns an error if JSON serialization fails (unlikely in practice)
pub fn export_trace_json_compact<A: AudioCallback>(
    gb: &Gb<A>,
) -> Result<String, serde_json::Error> {
    let entries: Vec<_> = gb.trace_entries().collect();

    let export = TraceExport {
        metadata: TraceMetadata {
            entry_count: gb.trace_count(),
            buffer_capacity: gb.trace_capacity(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        },
        entries,
    };

    serde_json::to_string(&export)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ceres_core::{AudioCallback, GbBuilder, Sample};

    struct DummyAudio;
    impl AudioCallback for DummyAudio {
        fn audio_sample(&self, _l: Sample, _r: Sample) {}
    }

    #[test]
    fn test_export_empty_trace() {
        let gb = GbBuilder::new(44100, DummyAudio).build();
        let json = export_trace_json(&gb).unwrap();

        // Should contain metadata with 0 entries
        assert!(json.contains("\"entry_count\": 0"));
        assert!(json.contains("\"entries\": []"));
    }

    #[test]
    fn test_export_with_traces() {
        let mut gb = GbBuilder::new(44100, DummyAudio).build();
        gb.trace_enable();

        // Run a few instructions
        for _ in 0..10 {
            gb.run_cpu();
        }

        let json = export_trace_json(&gb).unwrap();

        // Should contain some entries
        assert!(json.contains("\"entries\":"));
        assert!(gb.trace_count() > 0);

        // Parse it back to verify it's valid JSON
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed["metadata"]["entry_count"].as_u64().unwrap() > 0);
    }

    #[test]
    fn test_compact_export() {
        let mut gb = GbBuilder::new(44100, DummyAudio).build();
        gb.trace_enable();

        for _ in 0..5 {
            gb.run_cpu();
        }

        let pretty = export_trace_json(&gb).unwrap();
        let compact = export_trace_json_compact(&gb).unwrap();

        // Compact should be smaller
        assert!(compact.len() < pretty.len());

        // Both should be valid JSON with same data
        let pretty_parsed: serde_json::Value = serde_json::from_str(&pretty).unwrap();
        let compact_parsed: serde_json::Value = serde_json::from_str(&compact).unwrap();
        assert_eq!(
            pretty_parsed["metadata"]["entry_count"],
            compact_parsed["metadata"]["entry_count"]
        );
    }
}
