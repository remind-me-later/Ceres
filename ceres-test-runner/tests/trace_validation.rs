//! Simple test to validate that the tracing infrastructure is working

use ceres_test_runner::test_tracer::TestTracer;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt};

#[test]
fn test_tracing_infrastructure() {
    // Create a test tracer
    let tracer = TestTracer::new(1000);

    // Set up the tracing subscriber with our tracer layer
    let filter = EnvFilter::new("trace");

    let subscriber = tracing_subscriber::registry()
        .with(filter)
        .with(tracer.clone());

    // Install the subscriber for this test
    let _guard = tracing::subscriber::set_default(subscriber);

    // Emit some test events
    tracing::event!(
        target: "cpu_execution",
        tracing::Level::TRACE,
        pc = 0x100u16,
        instruction = "NOP",
        "TEST_EVENT"
    );

    tracing::event!(
        target: "ceres_test",
        tracing::Level::INFO,
        message = "test message",
        "ANOTHER_EVENT"
    );

    // Give it a moment to process
    std::thread::sleep(std::time::Duration::from_millis(100));

    // Check if traces were collected
    let traces = tracer.get_traces();
    eprintln!("Collected {} trace entries", traces.len());
    for trace in &traces {
        eprintln!(
            "  - target: {}, level: {}, name: {}",
            trace.target, trace.level, trace.name
        );
    }

    assert!(!traces.is_empty(), "No traces were collected!");
    assert_eq!(
        traces.len(),
        2,
        "Expected 2 trace entries, got {}",
        traces.len()
    );
}
