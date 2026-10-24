//! Runs the Gambatte hardware test ROMs on ceres and prints one line per
//! check, `model path PASS|FAIL expected actual`, sorted by model and path.
//!
//! Faster to iterate with than the `gambatte` test: it can be limited to the
//! ROMs whose path contains a substring, and the output can be compared with
//! the one of the reference emulator (see `tools/gambatte-oracle`).
//!
//! ```text
//! cargo run --release -p ceres-test-runner --example gambatte_runner -- [dmg|cgb|both] [filter]
//! ```
//!
//! It runs on at most 4 threads (`GAMBATTE_JOBS` changes that). Like the
//! test, it stops a ROM once its result has been on screen for two frames;
//! `GAMBATTE_FULL=1` runs all 16 frames of every ROM instead, as Gambatte's
//! runner does. `GAMBATTE_FRAMES=1` adds the number of frames each ROM ran as
//! a last column, which is how to check that the two modes agree and how many
//! frames the ROMs need.

use {
    ceres_test_runner::gambatte::{jobs, run_rom},
    std::{
        sync::{
            Mutex,
            atomic::{AtomicUsize, Ordering},
        },
        thread,
    },
};

fn main() {
    let mut args = std::env::args().skip(1);
    let which = args.next().unwrap_or_else(|| "both".into());
    let filter = args.next().unwrap_or_default();
    let models: &[bool] = match which.as_str() {
        "dmg" => &[false],
        "cgb" => &[true],
        "both" => &[false, true],
        other => {
            eprintln!("unknown model {other:?}: expected dmg, cgb or both");
            std::process::exit(2);
        }
    };

    // One job per (ROM, model) with an expected result.
    let jobs: Vec<_> = models
        .iter()
        .flat_map(|&cgb| jobs(cgb).into_iter().map(move |job| (cgb, job)))
        .filter(|(_, job)| job.name.contains(&filter))
        .collect();
    let full = std::env::var_os("GAMBATTE_FULL").is_some();
    let show_frames = std::env::var_os("GAMBATTE_FRAMES").is_some();

    let threads = std::env::var("GAMBATTE_JOBS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or_else(|| {
            thread::available_parallelism()
                .map_or(1, usize::from)
                .min(4)
        });
    let next = AtomicUsize::new(0);
    let lines = Mutex::new(Vec::new());
    thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some((cgb, job)) = jobs.get(i) else {
                        break;
                    };
                    let shown = run_rom(&job.path, *cgb, &job.expected, full);
                    let got = shown.as_ref().map_or("LOADFAIL", |s| s.text.as_str());
                    let verdict = if got == job.expected { "PASS" } else { "FAIL" };
                    let model = if *cgb { "cgb" } else { "dmg" };
                    let frames = if show_frames {
                        format!(" {}", shown.as_ref().map_or(0, |s| s.frames))
                    } else {
                        String::new()
                    };
                    let line = format!(
                        "{model} {} {verdict} {} {got}{frames}",
                        job.name, job.expected
                    );
                    lines
                        .lock()
                        .expect("no panic while the lock is held")
                        .push(((model, job.name.clone()), line));
                }
            });
        }
    });

    let mut lines = lines.into_inner().expect("no panic while the lock is held");
    lines.sort();
    for (_, line) in lines {
        println!("{line}");
    }
}
