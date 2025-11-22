//! Debug test for call_cc_timing2 with optimized tracing
//!
//! This test uses ring buffer tracing and PC-range filtering to capture
//! only the relevant execution around OAM DMA and CALL instructions.
//!
//! Usage: cargo test --package ceres-test-runner debug_call_cc_timing2 -- --ignored --nocapture

use ceres_core::Model;
use ceres_test_runner::{
    load_test_rom,
    test_runner::{TestConfig, TestRunner, timeouts},
};
use tracing_subscriber::{layer::SubscriberExt, EnvFilter};

#[test]
#[ignore] // Run explicitly when debugging
fn debug_call_cc_timing2_with_ring_buffer() {
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let trace_dir = manifest_dir.join("target/traces");
    std::fs::create_dir_all(&trace_dir).unwrap();
    
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let trace_path = trace_dir.join(format!("debug_call_cc_timing2_{timestamp}.json"));
    
    eprintln!("=== Debugging call_cc_timing2 with optimized tracing ===");
    eprintln!("Trace file: {}", trace_path.display());
    
    // Set up Chrome tracing with minimal overhead
    let (chrome_layer, _guard) = tracing_chrome::ChromeLayerBuilder::new()
        .file(&trace_path)
        .include_args(true)
        .build();
    
    // Focus on DMA, OAM, and memory events. Keep CPU at info level to reduce noise.
    let filter = EnvFilter::new("dma=trace,memory=trace,cpu_execution=info");
    let subscriber = tracing_subscriber::registry()
        .with(filter)
        .with(chrome_layer);
    
    let _subscriber_guard = tracing::subscriber::set_default(subscriber);
    
    // Load the failing test ROM
    let rom = load_test_rom("mooneye-test-suite/acceptance/call_cc_timing2.gb")
        .expect("Failed to load test ROM");

    let config = TestConfig {
        model: Model::Cgb,
        timeout_frames: timeouts::MOONEYE_ACCEPTANCE,
        use_mooneye_validation: true,
        capture_serial: false,
        test_name: "call_cc_timing2".to_string(),
        ..TestConfig::default()
    };

    let mut runner = TestRunner::new(rom, config).expect("Failed to create test runner");
    
    // Enable tracing on the emulator
    runner.enable_tracing();
    
    // Skip boot ROM - only trace game code starting at 0x0100
    // This dramatically reduces trace size
    runner.set_trace_pc_range(0x0100, 0xFFFF);
    
    eprintln!("Running test with PC range filtering (0x0100-0xFFFF)...");
    let result = runner.run();
    
    eprintln!("\nTest result: {:?}", result);
    eprintln!("Frames run: {}", runner.frames_run());
    
    // Explicitly drop to flush
    drop(runner);
    drop(_subscriber_guard);
    drop(_guard);
    
    std::thread::sleep(std::time::Duration::from_millis(500));
    
    if trace_path.exists() {
        let size = std::fs::metadata(&trace_path).unwrap().len();
        eprintln!("\n✓ Trace file created: {} MB", size / 1_000_000);
        eprintln!("Open it in ui.perfetto.dev to analyze DMA/OAM timing");
        eprintln!("\nQuery suggestions:");
        eprintln!("  1. Find OAM writes: SELECT * FROM slice WHERE cat='memory' AND name LIKE '%OAM%'");
        eprintln!("  2. Find DMA transfers: SELECT * FROM slice WHERE cat='dma'");
        eprintln!("  3. Find CALL instructions: SELECT * FROM slice WHERE name LIKE '%CALL%'");
    } else {
        eprintln!("✗ Trace file was not created");
    }
}

#[test]
#[ignore] // Run explicitly when debugging
fn debug_call_cc_timing2_minimal() {
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let trace_dir = manifest_dir.join("target/traces");
    std::fs::create_dir_all(&trace_dir).unwrap();
    
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let trace_path = trace_dir.join(format!("debug_call_cc_timing2_minimal_{timestamp}.json"));
    
    eprintln!("=== Debugging call_cc_timing2 (MINIMAL - DMA only) ===");
    eprintln!("Trace file: {}", trace_path.display());
    
    let (chrome_layer, _guard) = tracing_chrome::ChromeLayerBuilder::new()
        .file(&trace_path)
        .include_args(true)
        .build();
    
    // Only DMA and OAM-related memory events - absolute minimal
    let filter = EnvFilter::new("dma=trace,oam=trace");
    let subscriber = tracing_subscriber::registry()
        .with(filter)
        .with(chrome_layer);
    
    let _subscriber_guard = tracing::subscriber::set_default(subscriber);
    
    let rom = load_test_rom("mooneye-test-suite/acceptance/call_cc_timing2.gb")
        .expect("Failed to load test ROM");

    let config = TestConfig {
        model: Model::Cgb,
        timeout_frames: timeouts::MOONEYE_ACCEPTANCE,
        use_mooneye_validation: true,
        capture_serial: false,
        test_name: "call_cc_timing2".to_string(),
        ..TestConfig::default()
    };

    let mut runner = TestRunner::new(rom, config).expect("Failed to create test runner");
    runner.enable_tracing();
    runner.set_trace_pc_range(0x0100, 0xFFFF);
    
    eprintln!("Running test (DMA-only tracing)...");
    let result = runner.run();
    
    eprintln!("\nTest result: {:?}", result);
    eprintln!("Frames run: {}", runner.frames_run());
    
    drop(runner);
    drop(_subscriber_guard);
    drop(_guard);
    
    std::thread::sleep(std::time::Duration::from_millis(500));
    
    if trace_path.exists() {
        let size = std::fs::metadata(&trace_path).unwrap().len();
        eprintln!("\n✓ Minimal trace created: {} KB", size / 1_000);
        eprintln!("Open it in ui.perfetto.dev");
    } else {
        eprintln!("✗ Trace file was not created");
    }
}

#[test]
#[ignore] // Run explicitly when debugging
fn debug_call_timing2_comparison() {
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let trace_dir = manifest_dir.join("target/traces");
    std::fs::create_dir_all(&trace_dir).unwrap();
    
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let trace_path = trace_dir.join(format!("debug_call_timing2_comparison_{timestamp}.json"));
    
    eprintln!("=== Debugging call_timing2 (should pass) for comparison ===");
    eprintln!("Trace file: {}", trace_path.display());
    
    let (chrome_layer, _guard) = tracing_chrome::ChromeLayerBuilder::new()
        .file(&trace_path)
        .include_args(true)
        .build();
    
    let filter = EnvFilter::new("dma=trace,memory=trace,cpu_execution=info");
    let subscriber = tracing_subscriber::registry()
        .with(filter)
        .with(chrome_layer);
    
    let _subscriber_guard = tracing::subscriber::set_default(subscriber);
    
    // Load call_timing2 (unconditional, also currently failing according to ignore)
    let rom = load_test_rom("mooneye-test-suite/acceptance/call_timing2.gb")
        .expect("Failed to load test ROM");

    let config = TestConfig {
        model: Model::Cgb,
        timeout_frames: timeouts::MOONEYE_ACCEPTANCE,
        use_mooneye_validation: true,
        capture_serial: false,
        test_name: "call_timing2".to_string(),
        ..TestConfig::default()
    };

    let mut runner = TestRunner::new(rom, config).expect("Failed to create test runner");
    runner.enable_tracing();
    runner.set_trace_pc_range(0x0100, 0xFFFF);
    
    eprintln!("Running test...");
    let result = runner.run();
    
    eprintln!("\nTest result: {:?}", result);
    eprintln!("Frames run: {}", runner.frames_run());
    
    drop(runner);
    drop(_subscriber_guard);
    drop(_guard);
    
    std::thread::sleep(std::time::Duration::from_millis(500));
    
    if trace_path.exists() {
        let size = std::fs::metadata(&trace_path).unwrap().len();
        eprintln!("\n✓ Trace file created: {} MB", size / 1_000_000);
        eprintln!("Compare this with call_cc_timing2 trace to find differences");
    }
}
