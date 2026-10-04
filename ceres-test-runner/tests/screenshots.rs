//! Integration tests that compare the final screen of a ROM with a
//! reference screenshot: the blargg, scribbltests, turtle-tests, bully,
//! little-things-gb, mbc3-tester and strikethrough ROMs and AGE's `m3-*`.
//!
//! The screenshot's name says which hardware it was taken on (`-dmg`, `-cgb`,
//! `-dmg-cgb`, AGE's `-dmgC`, `-cgbBCE`, `-ncmBC`, ...). Colours are compared
//! by rank (brightest first), so the palette does not matter. The ROMs run
//! from the real boot ROMs. The test is slow (minutes): run it with
//! `cargo test --test screenshots -- --ignored`.
//!
//! The failures that are known are listed in `screenshots_known_failures.txt`
//! (`<model> <rom>`): SameBoy, the reference this emulator follows, shows the
//! same screen on each of them. The test fails on any other failure, and on
//! a known failure that now passes. Run with `BLESS=1` to rewrite the list.

use ceres_core::{AudioCallback, Button, ColorCorrectionMode, GbBuilder, Model, Sample};
use ceres_test_runner::test_roms_dir;
use std::path::{Path, PathBuf};

struct NoAudio;

impl AudioCallback for NoAudio {
    fn audio_sample(&self, _l: Sample, _r: Sample) {}
}

/// Ranks of the colours of an RGBA image, brightest first.
fn rank_image(rgba: &[u8]) -> Vec<u8> {
    let mut colors: Vec<[u8; 3]> = rgba.chunks(4).map(|p| [p[0], p[1], p[2]]).collect();
    colors.sort_by_key(|c| core::cmp::Reverse(u32::from(c[0]) + u32::from(c[1]) + u32::from(c[2])));
    colors.dedup();
    rgba.chunks(4)
        .map(|p| {
            colors
                .iter()
                .position(|c| *c == [p[0], p[1], p[2]])
                .map_or(255, |i| u8::try_from(i).unwrap_or(255))
        })
        .collect()
}

/// Models a screenshot suffix stands for.
fn models(suffix: &str) -> Option<&'static [(&'static str, Model)]> {
    const DMG: &[(&str, Model)] = &[("dmg", Model::DmgB)];
    const CGB: &[(&str, Model)] = &[("cgbc", Model::CgbC), ("cgbe", Model::CgbE)];
    const BC: &[(&str, Model)] = &[("cgbc", Model::CgbC)];
    const E: &[(&str, Model)] = &[("cgbe", Model::CgbE)];
    const BOTH: &[(&str, Model)] = &[
        ("dmg", Model::DmgB),
        ("cgbc", Model::CgbC),
        ("cgbe", Model::CgbE),
    ];
    const NONE: &[(&str, Model)] = &[("dmg", Model::DmgB), ("cgbe", Model::CgbE)];
    Some(match suffix {
        "-dmg" | "-dmgC" => DMG,
        "-cgb" | "-cgbBCE" | "-ncmBCE" => CGB,
        "-ncmBC" => BC,
        "-ncmE" => E,
        "-dmg-cgb" | "-cgb-dmg" => BOTH,
        "" => NONE,
        _ => return None,
    })
}

fn frames(rom: &Path) -> u32 {
    let name = rom.file_name().and_then(|n| n.to_str()).unwrap_or_default();
    match name {
        "cpu_instrs.gb" | "dmg_sound.gb" | "cgb_sound.gb" => 5400,
        "mbc3-tester.gb" => 2300,
        _ if rom.to_string_lossy().contains("/blargg/") => 3300,
        _ => 700,
    }
}

fn run(rom: &Path, model: Model, reference: &Path) -> bool {
    let data = std::fs::read(rom).expect("read ROM");
    let Ok(builder) = GbBuilder::new(48000, NoAudio)
        .with_model(model)
        .with_run_bootrom(true)
        .with_rom(data.into_boxed_slice())
    else {
        return false;
    };
    let mut gb = builder.build();
    gb.set_color_correction_mode(ColorCorrectionMode::Disabled);
    for _ in 0..frames(rom) {
        gb.run_frame();
    }
    let expected = image::open(reference).expect("reference").to_rgba8();
    expected.width() == 160
        && expected.height() == 144
        && rank_image(expected.as_raw()) == rank_image(gb.pixel_data_rgba())
}

/// `(reference, ROM, suffix)` of every screenshot of `dir`.
fn collect(dir: &Path, out: &mut Vec<(PathBuf, PathBuf, String)>) {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .expect("directory")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            collect(&path, out);
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("png") {
            continue;
        }
        let base = path.file_stem().and_then(|s| s.to_str()).expect("name");
        let (stem, suffix) = [
            "-dmg-cgb", "-cgb-dmg", "-dmgC", "-cgbBCE", "-ncmBCE", "-ncmBC", "-ncmE", "-dmg",
            "-cgb",
        ]
        .iter()
        .find_map(|s| base.strip_suffix(s).map(|stem| (stem, (*s).to_string())))
        .unwrap_or((base, String::new()));
        let rom = ["gb", "gbc"]
            .iter()
            .map(|e| path.with_file_name(format!("{stem}.{e}")))
            .find(|p| p.exists());
        if let Some(rom) = rom {
            out.push((path, rom, suffix));
        }
    }
}

#[test]
#[ignore = "slow (minutes); run with --ignored"]
fn screenshots() {
    let root = test_roms_dir();
    let mut jobs = Vec::new();
    for dir in [
        "blargg",
        "bully",
        "little-things-gb",
        "mbc3-tester",
        "scribbltests",
        "strikethrough",
        "turtle-tests",
        "age-test-roms/m3-bg-bgp",
        "age-test-roms/m3-bg-lcdc",
        "age-test-roms/m3-bg-scx",
    ] {
        collect(&root.join(dir), &mut jobs);
    }

    let mut failures = Vec::new();
    let mut ran = 0;
    for (reference, rom, suffix) in &jobs {
        let Some(models) = models(suffix) else {
            continue;
        };
        for (name, model) in models {
            ran += 1;
            if !run(rom, *model, reference) {
                let rel = rom.strip_prefix(&root).expect("under root");
                failures.push(format!(
                    "{name} {}",
                    rel.to_string_lossy().replace('\\', "/")
                ));
            }
        }
    }
    failures.sort();
    failures.dedup();

    if std::env::var("BLESS").is_ok() {
        let mut text = failures.join("\n");
        text.push('\n');
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("screenshots_known_failures.txt");
        std::fs::write(path, text).expect("write known failures");
        return;
    }

    let known: Vec<&str> = include_str!("screenshots_known_failures.txt")
        .lines()
        .filter(|l| !l.is_empty())
        .collect();
    let new_failures: Vec<&String> = failures
        .iter()
        .filter(|f| !known.contains(&f.as_str()))
        .collect();
    let fixed: Vec<&&str> = known
        .iter()
        .filter(|k| !failures.iter().any(|f| f == **k))
        .collect();
    eprintln!("{ran} runs, {} known failures", failures.len());
    assert!(ran > 50, "only {ran} screenshot runs");
    assert!(new_failures.is_empty(), "new failures: {new_failures:#?}");
    assert!(
        fixed.is_empty(),
        "these known failures now pass (run with BLESS=1): {fixed:#?}"
    );
}

/// `rtc3test` has a menu: the buttons to press, with the frame to press each at.
const RTC3TEST: &[(&str, &[(u32, Button)])] = &[
    ("basic-tests", &[(300, Button::A)]),
    ("range-tests", &[(300, Button::Down), (330, Button::A)]),
    (
        "sub-second-writes",
        &[(300, Button::Down), (320, Button::Down), (350, Button::A)],
    ),
];

#[test]
#[ignore = "slow (minutes); run with --ignored"]
fn rtc3test() {
    let dir = test_roms_dir().join("rtc3test");
    let rom = std::fs::read(dir.join("rtc3test.gb")).expect("read ROM");
    let mut failures = Vec::new();
    // The DMG screenshots are from a unit whose clock differs by a fraction of
    // a millisecond; SameBoy fails them as well.
    for (name, model) in [("cgbc", Model::CgbC), ("cgbe", Model::CgbE)] {
        for (test, keys) in RTC3TEST {
            let mut gb = GbBuilder::new(48000, NoAudio)
                .with_model(model)
                .with_run_bootrom(true)
                .with_rom(rom.clone().into_boxed_slice())
                .expect("valid ROM")
                .build();
            gb.set_color_correction_mode(ColorCorrectionMode::Disabled);
            for frame in 0..4200 {
                for (at, button) in *keys {
                    if frame == *at {
                        gb.press(*button);
                    }
                    if frame == at + 6 {
                        gb.release(*button);
                    }
                }
                gb.run_frame();
            }
            let reference = image::open(dir.join(format!("rtc3test-{test}-cgb.png")))
                .expect("reference")
                .to_rgba8();
            if rank_image(reference.as_raw()) != rank_image(gb.pixel_data_rgba()) {
                failures.push(format!("{name} {test}"));
            }
        }
    }
    assert!(failures.is_empty(), "rtc3test failures: {failures:?}");
}
