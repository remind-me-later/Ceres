//! Test runner infrastructure for executing test ROMs

/// Timeout constants for Blargg CPU test suites (in frames).
///
/// Based on the official documentation at:
/// <https://github.com/c-sp/game-boy-test-roms/blob/master/src/howto/blargg.md>
///
/// The documented times indicate how long tests take to complete on real hardware.
/// We use a generous 60-second timeout for all tests to account for emulation overhead
/// and varying system performance.
///
/// Note: Game Boy runs at approximately 59.73 Hz (not 60 Hz), which equals
/// about 70224 clock cycles per frame (4194304 Hz / 59.73 Hz).
///
/// Official hardware completion times:
/// - `cpu_instrs`: 55s (DMG), 31s (CGB)
/// - `instr_timing`: 1s (DMG/CGB)
/// - `mem_timing`: 3s (DMG/CGB)
/// - `mem_timing-2`: 2s (DMG/CGB)
/// - `interrupt_time`: <1s (DMG/CGB)
pub mod timeouts {
    /// Game Boy frame rate: ~59.73 Hz (4194304 Hz / 70224 cycles per frame)
    /// For timeout calculations, we use 59.73 frames per second.
    /// We use 2x the documented hardware time to account for emulation overhead.
    ///
    /// `cpu_instrs` tests (hardware: 55s DMG, 31s CGB)
    /// Timeout: 62 seconds (2x CGB time) ≈ 3703 frames
    pub const CPU_INSTRS: u32 = 3703;

    /// `instr_timing` test (hardware: 1s DMG/CGB)
    /// Timeout: 10 seconds (generous buffer) ≈ 597 frames
    pub const INSTR_TIMING: u32 = 597;

    /// `mem_timing` tests (hardware: 3s DMG/CGB)
    /// Timeout: 6 seconds ≈ 358 frames
    pub const MEM_TIMING: u32 = 358;

    /// `mem_timing-2` tests (hardware: 2s DMG/CGB documented, but actually 4s)
    /// Timeout: 8 seconds ≈ 478 frames
    pub const MEM_TIMING_2: u32 = 478;

    /// `interrupt_time` test (hardware: <1s DMG/CGB documented, but actually 2s)
    /// Timeout: 4 seconds ≈ 239 frames
    pub const INTERRUPT_TIME: u32 = 239;
}

use anyhow::Result;
use ceres_core::{AudioCallback, Gb, GbBuilder, Model, Sample};

/// Maximum number of frames to run before timing out (default: 30 seconds)
/// Game Boy runs at ~59.73 Hz, so 30 seconds ≈ 1792 frames
const DEFAULT_TIMEOUT_FRAMES: u32 = 1792;

/// A dummy audio callback for headless testing
#[derive(Default)]
struct DummyAudioCallback;

impl AudioCallback for DummyAudioCallback {
    fn audio_sample(&self, _l: Sample, _r: Sample) {
        // Discard audio samples during testing
    }
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
    /// Whether to capture serial output
    pub capture_serial: bool,
    /// Game Boy model to use
    pub model: Model,
    /// Maximum number of frames before timeout
    pub timeout_frames: u32,
}

impl Default for TestConfig {
    #[inline]
    fn default() -> Self {
        Self {
            capture_serial: true,
            model: Model::Cgb,
            timeout_frames: DEFAULT_TIMEOUT_FRAMES,
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
        // Blargg tests output "Passed" or an error message via serial
        let output = self.serial_output.trim();

        if output.contains("Passed") {
            return Some(TestResult::Passed);
        }

        // Check for common failure patterns
        if output.contains("Failed") || output.contains("Error") {
            return Some(TestResult::Failed(output.into()));
        }

        // Some tests output specific error codes
        if !output.is_empty() && !output.contains("Running") {
            // Could be a test result we haven't parsed yet
            // For now, continue running
        }

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

        let gb = GbBuilder::new(48000, DummyAudioCallback)
            .with_model(config.model)
            .with_rom(rom_boxed)?
            .build();

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

            // Check if test has completed
            if let Some(result) = self.check_completion() {
                return result;
            }

            self.frames_run += 1;
        }

        TestResult::Timeout
    }

    /// Run a single frame of emulation
    fn run_frame(&mut self) {
        self.gb.run_frame();

        // Capture serial output if enabled
        if self.config.capture_serial {
            // The Gb serial buffer accumulates, so we just copy the entire string
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
        // Create a minimal valid ROM header
        let mut rom = vec![0; 0x8000]; // 32KB ROM

        // Set ROM size to 32KB (value 0)
        rom[0x148] = 0;
        // Set RAM size to none
        rom[0x149] = 0;
        // Set cartridge type to ROM only
        rom[0x147] = 0;

        // Calculate header checksum
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
