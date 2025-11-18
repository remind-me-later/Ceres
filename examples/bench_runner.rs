use ceres_test_runner::{load_test_rom, test_runner::{TestConfig, TestRunner}};
use std::time::Instant;
use tracing_subscriber::{layer::SubscriberExt, EnvFilter};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let enable_trace = args.contains(&"--trace".to_string());
    let rom_path = "test-roms/blargg/cpu_instrs/individual/01-special.gb";
    
    println!("Loading ROM: {}", rom_path);
    let rom = load_test_rom(rom_path).expect("Failed to load ROM");
    
    let config = TestConfig {
        timeout_frames: 2000, // Run for ~33 seconds (at 60fps)
        ..TestConfig::default()
    };

    // Setup tracing if requested
    let _guard = if enable_trace {
        let (chrome_layer, guard) = tracing_chrome::ChromeLayerBuilder::new()
            .include_args(true)
            .build();
            
        let filter = EnvFilter::new("trace,cpu_execution=info");
        let subscriber = tracing_subscriber::registry()
            .with(filter)
            .with(chrome_layer);
            
        tracing::subscriber::set_global_default(subscriber).unwrap();
        Some(guard)
    } else {
        None
    };

    let mut runner = TestRunner::new(rom, config).expect("Failed to create runner");
    
    if enable_trace {
        runner.enable_tracing();
    }

    let start = Instant::now();
    let result = runner.run();
    let duration = start.elapsed();
    
    println!("Result: {:?}", result);
    println!("Duration: {:.2?}", duration);
}
