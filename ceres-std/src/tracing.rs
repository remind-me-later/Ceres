use std::{
    collections::BTreeMap,
    fs::File,
    io::{BufWriter, Write},
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{SystemTime, UNIX_EPOCH},
};

use serde::Serialize;
use tracing::{
    field::{Field, Visit},
    Event, Subscriber,
};
use tracing_subscriber::{layer::Context, Layer};

#[derive(Serialize, Clone)]
struct StoredEvent {
    timestamp: u64,
    level: String,
    target: String,
    name: String,
    fields: BTreeMap<String, serde_json::Value>,
    thread_id: String,
}

struct JsonVisitor<'a> {
    fields: &'a mut BTreeMap<String, serde_json::Value>,
}

impl<'a> Visit for JsonVisitor<'a> {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.fields.insert(
            field.name().to_string(),
            serde_json::Value::String(format!("{:?}", value)),
        );
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.fields
            .insert(field.name().to_string(), serde_json::Value::from(value));
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.fields
            .insert(field.name().to_string(), serde_json::Value::from(value));
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.fields
            .insert(field.name().to_string(), serde_json::Value::from(value));
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.fields
            .insert(field.name().to_string(), serde_json::Value::from(value));
    }

    fn record_f64(&mut self, field: &Field, value: f64) {
        self.fields
            .insert(field.name().to_string(), serde_json::Value::from(value));
    }
}

#[derive(Clone)]
pub struct RingBufferLayer {
    buffer: Arc<Mutex<Vec<StoredEvent>>>,
    position: Arc<Mutex<usize>>,
    wrapped: Arc<Mutex<bool>>,
    size: usize,
}

impl RingBufferLayer {
    pub fn new(size: usize) -> Self {
        Self {
            buffer: Arc::new(Mutex::new(Vec::with_capacity(size))),
            position: Arc::new(Mutex::new(0)),
            wrapped: Arc::new(Mutex::new(false)),
            size,
        }
    }

    pub fn flush_to_file<P: AsRef<Path>>(&self, path: P) -> std::io::Result<()> {
        let buffer = self.buffer.lock().unwrap();
        let position = *self.position.lock().unwrap();
        let wrapped = *self.wrapped.lock().unwrap();

        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);

        write!(writer, "[")?;

        let mut first = true;

        let iter = if wrapped {
            // If wrapped, start from position (oldest) to end, then 0 to position
            buffer[position..].iter().chain(buffer[0..position].iter())
        } else {
            // If not wrapped, just 0 to position
            buffer[0..position].iter().chain([].iter())
        };

        for event in iter {
            if !first {
                write!(writer, ",")?;
            }
            first = false;

            // Convert to Chrome Trace Event format
            // https://docs.google.com/document/d/1CvAClvFfyA5R-PhYUmn5OOQtYMH4h6I0nSsKchNAySU/preview
            let chrome_event = serde_json::json!({
                "name": event.name,
                "cat": event.target,
                "ph": "i", // Instant event
                "ts": event.timestamp,
                "pid": 1,
                "tid": event.thread_id,
                "s": "g", // Global scope
                "args": event.fields
            });

            serde_json::to_writer(&mut writer, &chrome_event)?;
        }

        write!(writer, "]")?;
        Ok(())
    }
}

impl<S> Layer<S> for RingBufferLayer
where
    S: Subscriber,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let mut fields = BTreeMap::new();
        let mut visitor = JsonVisitor { fields: &mut fields };
        event.record(&mut visitor);

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64;

        let stored_event = StoredEvent {
            timestamp,
            level: event.metadata().level().to_string(),
            target: event.metadata().target().to_string(),
            name: event.metadata().name().to_string(),
            fields,
            thread_id: format!("{:?}", std::thread::current().id()),
        };

        let mut buffer = self.buffer.lock().unwrap();
        let mut position = self.position.lock().unwrap();
        let mut wrapped = self.wrapped.lock().unwrap();

        if buffer.len() < self.size {
            buffer.push(stored_event);
            *position += 1;
        } else {
            if *position >= self.size {
                *position = 0;
                *wrapped = true;
            }
            buffer[*position] = stored_event;
            *position += 1;
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trigger {
    Pc(u16),
    // Trigger when cycle count reaches this value
    Cycle(u64),
}

struct TriggerVisitor {
    pc: Option<u16>,
    cycles: Option<u64>,
}

impl Visit for TriggerVisitor {
    fn record_debug(&mut self, _field: &Field, _value: &dyn std::fmt::Debug) {}

    fn record_u64(&mut self, field: &Field, value: u64) {
        match field.name() {
            "pc" => self.pc = Some(value as u16),
            "cycles" => self.cycles = Some(value),
            _ => {}
        }
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        match field.name() {
            "pc" => self.pc = Some(value as u16),
            "cycles" => self.cycles = Some(value as u64),
            _ => {}
        }
    }
    
    fn record_bool(&mut self, _field: &Field, _value: bool) {}
    fn record_str(&mut self, _field: &Field, _value: &str) {}
    fn record_f64(&mut self, _field: &Field, _value: f64) {}
}

#[derive(Clone)]
pub struct TriggerLayer<L> {
    layer: L,
    start_trigger: Option<Trigger>,
    stop_trigger: Option<Trigger>,
    active: Arc<AtomicBool>,
    current_cycles: Arc<Mutex<u64>>,
}

impl<L> TriggerLayer<L> {
    pub fn new(layer: L, start_trigger: Option<Trigger>, stop_trigger: Option<Trigger>) -> Self {
        let active = Arc::new(AtomicBool::new(start_trigger.is_none()));
        Self {
            layer,
            start_trigger,
            stop_trigger,
            active,
            current_cycles: Arc::new(Mutex::new(0)),
        }
    }
}

impl<S, L> Layer<S> for TriggerLayer<L>
where
    S: Subscriber,
    L: Layer<S>,
{
    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        let mut current_cycles = self.current_cycles.lock().unwrap();
        let is_active = self.active.load(Ordering::SeqCst);

        // Check triggers if we have any
        if self.start_trigger.is_some() || self.stop_trigger.is_some() {
            let mut visitor = TriggerVisitor {
                pc: None,
                cycles: None,
            };
            event.record(&mut visitor);

            // Update cycle count if present
            if let Some(cycles) = visitor.cycles {
                *current_cycles += cycles;
            }

            let cycles = *current_cycles;

            if !is_active {
                if let Some(trigger) = self.start_trigger {
                    match trigger {
                        Trigger::Pc(target_pc) => {
                            if let Some(pc) = visitor.pc {
                                if pc == target_pc {
                                    self.active.store(true, Ordering::SeqCst);
                                }
                            }
                        }
                        Trigger::Cycle(target_cycle) => {
                            if cycles >= target_cycle {
                                self.active.store(true, Ordering::SeqCst);
                            }
                        }
                    }
                }
            } else if let Some(trigger) = self.stop_trigger {
                match trigger {
                    Trigger::Pc(target_pc) => {
                        if let Some(pc) = visitor.pc {
                            if pc == target_pc {
                                self.active.store(false, Ordering::SeqCst);
                            }
                        }
                    }
                    Trigger::Cycle(target_cycle) => {
                        if cycles >= target_cycle {
                            self.active.store(false, Ordering::SeqCst);
                        }
                    }
                }
            }
        }

        // Reload active state as it might have changed
        if self.active.load(Ordering::SeqCst) {
            self.layer.on_event(event, ctx);
        }
    }
}
