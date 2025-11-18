//! Minimal test to verify tracing-chrome functionality

#[test]
#[ignore]
fn test_minimal_chrome_trace() {
    use tracing_subscriber::layer::SubscriberExt;
    
    // Use absolute path from CARGO_MANIFEST_DIR to avoid path issues
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let trace_dir = manifest_dir.join("target/traces");
    std::fs::create_dir_all(&trace_dir).unwrap();
    let trace_path = trace_dir.join("minimal_test.json");
    
    eprintln!("Creating trace at: {}", trace_path.display());
    
    let (chrome_layer, guard) = tracing_chrome::ChromeLayerBuilder::new()
        .file(&trace_path)
        .include_args(true)
        .build();
    
    let subscriber = tracing_subscriber::registry().with(chrome_layer);
    
    tracing::subscriber::with_default(subscriber, || {
        tracing::info!("Hello from tracing!");
        
        let span = tracing::info_span!("my_span", value = 42);
        let _enter = span.enter();
        tracing::info!("Inside span");
        std::thread::sleep(std::time::Duration::from_millis(10));
    });
    
    drop(guard);
    std::thread::sleep(std::time::Duration::from_millis(200));
    
    eprintln!("Checking if file exists...");
    eprintln!("File exists: {}", trace_path.exists());
    if trace_path.exists() {
        let size = std::fs::metadata(&trace_path).unwrap().len();
        eprintln!("File size: {} bytes", size);
    }
    
    assert!(trace_path.exists(), "Trace file should exist");
}
