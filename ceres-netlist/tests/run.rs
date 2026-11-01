//! The test ROMs of `hdl/tests.txt` on `NetChip` from the power-on
//! (`ceres_netlist::run`), with dmg-sim's quick boot ROM, as
//! `hdl/run-tests.sh` runs them on dmg-sim and the readable model; each
//! result is compared with dmg-sim's (`hdl/build/runs/<ROM>/result.txt`).
//!
//! `cargo test -p ceres-netlist --release --test run -- --ignored --nocapture`
//! (with `RUN_ONLY=a,b`, only the ROMs whose paths contain a or b).

use {
    ceres_netlist::run::{Outcome, Run, Test},
    std::{
        fs,
        path::Path,
        sync::{
            Mutex,
            atomic::{AtomicUsize, Ordering},
        },
        thread,
    },
};

/// The ROMs run at once.
const THREADS: usize = 4;

/// A line of `tests.txt`: how the ROM reports, its path, its time limit.
struct Entry {
    test: Test,
    rom: String,
    secs: f64,
}

fn entries(root: &Path) -> Vec<Entry> {
    let list = fs::read_to_string(root.join("hdl/tests.txt")).unwrap_or_default();
    let only = std::env::var("RUN_ONLY").ok();
    list.lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let test = match fields.next()? {
                "mooneye" => Test::Mooneye,
                "gbmicrotest" => Test::Gbmicrotest,
                "blargg" => Test::Blargg,
                _ => return None,
            };
            let rom = fields.next()?.to_owned();
            let secs = fields.next()?.parse().ok()?;
            Some(Entry { test, rom, secs })
        })
        .filter(|e| {
            only.as_ref()
                .is_none_or(|only| only.split(',').any(|part| e.rom.contains(part)))
        })
        .collect()
}

/// pass, fail or timeout, as the result files begin.
const fn word(outcome: &Outcome) -> &'static str {
    match outcome {
        Outcome::Pass(_) => "pass",
        Outcome::Fail(_) => "fail",
        Outcome::Timeout => "timeout",
    }
}

#[test]
#[ignore = "needs the test ROMs, dmg-sim's boot ROM and its results: hdl/run-tests.sh"]
#[expect(
    clippy::expect_used,
    clippy::print_stderr,
    reason = "a test, which reports what it finds"
)]
fn netchip_runs_the_tests() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let boot_rom =
        fs::read(root.join("external/reference-implementations/dmg-sim/boot/quickboot.bin"))
            .expect("dmg-sim's quick boot ROM");
    let entries = entries(&root);
    assert!(!entries.is_empty(), "no tests in hdl/tests.txt");

    let results = Mutex::new(Vec::new());
    let next = AtomicUsize::new(0);
    thread::scope(|scope| {
        for _ in 0..THREADS {
            scope.spawn(|| {
                loop {
                    let n = next.fetch_add(1, Ordering::Relaxed);
                    let Some(entry) = entries.get(n) else {
                        break;
                    };
                    let outcome = fs::read(root.join(&entry.rom)).map_or_else(
                        |e| Outcome::Fail(format!("{}: {e}", entry.rom)),
                        |rom| Run::new(rom, boot_rom.clone()).until(entry.test, entry.secs),
                    );
                    let dir = entry.rom.trim_end_matches(".gb");
                    let theirs =
                        fs::read_to_string(root.join(format!("hdl/build/runs/{dir}/result.txt")))
                            .ok()
                            .and_then(|r| r.split_whitespace().next().map(str::to_owned));
                    results
                        .lock()
                        .expect("the results")
                        .push((n, outcome, theirs));
                }
            });
        }
    });

    let mut results = results.into_inner().expect("the results");
    results.sort_by_key(|(n, _, _)| *n);
    let mut differ = 0;
    for (n, outcome, theirs) in &results {
        let ours = word(outcome);
        let mark = match theirs.as_deref() {
            Some(theirs) if theirs == ours => "  ",
            None => "? ",
            Some(_) => {
                differ += 1;
                "! "
            }
        };
        let detail = match outcome {
            Outcome::Pass(detail) | Outcome::Fail(detail) => detail.as_str(),
            Outcome::Timeout => "",
        };
        eprintln!(
            "{mark}{ours:7} (dmg-sim {:7}) {} {detail}",
            theirs.as_deref().unwrap_or("-"),
            entries[*n].rom
        );
    }
    let passed = results
        .iter()
        .filter(|(_, o, _)| matches!(o, Outcome::Pass(_)))
        .count();
    eprintln!(
        "{passed} of {} pass; {differ} differ from dmg-sim",
        results.len()
    );
    assert_eq!(differ, 0, "NetChip's results differ from dmg-sim's");
}
