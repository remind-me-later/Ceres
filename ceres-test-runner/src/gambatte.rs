//! Support for the Gambatte hardware test ROMs.
//!
//! Source: <https://github.com/pokemon-speedrunning/gambatte-core> (`test/hwtests`).
//! Most ROMs show a hexadecimal result on screen whose expected value, for a
//! DMG-CPU-08 and/or a CPU-CGB-C, is encoded in the file name
//! (`..._dmg08_out<hex>`, `..._cgb04c_out<hex>`, `..._dmg08_cgb04c_out<hex>`).
//! The screen has to match Gambatte's monochrome digit glyphs.

use {
    crate::{
        DummyAudioCallback, Run,
        checks::{CompletionCheck, TestResult},
        collect_roms, test_roms_dir,
    },
    ceres_core::{Gb, Model, PX_WIDTH},
    std::{
        cell::Cell,
        path::{Path, PathBuf},
    },
};

/// Frames a ROM runs at most from the post-boot state, as in Gambatte's test
/// runner.
const FRAMES: u32 = 16;

/// Frames in a row the expected digits have to be on screen for a ROM to be
/// done before [`FRAMES`]: it has printed its result and is idle.
const STABLE_FRAMES: u32 = 2;

/// Gambatte's 8x8 hex digit glyphs (bit 7 = leftmost pixel, set = black).
const GLYPHS: [[u8; 8]; 16] = [
    [0x00, 0x7F, 0x41, 0x41, 0x41, 0x41, 0x41, 0x7F],
    [0x00, 0x08, 0x08, 0x08, 0x08, 0x08, 0x08, 0x08],
    [0x00, 0x7F, 0x01, 0x01, 0x7F, 0x40, 0x40, 0x7F],
    [0x00, 0x7F, 0x01, 0x01, 0x3F, 0x01, 0x01, 0x7F],
    [0x00, 0x41, 0x41, 0x41, 0x7F, 0x01, 0x01, 0x01],
    [0x00, 0x7F, 0x40, 0x40, 0x7E, 0x01, 0x01, 0x7E],
    [0x00, 0x7F, 0x40, 0x40, 0x7F, 0x41, 0x41, 0x7F],
    [0x00, 0x7F, 0x01, 0x02, 0x04, 0x08, 0x10, 0x10],
    [0x00, 0x3E, 0x41, 0x41, 0x3E, 0x41, 0x41, 0x3E],
    [0x00, 0x7F, 0x41, 0x41, 0x7F, 0x01, 0x01, 0x7F],
    [0x00, 0x08, 0x22, 0x41, 0x7F, 0x41, 0x41, 0x41],
    [0x00, 0x7E, 0x41, 0x41, 0x7E, 0x41, 0x41, 0x7E],
    [0x00, 0x3E, 0x41, 0x40, 0x40, 0x40, 0x41, 0x3E],
    [0x00, 0x7E, 0x41, 0x41, 0x41, 0x41, 0x41, 0x7E],
    [0x00, 0x7F, 0x40, 0x40, 0x7F, 0x40, 0x40, 0x7F],
    [0x00, 0x7F, 0x40, 0x40, 0x7F, 0x40, 0x40, 0x40],
];

/// Directory with the Gambatte test ROMs.
#[must_use]
pub fn roms_dir() -> PathBuf {
    test_roms_dir().join("gambatte")
}

/// The expected result (upper-case hex digits) for the ROM called `stem`
/// (its file name without extension) on the given hardware, if it has one.
#[must_use]
pub fn expected(stem: &str, cgb: bool) -> Option<String> {
    let (dmg_key, cgb_key) = if stem.contains("dmg08_cgb04c_out") {
        (Some("dmg08_cgb04c_out"), Some("dmg08_cgb04c_out"))
    } else if stem.contains("dmg08_out") {
        (
            Some("dmg08_out"),
            stem.contains("cgb04c_out").then_some("cgb04c_out"),
        )
    } else if stem.contains("_out") {
        (None, Some("_out"))
    } else {
        return None;
    };
    let key = if cgb { cgb_key } else { dmg_key }?;
    let pos = stem.find(key)?;
    let out = &stem[pos + key.len()..];
    // Audio tests compare sound output, not the screen.
    (!out.starts_with("audio")).then(|| {
        out.chars()
            .take_while(char::is_ascii_hexdigit)
            .map(|c| c.to_ascii_uppercase())
            .collect()
    })
}

/// Whether the 8x8 cell number `cell` of the top row shows `glyph`.
fn glyph_matches(rgba: &[u8], cell: usize, glyph: [u8; 8]) -> bool {
    glyph.iter().enumerate().all(|(y, row)| {
        (0..8).all(|x| {
            let p = (y * usize::from(PX_WIDTH) + cell * 8 + x) * 4;
            let px = &rgba[p..p + 3];
            if row & (0x80 >> x) != 0 {
                px.iter().all(|c| c & 0xF8 == 0)
            } else {
                px.iter().all(|c| c & 0xF8 == 0xF8)
            }
        })
    })
}

/// The first `digits` hex digits shown on the screen (`?` for a cell that
/// does not show a glyph).
#[must_use]
pub fn screen_text(rgba: &[u8], digits: usize) -> String {
    (0..digits)
        .map(|cell| {
            GLYPHS
                .iter()
                .position(|&glyph| glyph_matches(rgba, cell, glyph))
                .map_or('?', |digit| char::from(b"0123456789ABCDEF"[digit]))
        })
        .collect()
}

/// A ROM with an expected result on one kind of hardware.
pub struct Job {
    pub path: PathBuf,
    /// The path under [`roms_dir`], with `/` as the separator.
    pub name: String,
    /// The digits the ROM has to show.
    pub expected: String,
}

/// The ROMs with an expected result on a CPU-CGB-C (`cgb`) or a DMG-CPU-08,
/// sorted by path.
#[must_use]
pub fn jobs(cgb: bool) -> Vec<Job> {
    let root = roms_dir();
    collect_roms(&root)
        .into_iter()
        .filter_map(|path| {
            let stem = path.file_stem()?.to_str()?;
            let expected = expected(stem, cgb)?;
            let name = path
                .strip_prefix(&root)
                .ok()?
                .to_string_lossy()
                .replace('\\', "/");
            Some(Job {
                path,
                name,
                expected,
            })
        })
        .collect()
}

/// What a ROM showed, and for how many frames it ran.
pub struct Shown {
    /// The digits on the screen when the ROM was done.
    pub text: String,
    /// Frames the ROM ran: up to the one where its expected digits had been
    /// on screen for [`STABLE_FRAMES`], or all of them.
    pub frames: u32,
}

/// Done once the expected digits have stayed on screen for [`STABLE_FRAMES`]
/// frames. If they never do, the ROM gets all its frames and passes when its
/// last screen shows them, as when it always runs them all.
struct Digits<'a> {
    expected: &'a str,
    /// Frames in a row the digits have been on screen.
    run: u32,
    frames: &'a Cell<u32>,
}

impl Digits<'_> {
    fn shown(&self, gb: &Gb<DummyAudioCallback>) -> String {
        screen_text(gb.pixel_data_rgba(), self.expected.len())
    }
}

impl CompletionCheck for Digits<'_> {
    fn check(&mut self, gb: &mut Gb<DummyAudioCallback>, frames: u32) -> Option<TestResult> {
        self.frames.set(frames);
        self.run = if self.shown(gb) == self.expected {
            self.run + 1
        } else {
            0
        };
        (self.run == STABLE_FRAMES).then_some(TestResult::Passed)
    }

    fn on_timeout(&mut self, gb: &mut Gb<DummyAudioCallback>) -> TestResult {
        let text = self.shown(gb);
        if text == self.expected {
            TestResult::Passed
        } else {
            TestResult::Failed(text)
        }
    }
}

/// Runs the ROM at `path` (on a CPU-CGB-C or a DMG-CPU-08) until it shows
/// `expected`, or for all its frames (`full`), and returns what it showed, or
/// `None` if the ROM cannot be loaded.
///
/// `full` runs every frame whatever the screen shows, which is how Gambatte's
/// own runner works.
#[must_use]
pub fn run_rom(path: &Path, cgb: bool, expected: &str, full: bool) -> Option<Shown> {
    let run = Run::new(path, if cgb { Model::CgbC } else { Model::DmgB })
        .skip_boot_rom()
        .timeout(FRAMES);
    if full {
        let gb = run.machine().ok()?;
        return Some(Shown {
            text: screen_text(gb.pixel_data_rgba(), expected.len()),
            frames: FRAMES,
        });
    }
    let frames = Cell::new(0);
    let digits = Digits {
        expected,
        run: 0,
        frames: &frames,
    };
    match run.check(digits) {
        TestResult::Passed => Some(Shown {
            text: expected.to_string(),
            frames: frames.get(),
        }),
        TestResult::Failed(text) => Some(Shown {
            text,
            frames: FRAMES,
        }),
        TestResult::Error(_) => None,
    }
}
