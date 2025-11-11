//! Tracing infrastructure for test debugging
//!
//! This module provides a custom tracing subscriber that captures execution events
//! during test runs and exports them only for failing tests.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use tracing::{Event, Level};
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
}

impl<C> Layer<C> for TestTracer
where
    C: tracing::Subscriber + for<'lookup> LookupSpan<'lookup>,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, C>) {
        // Only capture events from ceres-core components
        if event.metadata().target().starts_with("ceres") {
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
