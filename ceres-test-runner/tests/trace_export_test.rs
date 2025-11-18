//! Test that verifies Chrome Trace Event Format export

use ceres_test_runner::{load_test_rom, test_runner::{TestConfig, TestRunner}};

#[test]
#[ignore] // Ignore by default since it generates trace files
fn test_chrome_trace_export() {
    use tracing_subscriber::{layer::SubscriberExt, EnvFilter};
    
    // Create traces directory in target/ (already gitignored)
    // Use absolute path from CARGO_MANIFEST_DIR to avoid nested directories
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let trace_dir = manifest_dir.join("target/traces");
    std::fs::create_dir_all(&trace_dir).unwrap();
    
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let trace_path = trace_dir.join(format!("test_chrome_trace_export_{timestamp}.json"));
    
    eprintln!("Setting up Chrome tracing to: {}", trace_path.display());
    
    // Set up tracing before creating the test runner
    let (chrome_layer, _guard) = tracing_chrome::ChromeLayerBuilder::new()
        .file(&trace_path)
        .include_args(true)
        .build();
    
    // Enable trace level for hardware events (dma, ppu, memory)
    // but keep cpu_execution at info to avoid too much noise
    let filter = EnvFilter::new("trace,cpu_execution=info");
    let subscriber = tracing_subscriber::registry()
        .with(filter)
        .with(chrome_layer);
    
    let _subscriber_guard = tracing::subscriber::set_default(subscriber);
    
    // Load a simple test ROM
    let rom = load_test_rom("blargg/cpu_instrs/individual/01-special.gb")
        .expect("Failed to load test ROM");

    let config = TestConfig {
        timeout_frames: 100, // Run for a short time to generate trace data
        ..TestConfig::default()
    };

    let mut runner = TestRunner::new(rom, config).expect("Failed to create test runner");
    
    // Enable tracing on the emulator
    runner.enable_tracing();
    
    // Run the test - will generate trace data
    let result = runner.run();
    
    // Explicitly drop everything to flush
    drop(runner);
    drop(_subscriber_guard);
    drop(_guard);
    
    // Give it time to flush
    std::thread::sleep(std::time::Duration::from_millis(500));
    
    // The trace will be written to target/traces/test_chrome_trace_export_<timestamp>.json
    eprintln!("Test result: {result:?}");
    eprintln!("Trace file: {}", trace_path.display());
    
    // Check if file was created and show size
    if trace_path.exists() {
        let size = std::fs::metadata(&trace_path).unwrap().len();
        eprintln!("✓ Trace file created: {} bytes", size);
        eprintln!("Open it in ui.perfetto.dev or chrome://tracing");
    } else {
        eprintln!("✗ Trace file was not created");
    }
    
    // Verify file was created
    assert!(trace_path.exists(), "Trace file was not created at {}", trace_path.display());
}

#[test]
#[ignore] // Ignore by default since it generates trace files
fn test_trace_skip_bootrom() {
    use tracing_subscriber::{layer::SubscriberExt, EnvFilter};
    
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let trace_dir = manifest_dir.join("target/traces");
    std::fs::create_dir_all(&trace_dir).unwrap();
    
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let trace_path = trace_dir.join(format!("test_skip_bootrom_{timestamp}.json"));
    
    eprintln!("Setting up Chrome tracing with PC range filtering to: {}", trace_path.display());
    
    let (chrome_layer, _guard) = tracing_chrome::ChromeLayerBuilder::new()
        .file(&trace_path)
        .include_args(true)
        .build();
    
    let filter = EnvFilter::new("trace,cpu_execution=info");
    let subscriber = tracing_subscriber::registry()
        .with(filter)
        .with(chrome_layer);
    
    let _subscriber_guard = tracing::subscriber::set_default(subscriber);
    
    let rom = load_test_rom("blargg/cpu_instrs/individual/01-special.gb")
        .expect("Failed to load test ROM");

    let config = TestConfig {
        timeout_frames: 100,
        ..TestConfig::default()
    };

    let mut runner = TestRunner::new(rom, config).expect("Failed to create test runner");
    
    runner.enable_tracing();
    
    // Skip boot ROM - only trace game code starting at 0x0100
    // This filters out boot ROM instructions (PC < 0x100), keeping only game code
    // Result: ~99.7% of traced instructions are from game code (PC >= 0x100)
    runner.set_trace_pc_range(0x0100, 0xFFFF);
    
    let result = runner.run();
    
    drop(runner);
    drop(_subscriber_guard);
    drop(_guard);
    
    std::thread::sleep(std::time::Duration::from_millis(500));
    
    eprintln!("Test result: {result:?}");
    eprintln!("Trace file: {}", trace_path.display());
    
    if trace_path.exists() {
        let size = std::fs::metadata(&trace_path).unwrap().len();
        eprintln!("✓ Trace file created: {} bytes", size);
        eprintln!("This file should be significantly smaller than a full trace");
        eprintln!("Open it in ui.perfetto.dev or chrome://tracing");
    } else {
        eprintln!("✗ Trace file was not created");
    }
    
    assert!(trace_path.exists(), "Trace file was not created at {}", trace_path.display());
}
