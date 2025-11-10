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

use anyhow::Result;
use ceres_core::{AudioCallback, Button, Gb, GbBuilder, Model, Sample};

const DEFAULT_TIMEOUT_FRAMES: u32 = 1792;

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
        }
    }
}

/// A test runner for executing Game Boy test ROMs
pub struct TestRunner {
    config: TestConfig,
    frames_run: u32,
    gb: Gb<DummyAudioCallback>,
    serial_output: String,
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

        // Configure trace collection if enabled
        if config.enable_trace {
            gb.trace_resize(config.trace_buffer_size);
            gb.trace_enable();
        }

        Ok(Self {
            config,
            frames_run: 0,
            gb,
            serial_output: String::new(),
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
                // Export trace on failure if configured
                if result != TestResult::Passed && self.config.export_trace_on_failure {
                    self.export_trace_if_enabled();
                }
                return result;
            }
        }

        // Export trace on timeout if configured
        if self.config.export_trace_on_failure {
            self.export_trace_if_enabled();
        }

        TestResult::Timeout
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

    /// Export trace to JSON file if trace collection is enabled
    ///
    /// The trace file is saved to `target/traces/<timestamp>_trace.json`
    fn export_trace_if_enabled(&self) {
        if !self.config.enable_trace {
            return;
        }

        // Create traces directory
        let trace_dir = std::path::PathBuf::from("target/traces");
        if let Err(e) = std::fs::create_dir_all(&trace_dir) {
            eprintln!("Failed to create trace directory: {e}");
            return;
        }

        // Generate trace filename with timestamp
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let trace_path = trace_dir.join(format!("{timestamp}_trace.json"));

        // Export trace using the ceres_std trace_export module
        match export_trace_json(&self.gb) {
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

/// Export trace buffer as formatted JSON string.
///
/// This is a helper function that wraps the trace export functionality
/// for use in the test runner.
fn export_trace_json<A: AudioCallback>(gb: &Gb<A>) -> Result<String, serde_json::Error> {
    use serde::Serialize;

    #[derive(Debug, Serialize)]
    struct TraceMetadata {
        entry_count: usize,
        buffer_capacity: usize,
        timestamp: u64,
    }

    #[derive(Debug, Serialize)]
    struct TraceExport<'a> {
        metadata: TraceMetadata,
        entries: Vec<&'a ceres_core::trace::TraceEntry>,
    }

    let entries: Vec<_> = gb.trace_entries().collect();

    let export = TraceExport {
        metadata: TraceMetadata {
            entry_count: gb.trace_count(),
            buffer_capacity: gb.trace_capacity(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        },
        entries,
    };

    serde_json::to_string_pretty(&export)
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
