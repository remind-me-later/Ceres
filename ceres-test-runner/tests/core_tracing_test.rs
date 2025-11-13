//! Test that verifies the core emulator emits tracing events

use ceres_core::{AudioCallback, GbBuilder, Sample};
use ceres_test_runner::{load_test_rom, test_tracer::TestTracer};
use tracing_subscriber::{EnvFilter, layer::SubscriberExt};

struct DummyAudio;
impl AudioCallback for DummyAudio {
    fn audio_sample(&self, _l: Sample, _r: Sample) {}
}

#[test]
#[expect(clippy::similar_names)]
fn test_core_emits_trace_events() {
    // Load a simple test ROM
    let rom = load_test_rom("blargg/cpu_instrs/individual/01-special.gb")
        .expect("Failed to load test ROM");

    // Create a test tracer
    let tracer = TestTracer::new(10000);

    // Set up the tracing subscriber with our tracer layer
    let filter = EnvFilter::new("ceres=trace,cpu_execution=trace");

    let subscriber = tracing_subscriber::registry()
        .with(filter)
        .with(tracer.clone());

    // Install the subscriber
    let _guard = tracing::subscriber::set_default(subscriber);

    // Create and run the emulator for a short time
    let mut gb = GbBuilder::new(48000, DummyAudio)
        .with_rom(rom.into_boxed_slice())
        .expect("Failed to create GB")
        .build();

    gb.set_trace_enabled(true);

    // Run for just 100 instructions
    for _ in 0..100 {
        gb.run_cpu();
    }

    // Check if traces were collected
    let traces = tracer.get_traces();
    eprintln!(
        "Collected {} trace entries from emulator core",
        traces.len()
    );

    if !traces.is_empty() {
        eprintln!("First few traces:");
        for (i, trace) in traces.iter().take(5).enumerate() {
            eprintln!(
                "  {}. target: {}, level: {}",
                i + 1,
                trace.target,
                trace.level
            );
            if let Some(pc) = trace.fields.get("pc") {
                eprintln!("     PC: {pc}");
            }
            if let Some(inst) = trace.fields.get("instruction") {
                eprintln!("     Instruction: {inst}");
            }
        }
    }

    assert!(
        !traces.is_empty(),
        "Emulator core did not emit any trace events!"
    );
}
