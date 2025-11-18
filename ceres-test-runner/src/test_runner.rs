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
    /// Mooneye Test Suite acceptance tests (120 seconds maximum runtime)
    pub const MOONEYE_ACCEPTANCE: u32 = 7160;
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
#[expect(clippy::struct_excessive_bools, reason = "Config struct")]
pub struct TestConfig {
    pub capture_serial: bool,
    pub model: Model,
    pub timeout_frames: u32,
    pub expected_screenshot: Option<std::path::PathBuf>,
    pub button_events: Vec<ButtonEvent>,
    /// Use Mooneye Test Suite validation (Fibonacci register check)
    pub use_mooneye_validation: bool,
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
            use_mooneye_validation: false,
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
    /// 3. Mooneye validation: If Mooneye mode is enabled, check CPU registers for pass/fail.
    ///
    /// The timeout mechanism in `run()` serves as a safety net to catch infinitely looping tests
    /// that never signal completion.
    fn check_completion(&mut self) -> Option<TestResult> {
        // Check if the `ld b, b` breakpoint was hit (signals test completion for Acid2 and Mooneye tests)
        let breakpoint_hit = self.gb.check_and_reset_ld_b_b_breakpoint();

        // If Mooneye validation is enabled and breakpoint was hit, check CPU registers
        if self.config.use_mooneye_validation && breakpoint_hit
            && let Some(result) = self.check_mooneye_result()
        {
            return Some(result);
        }

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

    /// Check if a Mooneye test has passed or failed based on CPU register values.
    ///
    /// Mooneye tests use a specific protocol:
    /// - Pass: B=3, C=5, D=8, E=13, H=21, L=34 (Fibonacci sequence)
    /// - Fail: B=C=D=E=H=L=0x42
    ///
    /// This should only be called after detecting the `ld b, b` breakpoint.
    #[allow(clippy::many_single_char_names)]
    fn check_mooneye_result(&self) -> Option<TestResult> {
        let b = self.gb.cpu_b();
        let c = self.gb.cpu_c();
        let d = self.gb.cpu_d();
        let e = self.gb.cpu_e();
        let h = self.gb.cpu_h();
        let l = self.gb.cpu_l();

        // Check for pass condition (Fibonacci sequence)
        if b == 3 && c == 5 && d == 8 && e == 13 && h == 21 && l == 34 {
            return Some(TestResult::Passed);
        }

        // Check for fail condition (all 0x42)
        if b == 0x42 && c == 0x42 && d == 0x42 && e == 0x42 && h == 0x42 && l == 0x42 {
            return Some(TestResult::Failed(format!(
                "Mooneye test failed (registers: B={b:#04X} C={c:#04X} D={d:#04X} E={e:#04X} H={h:#04X} L={l:#04X})"
            )));
        }

        // If neither condition is met, the test hasn't completed yet
        None
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

        Ok(Self {
            config,
            frames_run: 0,
            gb,
            serial_output: String::new(),
        })
    }
    
    /// Enable tracing on the Game Boy instance
    /// 
    /// This should be called after setting up a tracing subscriber externally.
    pub fn enable_tracing(&mut self) {
        self.gb.set_trace_enabled(true);
    }

    /// Set the PC range for trace collection
    ///
    /// This allows filtering traces to only capture execution within a specific
    /// program counter range. Useful for skipping boot ROM execution or focusing
    /// on specific code sections.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use ceres_test_runner::test_runner::{TestRunner, TestConfig};
    /// # let mut runner = TestRunner::new(vec![], TestConfig::default()).unwrap();
    /// // Skip boot ROM and trace only game code
    /// runner.set_trace_pc_range(0x0100, 0xFFFF);
    /// ```
    pub fn set_trace_pc_range(&mut self, start: u16, end: u16) {
        self.gb.set_trace_pc_range(start, end);
    }

    /// Run the test ROM and return the result
    #[inline]
    pub fn run(&mut self) -> TestResult {
        while self.frames_run < self.config.timeout_frames {
            self.run_frame();
            self.frames_run += 1;

            // Check if test has completed (via breakpoint or screenshot/serial match)
            if let Some(result) = self.check_completion() {
                return result;
            }
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

    /// Read a byte from Game Boy memory
    ///
    /// This is useful for reading test result registers in test ROMs
    /// that don't use serial output or screenshots.
    #[must_use]
    #[inline]
    pub fn read_memory(&self, address: u16) -> u8 {
        self.gb.read_mem(address)
    }

    /// Get the serial output captured so far
    #[must_use]
    #[inline]
    pub fn serial_output(&self) -> &str {
        &self.serial_output
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
