//! Runs the Game Boy test ROMs (`external/test-roms`) on Ceres.
//!
//! A test describes the run with [`Run`] (the ROM, the model, the boot ROM,
//! the time it gets and the buttons pressed) and a
//! [`CompletionCheck`](checks::CompletionCheck) decides when the ROM is done
//! and whether it passed. [`rom_test!`] declares a test from that. The suites
//! that sweep a directory compare their failures with a list in `tests/`
//! (see [`check_known_failures`]).

pub mod checks;
pub mod gambatte;

use anyhow::{Context as _, Result};
use ceres_core::{AudioCallback, Button, Gb, GbBuilder, Model, Sample};
use checks::{CompletionCheck, ExactScreenshotCheck, RankedScreenshotCheck, TestResult};
use std::path::{Path, PathBuf};

/// How long each suite may run, in frames (about 60 a second).
pub mod timeouts {
    pub const CGB_ACID2: u32 = 300;
    pub const DMG_ACID2: u32 = 480;
    /// Mooneye's acceptance tests: 2 minutes at most.
    pub const MOONEYE: u32 = 7160;
    pub const MEALYBUG: u32 = 500;
    pub const BLARGG: u32 = 3000;
    /// blargg's combined ROMs, which run all the single ones.
    pub const BLARGG_COMBINED: u32 = 6000;
    pub const AGE: u32 = 800;
    /// The ROMs that show a final screen: AGE's `m3-*`, bully,
    /// little-things-gb and the scribbltests (the boot ROM, then a few
    /// seconds, statcount-auto's 270 frames being the longest).
    pub const SCREENSHOT: u32 = 900;
    /// mbc3-tester checks every bank before showing its result.
    pub const MBC3_TESTER: u32 = 2300;
    /// The slowest SameSuite ROM needs about 100.
    pub const SAME_SUITE: u32 = 600;
    /// `is_if_set_during_ime0`, the slowest GBMicrotest ROM, needs about 23.
    pub const GBMICROTEST: u32 = 60;
    /// Without a `Run::timeout`.
    pub const DEFAULT: u32 = 1792;
}

/// An audio callback that drops the samples.
#[derive(Default)]
pub struct DummyAudioCallback;

impl AudioCallback for DummyAudioCallback {
    fn audio_sample(&self, _l: Sample, _r: Sample) {}
}

/// The directory with the test ROMs.
#[must_use]
pub fn test_roms_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("external")
        .join("test-roms")
}

/// Reads a ROM: `path` is under the test ROMs directory (or absolute).
///
/// # Errors
///
/// Returns an error if the ROM is missing or cannot be read.
pub fn load_test_rom(path: impl AsRef<Path>) -> Result<Vec<u8>> {
    let rom_path = test_roms_dir().join(path);
    if !rom_path.exists() {
        anyhow::bail!(
            "Test ROM not found: {}\n\n\
             The build script downloads them into external/test-roms.\n\
             Try: cargo clean --package ceres-test-runner && cargo build --package ceres-test-runner",
            rom_path.display()
        );
    }
    std::fs::read(&rom_path)
        .with_context(|| format!("Failed to read test ROM: {}", rom_path.display()))
}

/// The reference screenshot of the ROM at `relative_path` for `model`.
///
/// It tries the names the suites use: `<rom>-cgb.png`/`<rom>-dmg.png`, Mealybug's
/// `<rom>_cgb_c.png` (`_cgb_d` first on a CGB-D) and `<rom>_dmg_blob.png`,
/// then `<rom>-dmg-cgb.png` for both.
#[must_use]
pub fn expected_screenshot_path(relative_path: &str, model: Model) -> Option<PathBuf> {
    let rom_path = test_roms_dir().join(relative_path);
    let dir = rom_path.parent()?;
    let stem = rom_path.file_stem()?.to_str()?;
    let cgb = model.is_cgb_hardware() && model != Model::Agb;

    let mut candidates = vec![format!("{stem}-{}.png", if cgb { "cgb" } else { "dmg" })];
    if model == Model::CgbD {
        candidates.push(format!("{stem}_cgb_d.png"));
    }
    candidates.push(if cgb {
        format!("{stem}_cgb_c.png")
    } else {
        format!("{stem}_dmg_blob.png")
    });
    candidates.push(format!("{stem}-dmg-cgb.png"));

    candidates
        .into_iter()
        .map(|name| dir.join(name))
        .find(|path| path.exists())
}

/// Every `.gb` and `.gbc` file below `dir`, each directory in name order.
///
/// # Panics
///
/// Panics if a directory cannot be read.
#[must_use]
pub fn collect_roms(dir: &Path) -> Vec<PathBuf> {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .filter_map(Result::ok)
        .map(|e| e.path())
        .collect();
    entries.sort();
    let mut roms = Vec::new();
    for path in entries {
        if path.is_dir() {
            roms.extend(collect_roms(&path));
        } else if matches!(
            path.extension().and_then(|e| e.to_str()),
            Some("gb" | "gbc")
        ) {
            roms.push(path);
        }
    }
    roms
}

/// Frames a pressed button is held.
const BUTTON_HOLD_FRAMES: u32 = 6;

/// A run of a test ROM.
pub struct Run {
    rom: PathBuf,
    model: Model,
    timeout_frames: u32,
    boot_rom: bool,
    buttons: Vec<(u32, Button)>,
}

impl Run {
    /// Runs `rom` (under the test ROMs directory, or absolute) on `model`,
    /// from the real boot ROM.
    #[must_use]
    pub fn new(rom: impl AsRef<Path>, model: Model) -> Self {
        Self {
            rom: rom.as_ref().to_path_buf(),
            model,
            timeout_frames: timeouts::DEFAULT,
            boot_rom: true,
            buttons: Vec::new(),
        }
    }

    /// Frames the ROM gets at most.
    #[must_use]
    pub const fn timeout(mut self, frames: u32) -> Self {
        self.timeout_frames = frames;
        self
    }

    /// Starts from the state the boot ROM leaves instead of running it.
    #[must_use]
    pub const fn skip_boot_rom(mut self) -> Self {
        self.boot_rom = false;
        self
    }

    /// Presses each button at its frame, for a few frames.
    #[must_use]
    pub fn press(mut self, buttons: &[(u32, Button)]) -> Self {
        self.buttons.extend_from_slice(buttons);
        self
    }

    /// Runs until `check` decides or the time is up.
    pub fn check(self, mut check: impl CompletionCheck) -> TestResult {
        let mut gb = match self.build() {
            Ok(gb) => gb,
            Err(e) => return TestResult::Error(e.to_string()),
        };
        for frame in 0..self.timeout_frames {
            self.run_frame(&mut gb, frame);
            if let Some(result) = check.check(&mut gb, frame + 1) {
                return result;
            }
        }
        check.on_timeout(&mut gb)
    }

    /// Runs every frame and returns the machine.
    ///
    /// # Errors
    ///
    /// Returns an error if the ROM cannot be read or loaded.
    pub fn machine(self) -> Result<Gb<DummyAudioCallback>> {
        let mut gb = self.build()?;
        for frame in 0..self.timeout_frames {
            self.run_frame(&mut gb, frame);
        }
        Ok(gb)
    }

    fn build(&self) -> Result<Gb<DummyAudioCallback>> {
        let rom = load_test_rom(&self.rom)?;
        let mut gb = GbBuilder::new(48000, DummyAudioCallback)
            .with_model(self.model)
            .with_run_bootrom(self.boot_rom)
            .with_rom(rom.into_boxed_slice())
            .with_context(|| format!("Failed to load {}", self.rom.display()))?
            .build();
        gb.set_color_correction_mode(ceres_core::ColorCorrectionMode::Disabled);
        Ok(gb)
    }

    fn run_frame(&self, gb: &mut Gb<DummyAudioCallback>, frame: u32) {
        for &(at, button) in &self.buttons {
            if frame == at {
                gb.press(button);
            } else if frame == at + BUTTON_HOLD_FRAMES {
                gb.release(button);
            } else {
                // Not this button's frame.
            }
        }
        gb.run_frame();
    }
}

/// A ROM whose final screen must look like `screenshot` (also under the test
/// ROMs directory), whatever the palette.
#[must_use]
pub fn run_ranked_screenshot(rom: &str, screenshot: &str, model: Model, frames: u32) -> TestResult {
    match RankedScreenshotCheck::new(&test_roms_dir().join(screenshot)) {
        Ok(check) => Run::new(rom, model).timeout(frames).check(check),
        Err(e) => TestResult::Error(format!("Failed to load the screenshot: {e}")),
    }
}

/// A ROM whose screen must be its reference screenshot (see
/// [`expected_screenshot_path`]) pixel for pixel at its `ld b, b` breakpoint.
#[must_use]
pub fn run_exact_screenshot(rom: &str, model: Model, frames: u32) -> TestResult {
    expected_screenshot_path(rom, model).map_or_else(
        || TestResult::Error(format!("No expected screenshot found for {rom}")),
        |screenshot| {
            Run::new(rom, model)
                .timeout(frames)
                .check(ExactScreenshotCheck::new(screenshot))
        },
    )
}

/// Compares the failures of a suite that sweeps a directory with the known ones.
///
/// They are listed in `tests/<list>`, one per line: it panics on a new failure
/// and on a known one that now passes. With `BLESS` set it rewrites the list
/// instead.
///
/// # Panics
///
/// Panics as described, or if the list cannot be read or written.
pub fn check_known_failures(list: &str, mut failures: Vec<String>) {
    failures.sort();
    failures.dedup();
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join(list);

    if std::env::var("BLESS").is_ok() {
        let mut text = failures.join("\n");
        text.push('\n');
        std::fs::write(&path, text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        return;
    }

    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let known: Vec<&str> = text.lines().filter(|l| !l.is_empty()).collect();
    let new_failures: Vec<&String> = failures
        .iter()
        .filter(|f| !known.contains(&f.as_str()))
        .collect();
    let fixed: Vec<&&str> = known
        .iter()
        .filter(|k| !failures.iter().any(|f| f == **k))
        .collect();
    eprintln!("{} known failures", failures.len());
    assert!(new_failures.is_empty(), "new failures: {new_failures:#?}");
    assert!(
        fixed.is_empty(),
        "these known failures now pass (run with BLESS=1): {fixed:#?}"
    );
}

/// Declares a test that passes if `$result` (a [`TestResult`]) does, with any
/// attributes (`#[ignore = "..."]`) before the name.
#[macro_export]
macro_rules! rom_test {
    ($(#[$attr:meta])* $name:ident: $result:expr) => {
        #[test]
        $(#[$attr])*
        fn $name() {
            let result = $result;
            assert!(result.is_passed(), "{result:?}");
        }
    };
}
