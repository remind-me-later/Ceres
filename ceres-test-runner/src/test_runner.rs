//! Test runner infrastructure for executing test ROMs

/// Timeout constants for test suites (in frames at ~59.73 Hz).
pub mod timeouts {
    pub const CGB_ACID2: u32 = 300;
    pub const DMG_ACID2: u32 = 480;
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

        let matches = expected_rgba.as_raw() == actual_rgba;
        if !matches {
            let mut count = 0;
            let mut line_counts = [0usize; 144];
            let mut line_first = [(0u8, [0u8; 4], [0u8; 4]); 144];
            for (idx, (e, a)) in expected_rgba
                .chunks(4)
                .zip(actual_rgba.chunks(4))
                .enumerate()
            {
                if e != a {
                    count += 1;
                    let y = idx / 160;
                    if line_counts[y] == 0 {
                        let mut ea = [0u8; 4];
                        let mut aa = [0u8; 4];
                        ea.copy_from_slice(e);
                        aa.copy_from_slice(a);
                        line_first[y] = (u8::try_from(idx % 160).unwrap_or(u8::MAX), ea, aa);
                    }
                    line_counts[y] += 1;
                    if count <= 10 {
                        let x = idx % 160;
                        eprintln!("Mismatch #{count} at ({x}, {y}): expected {e:?}, got {a:?}");
                    }
                }
            }
            for (y, c) in line_counts.iter().enumerate() {
                if *c > 0 {
                    let (x, e, a) = line_first[y];
                    eprintln!("Scanline {y}: {c} mismatches (first at x={x}: exp={e:?} got={a:?})");
                }
            }
            eprintln!("Total mismatching pixels: {count} / {}", 160 * 144);
        }
        Ok(matches)
    }
}

impl CompletionCheck for ScreenshotCheck {
    fn check(&self, gb: &mut Gb<DummyAudioCallback>) -> Option<TestResult> {
        if gb.take_ld_b_b_breakpoint() {
            match self.compare_screenshot(gb) {
                Ok(true) => Some(TestResult::Passed),
                Ok(false) => Some(TestResult::Failed("Screenshot mismatch".to_string())),
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

/// Check for a result in the registers (Mooneye's protocol, which AGE and
/// SameSuite follow).
///
/// On success they hold the Fibonacci numbers `3, 5, 8, 13, 21, 34` in `B, C,
/// D, E, H, L`. The ROM signals completion with the `ld b, b` breakpoint or by
/// executing an illegal opcode.
pub struct FibonacciCheck;

impl CompletionCheck for FibonacciCheck {
    fn check(&self, gb: &mut Gb<DummyAudioCallback>) -> Option<TestResult> {
        if !(gb.take_ld_b_b_breakpoint() || gb.take_illegal_opcode()) {
            return None;
        }

        let registers = [
            gb.cpu_b(),
            gb.cpu_c(),
            gb.cpu_d(),
            gb.cpu_e(),
            gb.cpu_h(),
            gb.cpu_l(),
        ];
        Some(if registers == [3, 5, 8, 13, 21, 34] {
            TestResult::Passed
        } else {
            TestResult::Failed(format!("registers {registers:02X?}"))
        })
    }
}

/// Check for the result of one of blargg's ROMs.
///
/// The older ones print their result over the serial port ("Passed" or
/// "Failed"); the newer ones leave it in the cartridge RAM instead: a
/// signature (`DE B0 61`) at `$A001`, then the text, and at `$A000` the result
/// code (`$80` while running, 0 on success).
pub struct BlarggCheck;

impl BlarggCheck {
    fn memory_text(gb: &Gb<DummyAudioCallback>) -> String {
        (0xA004..0xA400)
            .map(|address| gb.read_mem(address))
            .take_while(|&byte| byte != 0)
            .map(char::from)
            .collect()
    }
}

impl CompletionCheck for BlarggCheck {
    fn check(&self, gb: &mut Gb<DummyAudioCallback>) -> Option<TestResult> {
        let output = gb.serial_output();
        if output.contains("Passed") {
            return Some(TestResult::Passed);
        }
        if output.contains("Failed") {
            return Some(TestResult::Failed(format!("serial output: {output:?}")));
        }

        let signature = [
            gb.read_mem(0xA001),
            gb.read_mem(0xA002),
            gb.read_mem(0xA003),
        ];
        if signature != [0xDE, 0xB0, 0x61] {
            return None;
        }
        match gb.read_mem(0xA000) {
            0x80 => None,
            0 => Some(TestResult::Passed),
            code => Some(TestResult::Failed(format!(
                "result code {code:#04X}: {:?}",
                Self::memory_text(gb)
            ))),
        }
    }

    fn on_timeout(&self, gb: &mut Gb<DummyAudioCallback>) -> TestResult {
        TestResult::Failed(format!(
            "Timeout reached, serial output: {:?}, cartridge RAM text: {:?}",
            gb.serial_output(),
            Self::memory_text(gb)
        ))
    }
}

/// Frames between two looks at the screen.
const SCREEN_CHECK_INTERVAL: u32 = 30;

/// Rank of the colours of an RGBA image, brightest first: two images that
/// differ only in their palette have the same ranks.
fn rank_image(rgba: &[u8]) -> Vec<u8> {
    let mut colors: Vec<[u8; 3]> = rgba.chunks(4).map(|p| [p[0], p[1], p[2]]).collect();
    colors.sort_by_key(|c| core::cmp::Reverse(u32::from(c[0]) + u32::from(c[1]) + u32::from(c[2])));
    colors.dedup();
    rgba.chunks(4)
        .map(|p| {
            colors
                .iter()
                .position(|c| *c == [p[0], p[1], p[2]])
                .map_or(u8::MAX, |i| u8::try_from(i).unwrap_or(u8::MAX))
        })
        .collect()
}

/// Check that the screen ends up looking like a reference screenshot, whatever
/// the palette.
///
/// The screen is looked at every half second; the check passes as soon as it
/// matches twice in a row (the ROMs keep their final screen) or when the time
/// is up.
pub struct RankedScreenshotCheck {
    expected: Vec<u8>,
    frames: core::cell::Cell<u32>,
    matched_last_time: core::cell::Cell<bool>,
}

impl RankedScreenshotCheck {
    /// # Errors
    ///
    /// Returns an error if the screenshot cannot be read or is not 160x144.
    #[inline]
    pub fn new(screenshot: &std::path::Path) -> Result<Self> {
        let image = image::open(screenshot)?.to_rgba8();
        anyhow::ensure!(
            image.width() == u32::from(ceres_core::PX_WIDTH)
                && image.height() == u32::from(ceres_core::PX_HEIGHT),
            "reference screenshot is not {}x{}",
            ceres_core::PX_WIDTH,
            ceres_core::PX_HEIGHT
        );
        Ok(Self {
            expected: rank_image(image.as_raw()),
            frames: core::cell::Cell::new(0),
            matched_last_time: core::cell::Cell::new(false),
        })
    }

    fn matches(&self, gb: &Gb<DummyAudioCallback>) -> bool {
        rank_image(gb.pixel_data_rgba()) == self.expected
    }
}

impl CompletionCheck for RankedScreenshotCheck {
    fn check(&self, gb: &mut Gb<DummyAudioCallback>) -> Option<TestResult> {
        self.frames.set(self.frames.get() + 1);
        if !self.frames.get().is_multiple_of(SCREEN_CHECK_INTERVAL) {
            return None;
        }
        let matches = self.matches(gb);
        if matches && self.matched_last_time.get() {
            return Some(TestResult::Passed);
        }
        self.matched_last_time.set(matches);
        None
    }

    fn on_timeout(&self, gb: &mut Gb<DummyAudioCallback>) -> TestResult {
        if self.matches(gb) {
            TestResult::Passed
        } else {
            TestResult::Failed("Screenshot mismatch".to_string())
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
