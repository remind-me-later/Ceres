//! How a test ROM says it is done, and whether it passed.

use crate::DummyAudioCallback;
use anyhow::Result;
use ceres_core::{Gb, PX_HEIGHT, PX_WIDTH};
use std::path::Path;

/// The outcome of a test ROM.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TestResult {
    Passed,
    /// The ROM ran and failed (or never finished), with what it showed.
    Failed(String),
    /// The test could not run: a missing ROM or screenshot, an invalid ROM...
    Error(String),
}

impl TestResult {
    #[must_use]
    pub const fn is_passed(&self) -> bool {
        matches!(self, Self::Passed)
    }
}

/// Decides when a test ROM is done, and whether it passed.
pub trait CompletionCheck {
    /// Looks at the machine after each frame (`frames` have run): `Some` once
    /// the ROM is done.
    fn check(&mut self, gb: &mut Gb<DummyAudioCallback>, frames: u32) -> Option<TestResult>;

    /// The result when the time is up.
    fn on_timeout(&mut self, _gb: &mut Gb<DummyAudioCallback>) -> TestResult {
        TestResult::Failed("Timeout reached".to_string())
    }
}

/// The screen must be the reference screenshot, pixel for pixel.
///
/// It is compared when the ROM reaches its `ld b, b` breakpoint (or when the
/// time is up). The emulator's colours must match the reference's: see
/// [`RankedScreenshotCheck`] for references taken with another palette.
pub struct ExactScreenshotCheck {
    expected: std::path::PathBuf,
}

impl ExactScreenshotCheck {
    #[must_use]
    pub const fn new(expected: std::path::PathBuf) -> Self {
        Self { expected }
    }

    fn result(&self, gb: &Gb<DummyAudioCallback>) -> TestResult {
        let expected = match load_screenshot(&self.expected) {
            Ok(image) => image,
            Err(e) => return TestResult::Error(format!("Screenshot comparison error: {e}")),
        };
        let actual = gb.pixel_data_rgba();
        if expected == actual {
            return TestResult::Passed;
        }
        report_mismatches(&expected, actual);
        TestResult::Failed("Screenshot mismatch".to_string())
    }
}

impl CompletionCheck for ExactScreenshotCheck {
    fn check(&mut self, gb: &mut Gb<DummyAudioCallback>, _frames: u32) -> Option<TestResult> {
        gb.take_ld_b_b_breakpoint().then(|| self.result(gb))
    }

    fn on_timeout(&mut self, gb: &mut Gb<DummyAudioCallback>) -> TestResult {
        self.result(gb)
    }
}

/// A screenshot as RGBA bytes.
///
/// # Errors
///
/// Returns an error if it cannot be read or is not the size of the screen.
pub fn load_screenshot(path: &Path) -> Result<Vec<u8>> {
    let image = image::open(path)?.to_rgba8();
    anyhow::ensure!(
        image.width() == u32::from(PX_WIDTH) && image.height() == u32::from(PX_HEIGHT),
        "{} is not {PX_WIDTH}x{PX_HEIGHT}",
        path.display()
    );
    Ok(image.into_raw())
}

/// Prints the first mismatching pixels and a count per scanline.
fn report_mismatches(expected: &[u8], actual: &[u8]) {
    let width = usize::from(PX_WIDTH);
    let mut count = 0;
    let mut lines = vec![(0_usize, None); usize::from(PX_HEIGHT)];
    for (i, (e, a)) in expected.chunks(4).zip(actual.chunks(4)).enumerate() {
        if e == a {
            continue;
        }
        count += 1;
        let (x, y) = (i % width, i / width);
        if count <= 10 {
            eprintln!("Mismatch #{count} at ({x}, {y}): expected {e:?}, got {a:?}");
        }
        let line = &mut lines[y];
        line.0 += 1;
        line.1.get_or_insert((x, e, a));
    }
    for (y, (n, first)) in lines.iter().enumerate() {
        if let Some((x, e, a)) = first {
            eprintln!("Scanline {y}: {n} mismatches (first at x={x}: exp={e:?} got={a:?})");
        }
    }
    eprintln!(
        "Total mismatching pixels: {count} / {}",
        usize::from(PX_WIDTH) * usize::from(PX_HEIGHT)
    );
}

/// The rank of each pixel's colour, brightest first: two images that differ
/// only in their palette have the same ranks. Colours as bright as each
/// other keep the order they first appear in.
#[must_use]
pub fn rank_image(rgba: &[u8]) -> Vec<u8> {
    let rgb = |p: &[u8]| [p[0], p[1], p[2]];
    let mut colors: Vec<[u8; 3]> = Vec::new();
    for pixel in rgba.chunks(4).map(rgb) {
        if !colors.contains(&pixel) {
            colors.push(pixel);
        }
    }
    colors.sort_by_key(|c| core::cmp::Reverse(c.iter().map(|&v| u32::from(v)).sum::<u32>()));
    rgba.chunks(4)
        .map(|p| {
            let rank = colors.iter().position(|c| *c == rgb(p)).unwrap_or(0);
            u8::try_from(rank).unwrap_or(u8::MAX)
        })
        .collect()
}

/// Frames between two looks at the screen.
const SCREEN_CHECK_INTERVAL: u32 = 30;

/// The screen must end up looking like a reference screenshot, whatever the
/// palette (see [`rank_image`]).
///
/// The screen is looked at every half second; the check passes as soon as it
/// matches twice in a row (the ROMs keep their final screen) or when the time
/// is up.
pub struct RankedScreenshotCheck {
    expected: Vec<u8>,
    matched_last_time: bool,
}

impl RankedScreenshotCheck {
    /// # Errors
    ///
    /// Returns an error if the screenshot cannot be read or is not the size
    /// of the screen.
    pub fn new(screenshot: &Path) -> Result<Self> {
        Ok(Self {
            expected: rank_image(&load_screenshot(screenshot)?),
            matched_last_time: false,
        })
    }

    fn matches(&self, gb: &Gb<DummyAudioCallback>) -> bool {
        rank_image(gb.pixel_data_rgba()) == self.expected
    }
}

impl CompletionCheck for RankedScreenshotCheck {
    fn check(&mut self, gb: &mut Gb<DummyAudioCallback>, frames: u32) -> Option<TestResult> {
        if !frames.is_multiple_of(SCREEN_CHECK_INTERVAL) {
            return None;
        }
        let matches = self.matches(gb);
        if matches && self.matched_last_time {
            return Some(TestResult::Passed);
        }
        self.matched_last_time = matches;
        None
    }

    fn on_timeout(&mut self, gb: &mut Gb<DummyAudioCallback>) -> TestResult {
        if self.matches(gb) {
            TestResult::Passed
        } else {
            TestResult::Failed("Screenshot mismatch".to_string())
        }
    }
}

/// The result is in the registers (Mooneye's protocol, which Wilbertpol's
/// fork, AGE and SameSuite follow).
///
/// On success they hold the Fibonacci numbers `3, 5, 8, 13, 21, 34` in `B, C,
/// D, E, H, L`; on failure all six hold `0x42`. Mooneye's ROMs signal the end
/// with the `ld b, b` breakpoint, Wilbertpol's with the illegal opcode `0xED`.
/// A failure reports the registers, the start of WRAM and HRAM, and the text
/// on the screen.
pub struct RegisterCheck;

impl CompletionCheck for RegisterCheck {
    fn check(&mut self, gb: &mut Gb<DummyAudioCallback>, _frames: u32) -> Option<TestResult> {
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
        if registers == [3, 5, 8, 13, 21, 34] {
            return Some(TestResult::Passed);
        }
        let hex = |range: core::ops::Range<u16>| {
            range
                .map(|address| format!("{:02X}", gb.read_mem(address)))
                .collect::<Vec<_>>()
                .join(" ")
        };
        Some(TestResult::Failed(format!(
            "registers B C D E H L = {registers:02X?}, C000=[{}], HRAM=[{}], text: {:?}",
            hex(0xC000..0xC040),
            hex(0xFF80..0xFFA0),
            screen_text(gb)
        )))
    }
}

/// The text on the screen of a Mooneye-style ROM: the first tile map that has
/// any, its non-empty rows joined with `|`.
fn screen_text(gb: &Gb<DummyAudioCallback>) -> String {
    for map in [0x9800, 0x9C00] {
        let lines: Vec<String> = (0..18)
            .map(|row| {
                (0..20)
                    .map(|col| match gb.read_mem(map + row * 32 + col) {
                        0x00 | 0x19 => ' ',
                        tile @ 0x1A..=0x7E => char::from(tile + 0x20 - 0x1A),
                        _ => '.',
                    })
                    .collect::<String>()
                    .trim()
                    .to_string()
            })
            .filter(|line| !line.is_empty())
            .collect();
        if !lines.is_empty() {
            return lines.join(" | ");
        }
    }
    String::new()
}

/// The result of one of blargg's ROMs.
///
/// The older ones print it over the serial port ("Passed" or "Failed"); the
/// newer ones leave it in the cartridge RAM instead: a signature (`DE B0 61`)
/// at `$A001`, then the text, and at `$A000` the result code (`$80` while
/// running, 0 on success).
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
    fn check(&mut self, gb: &mut Gb<DummyAudioCallback>, _frames: u32) -> Option<TestResult> {
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

    fn on_timeout(&mut self, gb: &mut Gb<DummyAudioCallback>) -> TestResult {
        TestResult::Failed(format!(
            "Timeout reached, serial output: {:?}, cartridge RAM text: {:?}",
            gb.serial_output(),
            Self::memory_text(gb)
        ))
    }
}

/// The result of one of aappleby's GBMicrotest ROMs, in HRAM: `$FF80` holds
/// the actual value, `$FF81` the expected one and `$FF82` is `$01` on success
/// or `$FF` on failure.
pub struct MicrotestCheck;

impl CompletionCheck for MicrotestCheck {
    fn check(&mut self, gb: &mut Gb<DummyAudioCallback>, _frames: u32) -> Option<TestResult> {
        match gb.read_mem(0xFF82) {
            0x01 => Some(TestResult::Passed),
            0xFF => Some(TestResult::Failed(format!(
                "got {:#04X}, expected {:#04X}",
                gb.read_mem(0xFF80),
                gb.read_mem(0xFF81)
            ))),
            _ => None,
        }
    }
}
