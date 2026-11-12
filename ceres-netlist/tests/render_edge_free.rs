//! `NetPipeline` (`ceres_netlist::render_edge`) against `NetRender`, edge by
//! edge, from states that no ROM run reaches. The proofs quantify over every
//! state a relation allows, so `tests/render_edge.rs` (which compares on
//! `NetChip`'s runs of the test ROMs) is not enough.
//!
//! Each run starts from `NetRender::power_on`, which is consistent, and is
//! driven by a seeded xorshift generator through a legal line schedule (line
//! start, the OAM scan's stop, mode 3, VBlank, LY, the line's clocks), with
//! random inputs the ROMs do not produce: CPU writes to SCX, WY and WX at
//! random times, random LCDC, random sprite X values saved while the scan
//! runs (written to the store, then the wires are settled, as
//! `NetRender::settle_wires` is for state set from outside), and random
//! `ceno` during the scan, and resets held as levels (`ppu_reset` and
//! `hard_reset`, a few edges to a few hundred). At each edge the pipeline, taken
//! from render's state before it, must reach render's state after it, under
//! the same condition as `tests/render_edge.rs`.
//!
//! One difference is known and left out, counted: the edge at which the LCD's
//! reset rises while mode 3 is running. Render settles that edge delta by
//! delta, and its fetcher's restart (TEVO: mode 3, the first fetch not
//! started, NYKA and PORY) still sees mode 3 in the delta before the reset
//! clears it, so the fetcher's count is cleared; the pipeline clears mode 3
//! first, and its count stays (so too, in a few such edges, the window's
//! column, a sprite's slot and the pixel counter; 56 of 1000 such edges differ
//! here). The proofs
//! against the netlist take no reset in mode 3 (`check::quiet`), and the
//! ROMs' runs turn the LCD off outside it.
//!
//! `cargo test -p ceres-netlist --release --test render_edge_free`

#![expect(
    clippy::print_stderr,
    reason = "a test, which reports what it compared"
)]

use ceres_netlist::{
    render::{NetRender, RenderInputs, vram_power_on},
    render_edge::{EdgeInputs, NO_X, NetPipeline, SLOTS},
};

/// The edges of a line (its half dots), and of the OAM scan (the scan saves
/// sprites before `SCAN_END` and stops at it).
const EDGES: u32 = 912;
const SCAN_END: u32 = 80;
/// Lines per run, and runs.
const LINES: u32 = 3;
const RUNS: u64 = 2000;
/// The line from which VBlank (mode 1) holds.
const VBLANK_LY: u8 = 144;
/// The lines in a frame.
const FRAME_LINES: u8 = 154;
/// The addresses the CPU writes that the window and the scroll read.
const SCX: u16 = 0xFF43;
const WY: u16 = 0xFF4A;
const WX: u16 = 0xFF4B;
/// LCDC's bit for the LCD's enable.
const LCDC_LCD_ON: u8 = 0x80;

/// A xorshift64 generator (no dependencies): deterministic per seed.
struct Rng(u64);

impl Rng {
    const fn step(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    /// True one time in `one_in`.
    const fn chance(&mut self, one_in: u64) -> bool {
        self.step().is_multiple_of(one_in)
    }

    /// A number below `n`.
    fn below(&mut self, n: u64) -> u8 {
        u8::try_from(self.step() % n).unwrap_or(0)
    }

    const fn byte(&mut self) -> u8 {
        self.step().to_le_bytes()[0]
    }
}

/// What the runs compared.
#[derive(Default)]
struct Tally {
    runs: u64,
    edges: u64,
    in_mode3: u64,
    in_window: u64,
    sprites_matched: u64,
    saves: u64,
    resets: u64,
    /// Edges where the LCD's reset rises in mode 3, which are not compared,
    /// and those of them where the two differ.
    reset_rises: u64,
    reset_rises_differing: u64,
}

/// The pipeline's fields that differ between two states, by name, with
/// their values; `inputs` if the inputs differ.
#[expect(clippy::cognitive_complexity, reason = "a field at a time")]
fn differing(a: &NetPipeline, b: &NetPipeline) -> String {
    let mut out = Vec::new();
    macro_rules! field {
        ($($f:ident),* $(,)?) => {
            $(
                if a.$f != b.$f {
                    out.push(format!("{} {:?} != {:?}", stringify!($f), a.$f, b.$f));
                }
            )*
        };
    }
    field!(
        mode3,
        voga,
        px,
        fetch_count,
        lyzu,
        nyka,
        pory,
        pygo,
        started,
        fine,
        puxa,
        nyze,
        discard,
        ryfa,
        rene,
        scx,
        wy,
        wx,
        sary,
        wy_latch,
        pyco,
        nunu,
        nopa,
        in_window,
        win_x,
        win_y,
        rydy,
        sovy,
        xs,
        fetched,
        sobu,
        suda,
        sprite_step,
        busy,
        tobu,
        vonu,
        seba,
        tyfo,
    );
    if a.inputs != b.inputs {
        out.push("inputs".to_owned());
    }
    out.join("; ")
}

/// The inputs an edge takes, for a report.
fn describe(i: &RenderInputs) -> String {
    format!(
        "clk {} ppu_reset {} hard_reset {} hclk {} mode1 {} ly {} lcdc {:02x} \
         line_start {} stop_scan {} ceno {} cpu_wr_sync {} addr {:04x} data {:02x}",
        u8::from(i.clk),
        u8::from(i.ppu_reset),
        u8::from(i.hard_reset),
        u8::from(i.hclk),
        u8::from(i.mode1),
        i.ly,
        i.lcdc,
        u8::from(i.line_start),
        u8::from(i.stop_scan),
        u8::from(i.ceno),
        u8::from(i.cpu_wr_sync),
        i.addr,
        i.data,
    )
}

/// An edge of render's, from `before` to `after` with `inputs`, against the
/// pipeline's from `before`: tallied, and equal unless it is the known
/// difference. `at` is the run's seed, the line and the edge.
fn compare(
    tally: &mut Tally,
    at: (u64, u32, u32),
    before: &NetRender,
    after: &NetRender,
    inputs: &RenderInputs,
) {
    // As tests/render_edge.rs: edges where the scan saves are not
    // comparable (none is made here, so this never skips).
    if before.inputs.save != 0 || after.inputs.save != 0 {
        return;
    }
    let mut piped = NetPipeline::of(before);
    piped.edge(EdgeInputs::of(&after.inputs));
    let expected = NetPipeline::of(after);
    if inputs.ppu_reset && !before.inputs.ppu_reset && before.mode3 {
        // The known difference (the module's comment).
        tally.reset_rises += 1;
        tally.reset_rises_differing += u64::from(piped != expected);
        return;
    }
    tally.edges += 1;
    tally.in_mode3 += u64::from(before.mode3 | after.mode3);
    tally.in_window += u64::from(after.in_window);
    tally.sprites_matched += u64::from(after.wires.x_match);
    tally.resets += u64::from(inputs.ppu_reset | inputs.hard_reset);
    let (seed, line, edge) = at;
    assert!(
        piped == expected,
        "seed {seed:#x}, line {line} (ly {}), edge {edge}: the pipeline differs from \
         render:{}\n  inputs: {}\n  before (render): m3 {} px {} xs {:?}",
        inputs.ly,
        differing(&piped, &expected),
        describe(inputs),
        before.mode3,
        before.px,
        before.store.xs,
    );
}

/// One run: a line schedule from `NetRender::power_on`, from `seed`.
fn run(seed: u64, tally: &mut Tally) {
    let mut rng = Rng(seed);
    let mut vram = vram_power_on();
    let mut render = NetRender::power_on();
    render.settle_wires(&vram);
    tally.runs += 1;

    let mut ly = rng.below(u64::from(FRAME_LINES));
    let mut lcdc = LCDC_LCD_ON | rng.byte();
    // The CPU's write strobe: it rises on the edge 1 mod 8 and stays up three
    // edges, on the address and data the edge 0 mod 8 drove.
    let mut strobe_left = 0_u32;
    let (mut bus_addr, mut bus_data) = (SCX, 0_u8);
    // The LCD's reset and the chip's: levels, held for a few edges or more.
    let (mut ppu_reset_left, mut hard_reset_left) = (0_u32, 0_u32);

    for line in 0..LINES {
        if line > 0 {
            ly = (ly + 1) % FRAME_LINES;
        }
        if rng.chance(3) {
            lcdc = LCDC_LCD_ON | rng.byte();
        }
        let mode1 = ly >= VBLANK_LY;
        for e in 0..EDGES {
            let phase = e % 8;
            if phase == 0 {
                (bus_addr, bus_data) = match rng.below(4) {
                    0 => (SCX, rng.byte()),
                    1 => (WY, if rng.chance(2) { ly } else { rng.byte() }),
                    2 => (
                        WX,
                        if rng.chance(2) {
                            rng.below(168)
                        } else {
                            rng.byte()
                        },
                    ),
                    _ => (0xFF00 | u16::from(rng.byte()), rng.byte()),
                };
            }
            if phase == 1 && strobe_left == 0 && rng.chance(3) {
                strobe_left = 3;
            }
            let cpu_wr_sync = strobe_left > 0;
            strobe_left = strobe_left.saturating_sub(1);
            let ceno = e < SCAN_END && rng.chance(2);
            if ppu_reset_left == 0 && rng.chance(2000) {
                ppu_reset_left = 2 + u32::from(rng.below(300));
            }
            if hard_reset_left == 0 && rng.chance(20_000) {
                hard_reset_left = 2 + u32::from(rng.below(20));
            }
            let ppu_reset = ppu_reset_left > 0;
            let hard_reset = hard_reset_left > 0;
            ppu_reset_left = ppu_reset_left.saturating_sub(1);
            hard_reset_left = hard_reset_left.saturating_sub(1);
            let inputs = RenderInputs {
                clk: e % 2 == 1,
                ppu_reset,
                hard_reset,
                hclk: (e / 4) % 2 == 1,
                mode1,
                ly,
                lcdc,
                line_start: e == 0,
                stop_scan: e == SCAN_END,
                ceno,
                cpu_wr_sync,
                addr: bus_addr,
                data: bus_data,
                ..RenderInputs::POWER_ON
            };
            // The scan saves sprites: their X latches, while it runs.
            if (1..SCAN_END).contains(&e) && rng.chance(8) {
                let slot = usize::from(rng.below(10)) % SLOTS;
                let x = if rng.chance(4) { NO_X } else { rng.below(168) };
                render.store.xs[slot] = x;
                render.settle_wires(&vram);
                tally.saves += 1;
            }

            let before = render;
            render.edge(inputs, &mut vram);
            compare(tally, (seed, line, e), &before, &render, &inputs);
        }
    }
}

#[test]
fn pipeline_follows_render_from_free_states() {
    let mut tally = Tally::default();
    for seed in 1..=RUNS {
        // A seed's bits spread out, so that the runs' generators differ.
        run(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1, &mut tally);
    }
    eprintln!(
        "{} runs, {} edges compared ({} in mode 3, {} in the window, {} with a sprite \
         matched, {} sprite X saves, {} with a reset); {} more where the LCD's reset rises in \
         mode 3, {} of them differing",
        tally.runs,
        tally.edges,
        tally.in_mode3,
        tally.in_window,
        tally.sprites_matched,
        tally.saves,
        tally.resets,
        tally.reset_rises,
        tally.reset_rises_differing,
    );
}
