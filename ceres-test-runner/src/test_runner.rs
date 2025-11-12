//! Test runner infrastructure for executing test ROMs

/// Timeout constants for test suites (in frames at ~59.73 Hz).
pub mod timeouts {
    pub const CPU_INSTRS: u32 = 2091;
    pub const INSTR_TIMING: u32 = 250;
    pub const MEM_TIMING: u32 = 300;
    pub const MEM_TIMING_2: u32 = 360;
    pub const INTERRUPT_TIME: u32 = 240;
    pub const HALT_BUG: u32 = 330;
    pub const CGB_ACID2: u32 = 300;
    pub const DMG_ACID2: u32 = 480;
    pub const RTC3TEST_BASIC: u32 = 1050;
    pub const RTC3TEST_RANGE: u32 = 750;
}

use crate::test_tracer::TestTracer;
use anyhow::Result;
use ceres_core::{AudioCallback, Button, Gb, GbBuilder, Model, Sample};

const DEFAULT_TIMEOUT_FRAMES: u32 = 1792;

/// Format for trace export
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraceFormat {
    /// Structured JSON with metadata wrapper
    Json,
    /// JSON Lines format - one JSON object per line (default, machine-friendly)
    JsonLines,
}

impl Default for TraceFormat {
    #[inline]
    fn default() -> Self {
        TraceFormat::JsonLines
    }
}

/// Action to perform on a button
#[derive(Clone, Copy)]
pub enum ButtonAction {
    /// Press the button
    Press,
    /// Release the button
    Release,
}

/// A scheduled button event
#[derive(Clone, Copy)]
pub struct ButtonEvent {
    /// Frame number when this event should occur
    pub frame: u32,
    /// Button to affect
    pub button: Button,
    /// Action to perform
    pub action: ButtonAction,
}

/// A dummy audio callback for headless testing
#[derive(Default)]
struct DummyAudioCallback;

impl AudioCallback for DummyAudioCallback {
    fn audio_sample(&self, _l: Sample, _r: Sample) {}
}

/// Result of running a test ROM
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TestResult {
    /// Test failed with a message
    Failed(String),
    /// Test passed successfully
    Passed,
    /// Test timed out
    Timeout,
    /// Test result is unknown (couldn't parse output)
    Unknown,
}

/// Configuration for running a test ROM
pub struct TestConfig {
    pub capture_serial: bool,
    pub model: Model,
    pub timeout_frames: u32,
    pub expected_screenshot: Option<std::path::PathBuf>,
    pub button_events: Vec<ButtonEvent>,
    /// Enable trace collection during test execution
    pub enable_trace: bool,
    /// Export trace to JSON file on test failure
    pub export_trace_on_failure: bool,
    /// Trace buffer size (number of instructions to keep)
    pub trace_buffer_size: usize,
    /// Format for trace export
    pub trace_format: TraceFormat,
    /// Test name for metadata (auto-detected if not provided)
    pub test_name: Option<String>,
    /// Generate companion index file for fast lookups
    pub generate_index: bool,
    /// Checkpoint interval for index generation (every N instructions)
    pub checkpoint_interval: usize,
}

impl Default for TestConfig {
    #[inline]
    fn default() -> Self {
        Self {
            capture_serial: true,
            model: Model::Cgb,
            timeout_frames: DEFAULT_TIMEOUT_FRAMES,
            expected_screenshot: None,
            button_events: Vec::new(),
            enable_trace: false,
            export_trace_on_failure: false,
            trace_buffer_size: 1000,
            trace_format: TraceFormat::default(),
            test_name: None,
            generate_index: true, // Generate index by default for better analysis
            checkpoint_interval: 1000, // Checkpoint every 1000 instructions
        }
    }
}

/// A test runner for executing Game Boy test ROMs
pub struct TestRunner {
    config: TestConfig,
    frames_run: u32,
    gb: Gb<DummyAudioCallback>,
    serial_output: String,
    tracer: Option<TestTracer>,
    _guard: Option<tracing::subscriber::DefaultGuard>,
    start_time: std::time::Instant,
}

impl TestRunner {
    /// Check if the test has completed and parse the result
    ///
    /// Tests can complete in two ways:
    /// 1. Screenshot/serial comparison combined with breakpoint detection: Some test ROMs
    ///    (like cgb-acid2 and dmg-acid2) use the `ld b, b` instruction as a debug breakpoint
    ///    to signal test completion. When the screenshot matches AND a breakpoint was hit,
    ///    the test completes immediately without waiting for the timeout.
    /// 2. Screenshot/serial comparison: If a screenshot or serial output matches the expected result.
    ///
    /// The timeout mechanism in `run()` serves as a safety net to catch infinitely looping tests
    /// that never signal completion.
    fn check_completion(&mut self) -> Option<TestResult> {
        // Check if the `ld b, b` breakpoint was hit (signals test completion for Acid2 tests)
        let breakpoint_hit = self.gb.check_and_reset_ld_b_b_breakpoint();

        // If we have an expected screenshot, compare it
        if let Some(ref screenshot_path) = self.config.expected_screenshot {
            match self.compare_screenshot(screenshot_path) {
                Ok(true) => {
                    // Screenshot matches - test passed!
                    // If breakpoint was hit, this is a proper completion signal (e.g., Acid2 tests)
                    // If not, we're still waiting for the breakpoint or timeout (e.g., Blargg tests)
                    return Some(TestResult::Passed);
                }
                Ok(false) if breakpoint_hit => {
                    // Breakpoint hit but screenshot doesn't match yet - keep running
                    // This handles cases where the test uses ld b,b internally but isn't done yet
                }
                Err(e) if breakpoint_hit => {
                    // Error comparing screenshot after breakpoint
                    return Some(TestResult::Failed(format!(
                        "Screenshot comparison error: {e}"
                    )));
                }
                _ => {}
            }
        }

        let output = self.serial_output.trim();

        if output.contains("Passed") {
            return Some(TestResult::Passed);
        }

        if output.contains("Failed") || output.contains("Error") {
            return Some(TestResult::Failed(output.into()));
        }

        None
    }

    /// Compare the current screen against an expected screenshot
    fn compare_screenshot(&self, expected_path: &std::path::Path) -> Result<bool> {
        let expected_img = image::open(expected_path)?;
        let expected_rgba = expected_img.to_rgba8();
        let actual_rgba = self.gb.pixel_data_rgba();

        if expected_rgba.width() != u32::from(ceres_core::PX_WIDTH)
            || expected_rgba.height() != u32::from(ceres_core::PX_HEIGHT)
        {
            return Ok(false);
        }

        Ok(expected_rgba.as_raw() == actual_rgba)
    }

    /// Get the number of frames run
    #[must_use]
    #[inline]
    pub const fn frames_run(&self) -> u32 {
        self.frames_run
    }

    /// Create a new test runner with the given ROM
    ///
    /// # Errors
    ///
    /// Returns an error if the ROM is invalid or cannot be loaded.
    #[inline]
    pub fn new(rom: Vec<u8>, config: TestConfig) -> Result<Self> {
        let rom_boxed = rom.into_boxed_slice();

        let mut gb = GbBuilder::new(48000, DummyAudioCallback)
            .with_model(config.model)
            .with_rom(rom_boxed)?
            .build();

        gb.set_color_correction_mode(ceres_core::ColorCorrectionMode::Disabled);

        // Set up tracing infrastructure if enabled
        let (tracer, guard) = if config.enable_trace {
            // Enable tracing on the GB instance
            gb.set_trace_enabled(true);

            // Create the tracer layer
            let tracer = TestTracer::new(config.trace_buffer_size);

            // Set up the tracing subscriber with our tracer layer
            use tracing_subscriber::{EnvFilter, layer::SubscriberExt};

            // Create a filter that allows TRACE level for ceres modules
            let filter = EnvFilter::new("ceres=trace,cpu_execution=trace");

            let subscriber = tracing_subscriber::registry()
                .with(filter)
                .with(tracer.clone());

            // Install the subscriber for this test
            let guard = tracing::subscriber::set_default(subscriber);

            (Some(tracer), Some(guard))
        } else {
            (None, None)
        };

        Ok(Self {
            config,
            frames_run: 0,
            gb,
            serial_output: String::new(),
            tracer,
            _guard: guard,
            start_time: std::time::Instant::now(),
        })
    }

    /// Run the test ROM and return the result
    #[inline]
    pub fn run(&mut self) -> TestResult {
        while self.frames_run < self.config.timeout_frames {
            self.run_frame();
            self.frames_run += 1;

            // Check if test has completed (via breakpoint or screenshot/serial match)
            if let Some(result) = self.check_completion() {
                if result != TestResult::Passed {
                    // Export trace on failure if configured
                    if self.config.export_trace_on_failure {
                        self.export_trace_if_enabled(&result);
                    }
                } else {
                    // For passed tests, clear traces if not saving all
                    if let Some(ref tracer) = self.tracer {
                        tracer.clear(); // Clear traces to maintain performance for successful tests
                    }
                }
                return result;
            }
        }

        // Handle timeout case
        let result = TestResult::Timeout;
        if self.config.export_trace_on_failure {
            self.export_trace_if_enabled(&result);
        } else if let Some(ref tracer) = self.tracer {
            tracer.clear(); // Clear traces for timeout if not configured to export
        }

        result
    }

    /// Run a single frame of emulation
    fn run_frame(&mut self) {
        // Process any scheduled button events for this frame
        for event in &self.config.button_events {
            if event.frame == self.frames_run {
                match event.action {
                    ButtonAction::Press => self.gb.press(event.button),
                    ButtonAction::Release => self.gb.release(event.button),
                }
            }
        }

        self.gb.run_frame();

        if self.config.capture_serial {
            let output = self.gb.serial_output();
            if output.len() != self.serial_output.len() {
                self.serial_output = String::from(output);
            }
        }
    }

    /// Get the serial output captured so far
    #[must_use]
    #[inline]
    pub fn serial_output(&self) -> &str {
        &self.serial_output
    }

    /// Export trace to JSON/JSONL file if trace collection is enabled
    ///
    /// The trace file is saved to `target/traces/<test_name>_<timestamp>_trace.<ext>`
    /// Metadata is saved to `target/traces/<test_name>_<timestamp>_trace.meta.json`
    /// Index is saved to `target/traces/<test_name>_<timestamp>_trace.index.json` (if enabled)
    fn export_trace_if_enabled(&self, result: &TestResult) {
        if !self.config.enable_trace {
            return;
        }

        // Get the tracer and export the collected traces
        if let Some(ref tracer) = self.tracer {
            if tracer.is_empty() {
                eprintln!("No trace data collected for export.");
                return;
            }

            // Create traces directory
            let trace_dir = std::path::PathBuf::from("target/traces");
            if let Err(e) = std::fs::create_dir_all(&trace_dir) {
                eprintln!("Failed to create trace directory: {e}");
                return;
            }

            // Generate trace filename with timestamp and optional test name
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();

            let test_name = self.config.test_name.as_deref().unwrap_or("unknown_test");
            let base_name = format!("{}_{}", test_name, timestamp);

            // Choose extension based on format
            let extension = match self.config.trace_format {
                TraceFormat::Json => "json",
                TraceFormat::JsonLines => "jsonl",
            };

            let trace_path = trace_dir.join(format!("{base_name}_trace.{extension}"));
            let meta_path = trace_dir.join(format!("{base_name}_trace.meta.json"));
            let index_path = trace_dir.join(format!("{base_name}_trace.index.json"));

            // Build metadata
            use crate::test_tracer::TraceMetadata;

            let model_str = match self.config.model {
                Model::Dmg => "DMG",
                Model::Mgb => "MGB",
                Model::Cgb => "CGB",
                _ => "UNKNOWN",
            };

            let mut metadata = TraceMetadata::new(
                test_name.to_string(),
                model_str.to_string(),
                self.config.trace_buffer_size,
            );

            metadata.entry_count = tracer.len();
            metadata.duration_ms = self.start_time.elapsed().as_millis() as u64;
            metadata.frames_executed = self.frames_run;
            metadata.truncated = tracer.len() >= self.config.trace_buffer_size;
            metadata.failure_reason = match result {
                TestResult::Failed(msg) => Some(msg.clone()),
                TestResult::Timeout => Some("Timeout".to_string()),
                TestResult::Unknown => Some("Unknown".to_string()),
                TestResult::Passed => None,
            };

            // Export metadata
            if let Err(e) = TestTracer::export_metadata(&metadata, &meta_path) {
                eprintln!("Failed to export trace metadata: {e}");
            } else {
                println!("Trace metadata exported to: {}", meta_path.display());
            }

            // Export traces based on format
            match self.config.trace_format {
                TraceFormat::JsonLines => {
                    if let Err(e) = tracer.export_jsonl(&trace_path) {
                        eprintln!("Failed to export trace: {e}");
                    } else {
                        println!("Trace exported to: {}", trace_path.display());

                        // Generate index if enabled (only for JSONL format)
                        if self.config.generate_index {
                            println!("Generating trace index...");
                            use crate::trace_index::TraceIndex;

                            match TraceIndex::build_from_jsonl(
                                &trace_path,
                                self.config.checkpoint_interval,
                            ) {
                                Ok(index) => {
                                    let stats = index.stats();
                                    println!(
                                        "Index stats: {} entries, {} unique PCs, {} unique instructions, {} checkpoints",
                                        stats.total_entries,
                                        stats.unique_pcs,
                                        stats.unique_instructions,
                                        stats.checkpoint_count
                                    );

                                    if let Err(e) = index.export(&index_path) {
                                        eprintln!("Failed to export index: {e}");
                                    } else {
                                        println!(
                                            "Trace index exported to: {}",
                                            index_path.display()
                                        );
                                    }
                                }
                                Err(e) => {
                                    eprintln!("Failed to build trace index: {e}");
                                }
                            }
                        }
                    }
                }
                TraceFormat::Json => {
                    // Export structured JSON format
                    use crate::test_tracer::TraceEntry;
                    let traces = tracer.get_traces();

                    #[derive(serde::Serialize)]
                    struct TraceExport {
                        metadata: TraceMetadata,
                        entries: Vec<TraceEntry>,
                    }

                    let export = TraceExport {
                        metadata,
                        entries: traces,
                    };

                    match serde_json::to_string_pretty(&export) {
                        Ok(json) => {
                            if let Err(e) = std::fs::write(&trace_path, json) {
                                eprintln!("Failed to write trace file: {e}");
                            } else {
                                println!("Trace exported to: {}", trace_path.display());
                            }
                        }
                        Err(e) => {
                            eprintln!("Failed to serialize trace: {e}");
                        }
                    }
                }
            }
        } else {
            eprintln!("Tracing not configured for this test run");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runner_creation() {
        let mut rom = vec![0; 0x8000];

        rom[0x148] = 0;
        rom[0x149] = 0;
        rom[0x147] = 0;

        let mut checksum: u8 = 0;
        for byte in &rom[0x134..0x14D] {
            checksum = checksum.wrapping_sub(*byte).wrapping_sub(1);
        }
        rom[0x14D] = checksum;

        let config = TestConfig::default();
        let result = TestRunner::new(rom, config);

        assert!(result.is_ok(), "Failed to create test runner");
    }
}
