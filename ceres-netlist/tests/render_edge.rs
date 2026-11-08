//! `NetPipeline` (`ceres_netlist::render_edge`) against `NetRender`, edge
//! by edge, on `NetChip`'s runs of the test ROMs: at each edge where the
//! window and the sprites are idle, the pipeline steps from render's state
//! before it and must reach render's after it.
//!
//! It also measures where mode 3 starts (ceres-core's
//! `background::check::before_start`): on a visible line, render's mode 3
//! rises at `h = 154` (the line timing's half dot, `h = 904` where vclk
//! rises), and at `h = 162`, the end of the dot before ceres's mode 3 loop
//! starts, the pipeline is `BEFORE_START`, whatever SCX is. And where the
//! CPU's writes to SCX reach the latch (ceres-core's
//! `background::check::writes`): the write strobe on SCX's address rises
//! with the clock at `h = 1 (mod 8)`, after an edge that carries SCX's
//! address and 0xFF, and stays up three edges.
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

/// The line timing's half dot at which vclk rises (after its edge).
const LINE_END_H: u32 = 904;
const LINE_H: u32 = 912;
/// Where render's mode 3 rises, and the end of the dot before ceres's loop.
const MODE3_RISE_H: u32 = 154;
const BEFORE_START_H: u32 = 162;

/// A write strobe on SCX's address: the half dot (mod 8) it rises at, and the
/// edges it lasts.
const STROBE_PHASE: u32 = 1;
const STROBE_EDGES: u32 = 3;
/// SCX's address.
const SCX: u16 = 0xFF43;

/// The pipeline at `BEFORE_START_H` (its SCX and inputs aside): ceres-core's
/// `background::check::before_start`.
const BEFORE_START: NetPipeline = NetPipeline {
    mode3: true,
    voga: false,
    px: 0,
    fetch_count: 4,
    lyzu: true,
    nyka: false,
    pory: false,
    pygo: false,
    started: false,
    fine: 0,
    puxa: false,
    nyze: false,
    discard: true,
    ryfa: false,
    rene: false,
    scx: 0,
    inputs: EdgeInputs {
        clk: false,
        ppu_reset: false,
        hard_reset: false,
        line_start: false,
        stop_scan: false,
        cpu_wr_sync: false,
        addr: 0,
        data: 0,
    },
};

/// What a ROM's run found: the edges compared, those in mode 3, and the
/// first difference; the background-only lines whose mode 3 started as
/// `BEFORE_START` has it, and the first that did not; the SCX write strobes,
/// and the first that did not rise or last as `STROBE_PHASE` and
/// `STROBE_EDGES` have it.
struct Found {
    rom: String,
    compared: u64,
    in_mode3: u64,
    first: Option<String>,
    starts: u64,
    first_start: Option<String>,
    strobes: u64,
    first_strobe: Option<String>,
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
        starts: 0,
        first_start: None,
        strobes: 0,
        first_strobe: None,
    };
    // The line timing's half dot (once vclk has risen), and where mode 3
    // rose in this line, if the window and the sprites were idle.
    let mut h: Option<u32> = None;
    let mut rose_at: Option<u32> = None;
    // The edges the SCX write strobe has been up, and whether it rose as it
    // should.
    let mut strobe: Option<(u32, bool)> = None;
    while run.edge < edges {
        let before = run.chip.render;
        let vclk = run.chip.lcd.vclk;
        run.step();
        let after = &run.chip.render;
        h = if !vclk & run.chip.lcd.vclk {
            Some(LINE_END_H)
        } else {
            h.map(|h| (h + 1) % LINE_H)
        };
        if after.inputs.ppu_reset {
            h = None;
        }
        let (b, i) = (&before.inputs, &after.inputs);
        let strobed = i.cpu_wr_sync && i.addr == SCX;
        strobe = match (strobe, strobed, h) {
            (None, true, Some(h)) => {
                found.strobes += 1;
                let bus = b.addr == SCX && b.data == 0xFF && !b.cpu_wr_sync;
                Some((1, h % 8 == STROBE_PHASE && i.clk && bus))
            }
            (Some((n, ok)), true, _) => Some((n + 1, ok)),
            (Some((n, ok)), false, _) => {
                if (!ok || n != STROBE_EDGES) && found.first_strobe.is_none() {
                    found.first_strobe = Some(format!(
                        "edge {} ly {}: SCX strobe of {n} edges ending at h = {h:?}, rose as it should: {ok}",
                        run.edge, run.chip.lcd.ly
                    ));
                }
                None
            }
            (None, _, _) => None,
        };
        if !before.mode3 && after.mode3 && idle(after) {
            rose_at = h;
        }
        if let (Some(BEFORE_START_H), Some(rose)) = (h, rose_at) {
            rose_at = None;
            let mut pipeline = NetPipeline::of(after);
            pipeline.scx = 0;
            pipeline.inputs = BEFORE_START.inputs;
            let quiet = !after.inputs.ppu_reset && !after.inputs.stop_scan && !after.inputs.clk;
            if quiet && idle(after) && run.chip.lcd.ly < 144 {
                found.starts += 1;
                if (rose != MODE3_RISE_H || pipeline != BEFORE_START) && found.first_start.is_none()
                {
                    found.first_start = Some(format!(
                        "edge {} ly {}: mode 3 rose at h = {rose}\n  pipeline {:?}",
                        run.edge,
                        run.chip.lcd.ly,
                        Fields(&NetPipeline::of(after)),
                    ));
                }
            }
        }
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
            "{}: {} edges compared, {} in mode 3, {} starts, {} SCX writes{}{}{}",
            found.rom,
            found.compared,
            found.in_mode3,
            found.starts,
            found.strobes,
            if found.first.is_some() {
                ", DIFFERS"
            } else {
                ""
            },
            if found.first_start.is_some() {
                ", START DIFFERS"
            } else {
                ""
            },
            if found.first_strobe.is_some() {
                ", WRITE DIFFERS"
            } else {
                ""
            }
        );
        if let Some(first) = &found.first {
            differ += 1;
            eprintln!("  {first}");
        }
        if let Some(first) = &found.first_start {
            differ += 1;
            eprintln!("  {first}");
        }
        if let Some(first) = &found.first_strobe {
            differ += 1;
            eprintln!("  {first}");
        }
    }
    assert!(differ == 0, "{differ} ROMs differ");
}
