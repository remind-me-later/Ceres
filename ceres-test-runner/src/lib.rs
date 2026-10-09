//! Integration test runner for the Ceres Game Boy emulator
//!
//! This crate provides infrastructure for running Game Boy test ROMs
//! and validating the emulator's accuracy.

pub mod gambatte;
pub mod test_runner;

use test_runner::{
    BlarggCheck, ButtonAction, ButtonEvent, CompletionCheck, FibonacciCheck, RankedScreenshotCheck,
    TestConfig, TestResult, TestRunner,
};

use anyhow::{Context as _, Result};
use ceres_core::Model;
use std::path::{Path, PathBuf};

/// Get the path to the test-roms directory
#[inline]
#[must_use]
pub fn test_roms_dir() -> PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    PathBuf::from(manifest_dir)
        .parent()
        .unwrap()
        .join("external")
        .join("test-roms")
}

/// Check if test ROMs are available
#[inline]
#[must_use]
pub fn test_roms_available() -> bool {
    let test_roms = test_roms_dir();
    test_roms.exists() && test_roms.is_dir()
}

/// Load a test ROM file
#[inline]
pub fn load_test_rom(relative_path: &str) -> Result<Vec<u8>> {
    let rom_path = test_roms_dir().join(relative_path);

    if !rom_path.exists() {
        anyhow::bail!(
            "Test ROM not found: {}\n\n\
             This should not happen as ROMs are automatically downloaded.\n\
             Try: cargo clean --package ceres-test-runner && cargo build --package ceres-test-runner",
            rom_path.display()
        );
    }

    std::fs::read(&rom_path)
        .with_context(|| format!("Failed to read test ROM: {}", rom_path.display()))
}

/// Get path to expected screenshot for a test ROM
#[inline]
#[must_use]
pub fn expected_screenshot_path(relative_path: &str, model: Model) -> Option<PathBuf> {
    let rom_path = test_roms_dir().join(relative_path);
    let rom_dir = rom_path.parent()?;

    // Try model-specific screenshot first (e.g., "instr_timing-cgb.png")
    let rom_stem = rom_path.file_stem()?.to_str()?;
    let model_suffix = match model {
        Model::Cgb0 | Model::CgbA | Model::CgbB | Model::CgbC | Model::CgbD | Model::CgbE => "cgb",
        _ => "dmg", // DMG, MGB, and unknown models use DMG screenshots
    };

    // Try: test-name-model.png
    let model_specific = rom_dir.join(format!("{rom_stem}-{model_suffix}.png"));
    if model_specific.exists() {
        return Some(model_specific);
    }

    // Mealybug Tearoom naming conventions
    // CGB: _cgb_c.png (assuming CGB-C behavior)
    // DMG: _dmg_blob.png
    let p = match model {
        Model::CgbD => {
            // CGB-D has its own references; fall back to the CGB-C ones.
            let d = rom_dir.join(format!("{rom_stem}_cgb_d.png"));
            if d.exists() {
                d
            } else {
                rom_dir.join(format!("{rom_stem}_cgb_c.png"))
            }
        }
        Model::Cgb0 | Model::CgbA | Model::CgbB | Model::CgbC | Model::CgbE => {
            rom_dir.join(format!("{rom_stem}_cgb_c.png"))
        }
        _ => rom_dir.join(format!("{rom_stem}_dmg_blob.png")),
    };

    if p.exists() {
        return Some(p);
    }

    // Try: test-name-dmg-cgb.png (works for both)
    let combined = rom_dir.join(format!("{rom_stem}-dmg-cgb.png"));
    if combined.exists() {
        return Some(combined);
    }

    None
}

/// List all available test ROMs in a directory
#[inline]
pub fn list_test_roms(dir: &str) -> Result<Vec<PathBuf>> {
    fn collect_roms(dir: &Path, roms: &mut Vec<PathBuf>) -> Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() {
                if let Some(ext) = path.extension()
                    && (ext == "gb" || ext == "gbc")
                {
                    roms.push(path);
                }
            } else if path.is_dir() {
                collect_roms(&path, roms)?;
            } else {
                // Ignore other types (symlinks, etc.)
            }
        }
        Ok(())
    }

    let test_dir = test_roms_dir().join(dir);

    if !test_dir.exists() {
        return Ok(Vec::new());
    }

    let mut roms = Vec::new();

    collect_roms(&test_dir, &mut roms)?;
    roms.sort();

    Ok(roms)
}

/// Runs `relative_path` (under the test ROMs directory) on `model` from the
/// real boot ROM.
///
/// It runs for at most `timeout_frames`, with `check` deciding when it is done
/// and `buttons` pressed at the given frames (each is held for 6 frames).
#[must_use]
#[inline]
pub fn run_rom(
    relative_path: &str,
    model: Model,
    timeout_frames: u32,
    check: Box<dyn CompletionCheck>,
    buttons: &[(u32, ceres_core::Button)],
) -> TestResult {
    let rom = match load_test_rom(relative_path) {
        Ok(rom) => rom,
        Err(e) => return TestResult::Error(format!("Failed to load test ROM: {e}")),
    };

    let button_events = buttons
        .iter()
        .flat_map(|&(frame, button)| {
            [
                ButtonEvent {
                    frame,
                    button,
                    action: ButtonAction::Press,
                },
                ButtonEvent {
                    frame: frame + 6,
                    button,
                    action: ButtonAction::Release,
                },
            ]
        })
        .collect();
    let config = TestConfig {
        model,
        timeout_frames,
        button_events,
        test_name: relative_path.to_string(),
        run_bootrom: true,
    };

    match TestRunner::new(rom, config, check) {
        Ok(mut runner) => runner.run(),
        Err(e) => TestResult::Error(format!("Failed to create test runner: {e}")),
    }
}

/// A ROM that reports in the registers (Mooneye's protocol).
#[must_use]
#[inline]
pub fn run_register_test(relative_path: &str, model: Model, timeout_frames: u32) -> TestResult {
    run_rom(
        relative_path,
        model,
        timeout_frames,
        Box::new(FibonacciCheck),
        &[],
    )
}

/// One of blargg's ROMs (the result is on the serial port or in the cartridge RAM).
#[must_use]
#[inline]
pub fn run_blargg_test(relative_path: &str, model: Model, timeout_frames: u32) -> TestResult {
    run_rom(
        relative_path,
        model,
        timeout_frames,
        Box::new(BlarggCheck),
        &[],
    )
}

/// A ROM whose final screen must look like `screenshot` (also under the test
/// ROMs directory), whatever the palette.
#[must_use]
#[inline]
pub fn run_screenshot_test(
    relative_path: &str,
    screenshot: &str,
    model: Model,
    timeout_frames: u32,
) -> TestResult {
    match RankedScreenshotCheck::new(&test_roms_dir().join(screenshot)) {
        Ok(check) => run_rom(relative_path, model, timeout_frames, Box::new(check), &[]),
        Err(e) => TestResult::Error(format!("Failed to load the screenshot: {e}")),
    }
}
