//! Support for the Gambatte hardware test ROMs.
//!
//! Source: <https://github.com/pokemon-speedrunning/gambatte-core> (`test/hwtests`).
//! Most ROMs show a hexadecimal result on screen whose expected value, for a
//! DMG-CPU-08 and/or a CPU-CGB-C, is encoded in the file name
//! (`..._dmg08_out<hex>`, `..._cgb04c_out<hex>`, `..._dmg08_cgb04c_out<hex>`).
//! The screen has to match Gambatte's monochrome digit glyphs.

use {
    crate::{Run, test_roms_dir},
    ceres_core::{Model, PX_WIDTH},
    std::path::{Path, PathBuf},
};

/// Frames a ROM runs from the post-boot state, as in Gambatte's test runner.
const FRAMES: u32 = 16;

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

/// Runs the ROM at `path` (on a CPU-CGB-C or a DMG-CPU-08) and returns the
/// first `digits` digits it shows, or `None` if the ROM cannot be loaded.
#[must_use]
pub fn run_rom(path: &Path, cgb: bool, digits: usize) -> Option<String> {
    let gb = Run::new(path, if cgb { Model::CgbC } else { Model::DmgB })
        .skip_boot_rom()
        .timeout(FRAMES)
        .machine()
        .ok()?;
    Some(screen_text(gb.pixel_data_rgba(), digits))
}
