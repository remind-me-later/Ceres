//! Test runner infrastructure for executing test ROMs

/// Timeout constants for Blargg test suites (in frames at ~59.73 Hz).
pub mod timeouts {
    pub const CPU_INSTRS: u32 = 2091;
    pub const INSTR_TIMING: u32 = 250;
    pub const MEM_TIMING: u32 = 300;
    pub const MEM_TIMING_2: u32 = 360;
    pub const INTERRUPT_TIME: u32 = 240;
    pub const HALT_BUG: u32 = 330;
    pub const CGB_ACID2: u32 = 300;
    pub const DMG_ACID2: u32 = 240;
}

use anyhow::Result;
use ceres_core::{AudioCallback, Gb, GbBuilder, Model, Sample};

const DEFAULT_TIMEOUT_FRAMES: u32 = 1792;

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
}

impl Default for TestConfig {
    #[inline]
    fn default() -> Self {
        Self {
            capture_serial: true,
            model: Model::Cgb,
            timeout_frames: DEFAULT_TIMEOUT_FRAMES,
            expected_screenshot: None,
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
    fn check_completion(&self) -> Option<TestResult> {
        // If we have an expected screenshot, compare it
        if let Some(ref screenshot_path) = self.config.expected_screenshot
            && matches!(self.compare_screenshot(screenshot_path), Ok(true))
        {
            return Some(TestResult::Passed);
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

            // Check if test has completed
            if let Some(result) = self.check_completion() {
                return result;
            }
        }

        TestResult::Timeout
    }

    /// Run a single frame of emulation
    fn run_frame(&mut self) {
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
