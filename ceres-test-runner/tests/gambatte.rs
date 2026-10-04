//! Integration test using the Gambatte hardware test ROMs.
//!
//! Source: <https://github.com/pokemon-speedrunning/gambatte-core> (`test/hwtests`).
//! Most ROMs show a hexadecimal result on screen whose expected value, for a
//! DMG-CPU-08 and/or a CPU-CGB-C, is encoded in the file name
//! (`..._dmg08_out<hex>`, `..._cgb04c_out<hex>`, `..._dmg08_cgb04c_out<hex>`).
//! The screen has to match Gambatte's monochrome digit glyphs.
//!
//! This is a slow test (minutes): run it with `cargo test --test gambatte --
//! --ignored`. Failures that are known (the APU and serial ports, hardware
//! behaviour the reference emulators do not model either) are listed in
//! `gambatte_known_failures_{dmg,cgb}.txt`; the test fails on any other
//! failure, and on a known failure that now passes. Run with `BLESS=1` to
//! rewrite the lists.

use ceres_core::{AudioCallback, ColorCorrectionMode, GbBuilder, Model, Sample};
use ceres_test_runner::test_roms_dir;
use std::path::{Path, PathBuf};

/// Gambatte runs 15 frames from the post-boot state (the 16th is displayed).
const FRAMES: u32 = 16;

/// Gambatte's 8x8 hex digit glyphs (bit 7 = leftmost pixel, set = black).
pub const GLYPHS: [[u8; 8]; 16] = [
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

struct NoAudio;

impl AudioCallback for NoAudio {
    fn audio_sample(&self, _l: Sample, _r: Sample) {}
}

/// The expected result string for `stem` on the given hardware, if any.
fn expected(stem: &str, cgb: bool) -> Option<String> {
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
    (!out.starts_with("audio")).then(|| out.to_string())
}

fn run(path: &Path, cgb: bool, expected: &str) -> bool {
    let rom = std::fs::read(path).expect("read ROM");
    let Ok(builder) = GbBuilder::new(48000, NoAudio)
        .with_model(if cgb { Model::CgbC } else { Model::DmgB })
        .with_run_bootrom(false)
        .with_rom(rom.into_boxed_slice())
    else {
        return false;
    };
    let mut gb = builder.build();
    gb.set_color_correction_mode(ColorCorrectionMode::Disabled);
    for _ in 0..FRAMES {
        gb.run_frame();
    }

    let fb = gb.pixel_data_rgba();
    for (i, ch) in expected.chars().enumerate() {
        let Some(digit) = ch.to_digit(16) else {
            break;
        };
        let glyph = &GLYPHS[digit as usize];
        for y in 0..8 {
            for x in 0..8 {
                let p = (y * 160 + i * 8 + x) * 4;
                let px = &fb[p..p + 3];
                let black = px.iter().all(|c| c & 0xF8 == 0);
                let white = px.iter().all(|c| c & 0xF8 == 0xF8);
                let want_black = glyph[y] & (0x80 >> x) != 0;
                if (want_black && !black) || (!want_black && !white) {
                    return false;
                }
            }
        }
    }
    true
}

fn collect_roms(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .expect("gambatte directory")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            collect_roms(&path, out);
        } else if matches!(
            path.extension().and_then(|e| e.to_str()),
            Some("gb" | "gbc")
        ) {
            out.push(path);
        }
    }
}

fn check(cgb: bool, known: &str, known_path: &str) {
    let root = test_roms_dir().join("gambatte");
    let mut roms = Vec::new();
    collect_roms(&root, &mut roms);

    let mut failures = Vec::new();
    let mut ran = 0;
    for path in &roms {
        let stem = path.file_stem().and_then(|s| s.to_str()).expect("name");
        let Some(expected) = expected(stem, cgb) else {
            continue;
        };
        ran += 1;
        if !run(path, cgb, &expected) {
            let rel = path.strip_prefix(&root).expect("under root");
            failures.push(rel.to_string_lossy().replace('\\', "/"));
        }
    }
    failures.sort();

    if std::env::var("BLESS").is_ok() {
        let mut text = failures.join("\n");
        text.push('\n');
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join(known_path);
        std::fs::write(path, text).expect("write known failures");
        return;
    }

    let known: Vec<&str> = known.lines().filter(|l| !l.is_empty()).collect();
    let new_failures: Vec<&String> = failures
        .iter()
        .filter(|f| !known.contains(&f.as_str()))
        .collect();
    let fixed: Vec<&&str> = known
        .iter()
        .filter(|k| !failures.iter().any(|f| f == **k))
        .collect();
    eprintln!("{ran} ROMs run, {} known failures", failures.len());
    assert!(new_failures.is_empty(), "new failures: {new_failures:#?}");
    assert!(
        fixed.is_empty(),
        "these known failures now pass (run with BLESS=1): {fixed:#?}"
    );
}

#[test]
#[ignore = "slow (minutes); run with --ignored"]
fn gambatte_dmg() {
    check(
        false,
        include_str!("gambatte_known_failures_dmg.txt"),
        "gambatte_known_failures_dmg.txt",
    );
}

#[test]
#[ignore = "slow (minutes); run with --ignored"]
fn gambatte_cgb() {
    check(
        true,
        include_str!("gambatte_known_failures_cgb.txt"),
        "gambatte_known_failures_cgb.txt",
    );
}
