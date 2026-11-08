//! `NetPipeline` (`ceres_netlist::render_edge`) against `NetRender`, edge
//! by edge, on `NetChip`'s runs of the test ROMs: at each edge where the
//! window and the sprites are idle, the pipeline steps from render's state
//! before it and must reach render's after it.
//!
//! `TEST_ROOT=<a checkout with external/> cargo test -p ceres-netlist
//! --release --test render_edge -- --ignored --nocapture`, with the ROMs of
//! `hdl/tests.txt`, or `ROMS=a.gb,b.gb` (paths under `TEST_ROOT`), each run
//! for `SECS` (0.3 by default) after the power-on.

use {
    ceres_netlist::{
        render::NetRender,
        render_edge::{EdgeInputs, NetPipeline},
        run::{HALF_PERIOD_PS, Run},
    },
    std::{
        env, fs,
        path::{Path, PathBuf},
        sync::{
            Mutex,
            atomic::{AtomicUsize, Ordering},
        },
        thread,
    },
};

/// The ROMs run at once.
const THREADS: usize = 4;

/// The window and the sprites are idle: LCDC's bits 1 and 5 clear, WY not
/// matched in the frame, no X matched.
const fn idle(r: &NetRender) -> bool {
    (r.inputs.lcdc & 0x22 == 0)
        && !r.wy_latch
        && !r.sary
        && !r.in_window
        && !r.pyco
        && !r.nunu
        && !r.rydy
        && !r.sovy
        && !r.wires.x_match
        && !r.wires.x_match_held
}

/// What a ROM's run found: the edges compared, those in mode 3, and the
/// first difference.
struct Found {
    rom: String,
    compared: u64,
    in_mode3: u64,
    first: Option<String>,
}

fn roms(root: &Path) -> Vec<String> {
    if let Ok(list) = env::var("ROMS") {
        return list.split(',').map(str::to_owned).collect();
    }
    let list = fs::read_to_string(root.join("hdl/tests.txt")).unwrap_or_default();
    list.lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            matches!(fields.next()?, "mooneye" | "gbmicrotest" | "blargg")
                .then(|| fields.next().map(str::to_owned))?
        })
        .collect()
}

fn check(rom: Vec<u8>, boot: Vec<u8>, edges: u64, name: &str) -> Found {
    let mut run = Run::new(rom, boot);
    let mut found = Found {
        rom: name.to_owned(),
        compared: 0,
        in_mode3: 0,
        first: None,
    };
    while run.edge < edges {
        let before = run.chip.render;
        run.step();
        let after = &run.chip.render;
        if !(idle(&before) && idle(after)) {
            continue;
        }
        let mut pipeline = NetPipeline::of(&before);
        pipeline.edge(EdgeInputs::of(&after.inputs));
        found.compared += 1;
        found.in_mode3 += u64::from(before.mode3 | after.mode3);
        if !pipeline.same_as(after) && found.first.is_none() {
            found.first = Some(format!(
                "edge {} ly {} clk {}\n  before {:?}\n  render {:?}\n  model  {:?}",
                run.edge,
                run.chip.lcd.ly,
                after.inputs.clk,
                Fields(&NetPipeline::of(&before)),
                Fields(&NetPipeline::of(after)),
                Fields(&pipeline),
            ));
        }
    }
    found
}

/// The pipeline's fields, for a report.
struct Fields<'a>(&'a NetPipeline);

impl std::fmt::Debug for Fields<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let p = self.0;
        let i = &p.inputs;
        write!(
            f,
            "m3 {} voga {} px {} count {} lyzu {} nyka {} pory {} pygo {} started {} fine {} puxa {} nyze {} discard {} ryfa {} rene {} scx {} | ls {} stop {} wr {} {:04x}={:02x} reset {}",
            u8::from(p.mode3),
            u8::from(p.voga),
            p.px,
            p.fetch_count,
            u8::from(p.lyzu),
            u8::from(p.nyka),
            u8::from(p.pory),
            u8::from(p.pygo),
            u8::from(p.started),
            p.fine,
            u8::from(p.puxa),
            u8::from(p.nyze),
            u8::from(p.discard),
            u8::from(p.ryfa),
            u8::from(p.rene),
            p.scx,
            u8::from(i.line_start),
            u8::from(i.stop_scan),
            u8::from(i.cpu_wr_sync),
            i.addr,
            i.data,
            u8::from(i.ppu_reset),
        )
    }
}

#[test]
#[ignore = "needs the test ROMs (TEST_ROOT) and hdl/build/fastboot.bin"]
#[expect(
    clippy::expect_used,
    clippy::print_stderr,
    reason = "a test, which reports what it finds"
)]
fn pipeline_follows_render() {
    let here = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let root = env::var("TEST_ROOT").map_or_else(|_| here.clone(), PathBuf::from);
    let boot = fs::read(here.join("hdl/build/fastboot.bin")).expect("hdl/build/fastboot.bin");
    let secs: f64 = env::var("SECS").map_or(0.3, |s| s.parse().expect("SECS"));
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a positive time"
    )]
    let edges = (secs * 1e12) as u64 / HALF_PERIOD_PS;
    let roms = roms(&root);
    assert!(!roms.is_empty(), "no ROMs");

    let results = Mutex::new(Vec::new());
    let next = AtomicUsize::new(0);
    thread::scope(|scope| {
        for _ in 0..THREADS {
            scope.spawn(|| {
                loop {
                    let n = next.fetch_add(1, Ordering::Relaxed);
                    let Some(name) = roms.get(n) else {
                        break;
                    };
                    let Ok(rom) = fs::read(root.join(name)) else {
                        eprintln!("{name}: not found");
                        continue;
                    };
                    let found = check(rom, boot.clone(), edges, name);
                    results.lock().expect("the results").push((n, found));
                }
            });
        }
    });
    let mut results = results.into_inner().expect("the results");
    results.sort_by_key(|(n, _)| *n);
    let mut differ = 0;
    for (_, found) in &results {
        eprintln!(
            "{}: {} edges compared, {} in mode 3{}",
            found.rom,
            found.compared,
            found.in_mode3,
            if found.first.is_some() {
                ", DIFFERS"
            } else {
                ""
            }
        );
        if let Some(first) = &found.first {
            differ += 1;
            eprintln!("  {first}");
        }
    }
    assert!(differ == 0, "{differ} ROMs differ");
}
