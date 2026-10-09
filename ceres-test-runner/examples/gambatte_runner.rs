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
//! It runs on at most 4 threads (`GAMBATTE_JOBS` changes that).

use {
    ceres_test_runner::gambatte::{collect_roms, expected, roms_dir, run_rom},
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

    let root = roms_dir();
    let mut roms = Vec::new();
    collect_roms(&root, &mut roms);

    // One job per (ROM, model) with an expected result.
    let mut jobs = Vec::new();
    for path in &roms {
        let rel = path
            .strip_prefix(&root)
            .expect("under root")
            .to_string_lossy()
            .replace('\\', "/");
        if !rel.contains(&filter) {
            continue;
        }
        let stem = path.file_stem().and_then(|s| s.to_str()).expect("name");
        for &cgb in models {
            if let Some(expected) = expected(stem, cgb) {
                jobs.push((cgb, rel.clone(), expected));
            }
        }
    }

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
                    let Some((cgb, rel, expected)) = jobs.get(i) else {
                        break;
                    };
                    let got = run_rom(&root.join(rel), *cgb, expected.len())
                        .unwrap_or_else(|| "LOADFAIL".into());
                    let verdict = if got == *expected { "PASS" } else { "FAIL" };
                    let model = if *cgb { "cgb" } else { "dmg" };
                    lines
                        .lock()
                        .expect("no panic while the lock is held")
                        .push((
                            (model, rel.clone()),
                            format!("{model} {rel} {verdict} {expected} {got}"),
                        ));
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
