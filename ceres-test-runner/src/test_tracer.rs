//! Tracing infrastructure for test debugging
//!
//! This module provides a custom tracing subscriber that captures execution events
//! during test runs and exports them only for failing tests.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use tracing::Event;
use tracing_subscriber::{
    layer::{Context, Layer},
    registry::LookupSpan,
};

/// A single trace entry from the emulator execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceEntry {
    /// Target of the trace event (e.g., "cpu_execution", "apu", "ppu")
    pub target: String,
    /// The level of the tracing event
    pub level: String,
    /// The name of the event
    pub name: String,
    /// Timestamp of the event
    pub timestamp: u64,
    /// Key-value fields attached to the event
    pub fields: std::collections::HashMap<String, serde_json::Value>,
}

/// Metadata about a trace collection session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceMetadata {
    /// Name of the test that generated this trace
    pub test_name: String,
    /// Total number of trace entries
    pub entry_count: usize,
    /// Unix timestamp when trace was collected
    pub timestamp: u64,
    /// Duration of test execution in milliseconds
    pub duration_ms: u64,
    /// Number of frames executed
    pub frames_executed: u32,
    /// Emulator model (CGB, DMG, etc.)
    pub model: String,
    /// Failure reason if test failed
    pub failure_reason: Option<String>,
    /// Buffer size used for collection
    pub buffer_size: usize,
    /// Whether the buffer was truncated (filled before test ended)
    pub truncated: bool,
    /// Schema version for trace format
    pub schema_version: String,
}

impl TraceMetadata {
    /// Create new trace metadata
    #[must_use]
    pub fn new(test_name: String, model: String, buffer_size: usize) -> Self {
        Self {
            test_name,
            entry_count: 0,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            duration_ms: 0,
            frames_executed: 0,
            model,
            failure_reason: None,
            buffer_size,
            truncated: false,
            schema_version: "1.0".to_string(),
        }
    }
}

/// A custom tracing subscriber for test debugging
///
/// This subscriber captures all tracing events during test execution
/// and provides methods to export them or clear them.
#[derive(Clone)]
pub struct TestTracer {
    buffer: Arc<Mutex<VecDeque<TraceEntry>>>,
    max_entries: usize,
}

impl TestTracer {
    pub fn new(max_entries: usize) -> Self {
        Self {
            buffer: Arc::new(Mutex::new(VecDeque::with_capacity(max_entries.max(100)))),
            max_entries,
        }
    }

    pub fn with_default_capacity() -> Self {
        Self::new(10_000) // Default to 10k entries
    }

    /// Get a clone of the current trace entries
    pub fn get_traces(&self) -> Vec<TraceEntry> {
        self.buffer.lock().unwrap().clone().into()
    }

    /// Clear all trace entries
    pub fn clear(&self) {
        let mut buffer = self.buffer.lock().unwrap();
        buffer.clear();
    }

    /// Get the number of trace entries
    pub fn len(&self) -> usize {
        self.buffer.lock().unwrap().len()
    }

    /// Check if the trace buffer is empty
    pub fn is_empty(&self) -> bool {
        self.buffer.lock().unwrap().is_empty()
    }

    /// Get the shared buffer reference for use in test runner
    pub fn buffer(&self) -> Arc<Mutex<VecDeque<TraceEntry>>> {
        Arc::clone(&self.buffer)
    }

    /// Export traces in JSON Lines format (one JSON object per line)
    ///
    /// This format is machine-friendly and can be processed with standard Unix tools.
    /// Each line is a complete, flattened JSON object.
    ///
    /// # Errors
    ///
    /// Returns an error if file creation or writing fails.
    pub fn export_jsonl(&self, path: &std::path::Path) -> std::io::Result<()> {
        use std::io::Write;

        let file = std::fs::File::create(path)?;
        let mut writer = std::io::BufWriter::new(file);

        let traces = self.get_traces();
        for entry in traces {
            // Flatten the entry structure for easier querying
            // Extract common fields from the nested fields HashMap
            let flat_entry = serde_json::json!({
                "target": entry.target,
                "level": entry.level,
                "timestamp": entry.timestamp,
                "pc": entry.fields.get("pc").and_then(|v| v.as_u64()),
                "instruction": entry.fields.get("instruction").and_then(|v| v.as_str()),
                "a": entry.fields.get("a").and_then(|v| v.as_u64()),
                "f": entry.fields.get("f").and_then(|v| v.as_u64()),
                "b": entry.fields.get("b").and_then(|v| v.as_u64()),
                "c": entry.fields.get("c").and_then(|v| v.as_u64()),
                "d": entry.fields.get("d").and_then(|v| v.as_u64()),
                "e": entry.fields.get("e").and_then(|v| v.as_u64()),
                "h": entry.fields.get("h").and_then(|v| v.as_u64()),
                "l": entry.fields.get("l").and_then(|v| v.as_u64()),
                "sp": entry.fields.get("sp").and_then(|v| v.as_u64()),
                "cycles": entry.fields.get("cycles").and_then(|v| v.as_u64()),
            });

            serde_json::to_writer(&mut writer, &flat_entry)?;
            writeln!(writer)?;
        }

        writer.flush()?;
        Ok(())
    }

    /// Export metadata about the trace collection
    ///
    /// # Errors
    ///
    /// Returns an error if file creation or writing fails.
    pub fn export_metadata(
        metadata: &TraceMetadata,
        path: &std::path::Path,
    ) -> std::io::Result<()> {
        let json = serde_json::to_string_pretty(metadata)?;
        std::fs::write(path, json)?;
        Ok(())
    }
}

impl<C> Layer<C> for TestTracer
where
    C: tracing::Subscriber + for<'lookup> LookupSpan<'lookup>,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, C>) {
        // Only capture events from ceres-core components
        if event.metadata().target().starts_with("ceres")
            || event.metadata().target() == "cpu_execution"
        {
            let mut buffer = self.buffer.lock().unwrap();

            // Create a new trace entry from the event
            let mut fields = std::collections::HashMap::new();

            // Visit the event's fields to extract values
            let mut visitor = FieldVisitor::new(&mut fields);
            event.record(&mut visitor);

            let trace_entry = TraceEntry {
                target: event.metadata().target().to_string(),
                level: format!("{}", event.metadata().level()),
                name: event.metadata().name().to_string(),
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64,
                fields,
            };

            // Add to buffer, removing oldest entry if at capacity
            if buffer.len() >= self.max_entries {
                buffer.pop_front();
            }
            buffer.push_back(trace_entry);
        }
    }
}

/// Visitor to extract field values from tracing events
struct FieldVisitor<'a> {
    fields: &'a mut std::collections::HashMap<String, serde_json::Value>,
}

impl<'a> FieldVisitor<'a> {
    fn new(fields: &'a mut std::collections::HashMap<String, serde_json::Value>) -> Self {
        Self { fields }
    }
}

impl<'a> tracing::field::Visit for FieldVisitor<'a> {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.fields.insert(
            field.name().to_string(),
            serde_json::Value::String(format!("{:?}", value)),
        );
    }

    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        self.fields.insert(
            field.name().to_string(),
            serde_json::Value::Number(value.into()),
        );
    }

    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        self.fields.insert(
            field.name().to_string(),
            serde_json::Value::Number(serde_json::Number::from(value)),
        );
    }

    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        self.fields
            .insert(field.name().to_string(), serde_json::Value::Bool(value));
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        self.fields.insert(
            field.name().to_string(),
            serde_json::Value::String(value.to_string()),
        );
    }
}
