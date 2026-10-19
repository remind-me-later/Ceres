//! Integration tests using the Gambatte hardware test ROMs (see
//! `ceres_test_runner::gambatte` for how they report).
//!
//! Every ROM has to pass, on the DMG and on the CGB (4674 checks). A ROM is
//! done as soon as its result has been on screen for two frames, which takes a
//! few frames, not the 16 Gambatte's runner gives each one.
//!
//! The ROMs are split into shards, one test each, so that the test runner
//! spreads them over the cores: nextest runs every shard in its own process,
//! which also keeps threads from fighting over the coverage counters.

use ceres_test_runner::gambatte::{jobs, run_rom};

/// Runs the ROMs of the shard `shard` of `shards` (those whose place in the
/// sorted list of ROMs is `shard` modulo `shards`, so that every shard has
/// about the same mix).
fn check(cgb: bool, shard: usize, shards: usize) {
    let mut failures = Vec::new();
    let mut ran = 0;
    for job in jobs(cgb).into_iter().skip(shard).step_by(shards) {
        ran += 1;
        match run_rom(&job.path, cgb, &job.expected, false) {
            Some(shown) if shown.text == job.expected => {}
            Some(shown) => failures.push(format!(
                "{}: expected {}, shown {}",
                job.name, job.expected, shown.text
            )),
            None => failures.push(format!("{}: could not be loaded", job.name)),
        }
    }

    eprintln!("{ran} ROMs run, {} failures", failures.len());
    assert!(ran > 0, "shard {shard} of {shards} has no ROMs");
    assert!(failures.is_empty(), "failures: {failures:#?}");
}

/// One test per shard, `name = index`: the number of shards is the number of
/// tests listed, so list the indices from 0 without gaps.
macro_rules! shards {
    (@emit $count:expr, $cgb:expr; $($name:ident = $index:expr),*) => {
        $(
            #[test]
            fn $name() {
                check($cgb, $index, $count);
            }
        )*
    };
    ($cgb:expr; $($name:ident = $index:expr),* $(,)?) => {
        shards!(@emit [$($index),*].len(), $cgb; $($name = $index),*);
    };
}

shards! { false;
    gambatte_dmg_0 = 0,
    gambatte_dmg_1 = 1,
    gambatte_dmg_2 = 2,
    gambatte_dmg_3 = 3,
    gambatte_dmg_4 = 4,
    gambatte_dmg_5 = 5,
}

shards! { true;
    gambatte_cgb_0 = 0,
    gambatte_cgb_1 = 1,
    gambatte_cgb_2 = 2,
    gambatte_cgb_3 = 3,
    gambatte_cgb_4 = 4,
    gambatte_cgb_5 = 5,
    gambatte_cgb_6 = 6,
    gambatte_cgb_7 = 7,
    gambatte_cgb_8 = 8,
    gambatte_cgb_9 = 9,
}

/// The shards must not hide ROMs: the collection is the one the shards split.
#[test]
fn gambatte_roms_found() {
    let dmg = jobs(false).len();
    let cgb = jobs(true).len();
    assert!(dmg >= 1600, "only {dmg} DMG ROMs with an expected result");
    assert!(cgb >= 3000, "only {cgb} CGB ROMs with an expected result");
}
