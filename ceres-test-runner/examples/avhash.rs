//! The A/V hash: runs a set of test ROMs for a number of frames on the DMG
//! and the CGB and prints, for each run, a hash of every sample the APU
//! gave, of every frame the PPU drew, and of the save state at the end.
//!
//! ```text
//! 3f0c1a2b44d5e6f7 9a8b7c6d5e4f3021 0123456789abcdef dmg blargg/cpu_instrs/cpu_instrs.gb
//! ```
//!
//! It compares nothing with the hardware: it guards refactors. A change that
//! should not change behaviour leaves every line as it was, and
//! `ceres-test-runner/avhash.sh` builds this on another revision and on the
//! working tree and diffs their outputs. Which of the three hashes moved
//! says where: the sound, the picture, or only state the run never showed.
//!
//! The ROMs are those of `external/test-roms`: of each suite (its top
//! directory), at most `PER_SUITE`, every n-th in path order. The boot ROM runs first, so it is covered too.
//!
//! Usage: `cargo run -p ceres-test-runner --release --example avhash --
//! [--frames N] [--jobs N] [--roms DIR]` (600 frames and every core by
//! default).

use std::{
    cell::Cell,
    path::{Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

use ceres_core::{AudioCallback, ColorCorrectionMode, GbBuilder, Model, Sample};

const USAGE: &str = "usage: avhash [--frames N] [--jobs N] [--roms DIR]";

/// At most this many ROMs of each suite run, spread evenly over it.
const PER_SUITE: usize = 20;

const MODELS: [(Model, &str); 2] = [(Model::DmgB, "dmg"), (Model::CgbE, "cgb")];

/// FNV-1a, 64 bits: the same on every toolchain, unlike std's hasher.
#[derive(Clone, Copy)]
struct Fnv(u64);

impl Fnv {
    const fn new() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }

    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 ^= u64::from(b);
            self.0 = self.0.wrapping_mul(0x0100_0000_01b3);
        }
    }
}

/// Hashes the samples as they come.
struct AudioHash(Cell<Fnv>);

impl AudioCallback for AudioHash {
    fn audio_sample(&self, l: Sample, r: Sample) {
        let mut h = self.0.get();
        h.write(&l.to_le_bytes());
        h.write(&r.to_le_bytes());
        self.0.set(h);
    }
}

impl AudioCallback for &AudioHash {
    fn audio_sample(&self, l: Sample, r: Sample) {
        (*self).audio_sample(l, r);
    }
}

fn roms(dir: &Path) -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else if path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("gb") || e.eq_ignore_ascii_case("gbc"))
            {
                out.push(path);
            }
        }
    }

    let mut all = Vec::new();
    walk(dir, &mut all);
    all.sort();

    let suite = |path: &Path| {
        path.strip_prefix(dir)
            .ok()
            .and_then(|p| p.components().next())
            .map(|c| c.as_os_str().to_owned())
    };
    let mut kept = Vec::new();
    for group in all.chunk_by(|a, b| suite(a) == suite(b)) {
        let stride = group.len().div_ceil(PER_SUITE);
        kept.extend(group.iter().step_by(stride).cloned());
    }
    kept
}

/// One run's three hashes, or why it could not run.
fn run(rom: &Path, model: Model, frames: u32) -> String {
    let data = match std::fs::read(rom) {
        Ok(data) => data,
        Err(e) => return format!("unreadable: {e}"),
    };
    let audio = AudioHash(Cell::new(Fnv::new()));
    let builder = match GbBuilder::new(48000, &audio)
        .with_model(model)
        .with_rom(data.into_boxed_slice())
    {
        Ok(builder) => builder,
        Err(e) => return format!("rejected: {e:?}"),
    };
    let mut gb = builder.build();
    gb.set_color_correction_mode(ColorCorrectionMode::Disabled);

    let mut video = Fnv::new();
    for _ in 0..frames {
        gb.run_frame();
        video.write(gb.pixel_data_rgba());
    }

    let mut buf = Vec::new();
    gb.save_state(&mut buf, 0);
    let mut state = Fnv::new();
    state.write(&buf);
    drop(gb);

    format!("{:016x} {:016x} {:016x}", audio.0.get().0, video.0, state.0)
}

fn main() {
    let mut frames = 600_u32;
    let mut jobs = std::thread::available_parallelism().map_or(1, usize::from);
    let mut dir = ceres_test_runner::test_roms_dir();

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let value = args.next();
        let ok = match (arg.as_str(), value) {
            ("--frames", Some(v)) => v.parse().map(|v| frames = v).is_ok(),
            ("--jobs", Some(v)) => v.parse().map(|v: usize| jobs = v.max(1)).is_ok(),
            ("--roms", Some(v)) => {
                dir = PathBuf::from(v);
                true
            }
            _ => false,
        };
        if !ok {
            eprintln!("{USAGE}");
            std::process::exit(2);
        }
    }

    let roms = roms(&dir);
    if roms.is_empty() {
        eprintln!("avhash: no ROMs under {}", dir.display());
        std::process::exit(1);
    }
    let runs: Vec<(&Path, Model, &str)> = roms
        .iter()
        .flat_map(|rom| {
            MODELS
                .iter()
                .map(move |&(m, name)| (rom.as_path(), m, name))
        })
        .collect();

    let next = AtomicUsize::new(0);
    let lines = Mutex::new(Vec::with_capacity(runs.len()));
    std::thread::scope(|s| {
        for _ in 0..jobs {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(&(rom, model, name)) = runs.get(i) else {
                        break;
                    };
                    let hashes = std::panic::catch_unwind(|| run(rom, model, frames))
                        .unwrap_or_else(|_| "panicked".to_owned());
                    let rel = rom.strip_prefix(&dir).unwrap_or(rom);
                    let line = format!("{hashes} {name} {}", rel.display());
                    lines
                        .lock()
                        .expect("no thread panics holding it")
                        .push((i, line));
                }
            });
        }
    });

    let mut lines = lines.into_inner().expect("no thread panicked holding it");
    lines.sort_unstable_by_key(|(i, _)| *i);
    for (_, line) in lines {
        println!("{line}");
    }
}
