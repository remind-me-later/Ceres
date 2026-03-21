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
pub struct DummyAudioCallback;

impl AudioCallback for DummyAudioCallback {
    fn audio_sample(&self, _l: Sample, _r: Sample) {}
}

/// Result of running a test ROM
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TestResult {
    /// Test passed successfully
    Passed,
    /// Test failed with a message
    Failed(String),
    /// Test failed with a generic error (e.g. IO error, setup failure)
    Error(String),
}

impl TestResult {
    /// Check if the test result is Passed
    #[must_use]
    pub const fn is_passed(&self) -> bool {
        matches!(self, Self::Passed)
    }
}

/// Trait for defining test completion conditions
pub trait CompletionCheck {
    /// Check if the test has completed
    fn check(&self, gb: &mut Gb<DummyAudioCallback>) -> Option<TestResult>;

    /// Check result when timeout is reached
    fn on_timeout(&self, _gb: &mut Gb<DummyAudioCallback>) -> TestResult {
        TestResult::Failed("Timeout reached".to_string())
    }
}

/// Check for screenshot match
pub struct ScreenshotCheck {
    expected_path: std::path::PathBuf,
}

impl ScreenshotCheck {
    #[must_use]
    pub const fn new(expected_path: std::path::PathBuf) -> Self {
        Self { expected_path }
    }

    fn compare_screenshot(&self, gb: &Gb<DummyAudioCallback>) -> Result<bool> {
        let expected_img = image::open(&self.expected_path)?;
        let expected_rgba = expected_img.to_rgba8();
        let actual_rgba = gb.pixel_data_rgba();

        if expected_rgba.width() != u32::from(ceres_core::PX_WIDTH)
            || expected_rgba.height() != u32::from(ceres_core::PX_HEIGHT)
        {
            return Ok(false);
        }

        Ok(expected_rgba.as_raw() == actual_rgba)
    }
}

impl CompletionCheck for ScreenshotCheck {
    fn check(&self, gb: &mut Gb<DummyAudioCallback>) -> Option<TestResult> {
        if gb.check_and_reset_ld_b_b_breakpoint() {
            match self.compare_screenshot(gb) {
                Ok(true) => Some(TestResult::Passed),
                Ok(false) => None,
                Err(e) => Some(TestResult::Error(format!(
                    "Screenshot comparison error: {e}"
                ))),
            }
        } else {
            None
        }
    }

    fn on_timeout(&self, gb: &mut Gb<DummyAudioCallback>) -> TestResult {
        match self.compare_screenshot(gb) {
            Ok(true) => TestResult::Passed,
            Ok(false) => TestResult::Failed("Screenshot mismatch".to_string()),
            Err(e) => TestResult::Error(format!("Screenshot comparison error: {e}")),
        }
    }
}

/// Configuration for running a test ROM
pub struct TestConfig {
    pub model: Model,
    pub timeout_frames: u32,
    pub button_events: Vec<ButtonEvent>,
    pub test_name: String,
    pub run_bootrom: bool,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            model: Model::DmgB,
            timeout_frames: DEFAULT_TIMEOUT_FRAMES,
            button_events: Vec::new(),
            test_name: "Unknown Test".to_string(),
            run_bootrom: true,
        }
    }
}

/// A test runner for executing Game Boy test ROMs
pub struct TestRunner {
    config: TestConfig,
    frames_run: u32,
    gb: Gb<DummyAudioCallback>,
    check: Box<dyn CompletionCheck>,
}

impl TestRunner {
    /// Get the number of frames run
    #[must_use]
    #[inline]
    pub const fn frames_run(&self) -> u32 {
        self.frames_run
    }

    /// Read a byte from Game Boy memory
    ///
    /// This is useful for reading test result registers in test ROMs
    /// that don't use screenshots.
    #[must_use]
    #[inline]
    pub fn read_memory(&self, address: u16) -> u8 {
        self.gb.read_mem(address)
    }

    /// Get the current pixel data (RGBA format)
    #[must_use]
    #[inline]
    pub const fn pixel_data(&self) -> &[u8] {
        self.gb.pixel_data_rgba()
    }

    /// Create a new test runner with the given ROM
    ///
    /// # Errors
    ///
    /// Returns an error if the ROM is invalid or cannot be loaded.
    #[inline]
    pub fn new(rom: Vec<u8>, config: TestConfig, check: Box<dyn CompletionCheck>) -> Result<Self> {
        let rom_boxed = rom.into_boxed_slice();

        let mut gb = GbBuilder::new(48000, DummyAudioCallback)
            .with_model(config.model)
            .with_run_bootrom(config.run_bootrom)
            .with_rom(rom_boxed)?
            .build();

        gb.set_color_correction_mode(ceres_core::ColorCorrectionMode::Disabled);

        Ok(Self {
            config,
            frames_run: 0,
            gb,
            check,
        })
    }

    /// Run the test ROM and return the result
    #[inline]
    pub fn run(&mut self) -> TestResult {
        while self.frames_run < self.config.timeout_frames {
            self.run_frame();
            self.frames_run += 1;

            if let Some(result) = self.check.check(&mut self.gb) {
                return result;
            }
        }

        self.check.on_timeout(&mut self.gb)
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
    }
}
