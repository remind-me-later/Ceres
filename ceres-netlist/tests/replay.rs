//! Replays the readable model's traces of the divider, the timer, IF, OAM
//! DMA, the line timing, the OAM scan and the clocks (`hdl/trace-blocks.sh`,
//! in `hdl/build/blocks`, and from the power-on in its `power-on/`) through
//! their transcriptions, `NetTimer`, `NetIf`, `NetDma`, `NetLcd`,
//! `NetScan`, `NetOamLatches`, `NetClocks`, `NetRender` (render's traces
//! only, in `render/`), `NetBus` (in `bus/`), the CPU core `NetCore` (in
//! `core/`) and the APU `NetApu` (in `apu/`), phase by phase, and reports
//! where a transcription parts from the model; and the blocks wired
//! together (`NetChip`), each taking from the others what the model wires
//! in, where the trace has the clocks. ceres-core's proofs against ceres
//! (its `timing/netlist_proofs.rs`, `interrupts/netlist_proofs.rs`,
//! `memory/dma/netlist_proofs.rs`, `ppu/display/timing/netlist_proofs.rs`,
//! `ppu/display/objects/netlist_proofs.rs`, `netlist_proofs.rs`) rest on
//! these transcriptions; this checks them.
//!
//! Each block starts from the model's state at a phase boundary. Then every
//! phase gets the trace's inputs (the CPU's access, the interrupt requests
//! and acknowledges) and its state is compared with the model's at the end
//! of that phase. After a difference the transcription takes the model's
//! state again, so that one run lists every kind of difference.
//!
//! `cargo test -p ceres-netlist --release --test replay -- --ignored --nocapture`
//! (with `REPLAY_ONLY=a,b`, only the traces whose names contain a or b).

mod apu;
mod bus;
mod chip;
mod clocks;
mod core;
mod lcd;
mod render;
mod timer_if_dma;

use {chip::*, clocks::*, lcd::*, timer_if_dma::*};

use {
    ceres_netlist::{
        Phase,
        chip::{ChipEvents, ChipInputs, ChipMemories, NetChip},
        clocks::{ClockInputs, ClockOutputs, NetClocks},
        dma::NetDma,
        interrupts::{NetIf, lowest},
        lcd_control::NetLcdControl,
        lcd_timing::NetLcd,
        oam::{NetOamLatches, OamInputs},
        oam_scan::{Clocks as ScanClocks, NetScan},
        render::{NetRender, PinLog, vram_power_on},
        timer::{Access, NetTimer, Reg},
    },
    std::{
        collections::BTreeMap,
        fmt::{self, Write as _},
        fs,
        path::Path,
    },
};

/// A line of a trace: the model at the end of a phase (see `BLOCKS` in
/// `hdl/board/gameboy_tb.vhd`).
#[expect(
    clippy::struct_excessive_bools,
    reason = "the trace's strobes and flip-flops"
)]
struct Line {
    phase: char,
    addr: u16,
    rd: bool,
    wr: bool,
    data: u8,
    div: u16,
    tima: u8,
    tma: u8,
    tac: u8,
    int_timer: bool,
    bit7_before: bool,
    timer_clock: bool,
    write_tma: bool,
    write_tac: bool,
    acks: u8,
    flags: u8,
    read_value: u8,
    ie: u8,
    /// The requests that rose in the phase, also for less than its length.
    rose: u8,
    dma: NetDma,
    /// OAM's DMA write strobe.
    dma_write: bool,
    /// The DMA's clock, high in A to D and while the CPU is halted (older
    /// traces lack it).
    dma_phi: Option<bool>,
    /// The line timing (older traces lack it).
    lcd: Option<LcdLine>,
    /// The clocks and resets (older traces lack them).
    clocks: Option<ClockLine>,
    /// Render and the sprite store (older traces lack them).
    render: Option<render::RenderLine>,
    /// The bus side (older traces lack it).
    bus: Option<bus::BusLine>,
    /// The CPU core (older traces lack it).
    core: Option<core::CoreLine>,
    /// The APU (older traces lack it).
    apu: Option<apu::ApuLine>,
}

/// The clocks in a line of a trace: what they take in, their latches and
/// resets, and the clocks they make.
struct ClockLine {
    inputs: ClockInputs,
    net: NetClocks,
    outputs: ClockOutputs,
}

/// The line timing in a line of a trace: its flip-flops (the LCD's pins'
/// left out), and the clock, the LCD's reset and the modes it takes in.
#[expect(
    clippy::struct_excessive_bools,
    reason = "the trace's clock, reset and modes"
)]
struct LcdLine {
    net: NetLcd,
    clk: bool,
    ppu_reset: bool,
    mode0: bool,
    mode2: bool,
    mode3: bool,
    /// Mode 0 rose in the phase and fell again before its end: a sprite at
    /// X 167 stops mode 3 4 ns after it ends (older traces lack it).
    mode0_pulse: bool,
    /// The byte a read takes, in the middle of phase H (older traces lack it).
    read: Option<u8>,
    /// The OAM scan (older traces lack it).
    scan: Option<ScanLine>,
}

/// The OAM scan in a line of a trace: its flip-flops, the save strobes and
/// the Y match, and what it takes in: the Y read from OAM and LCDC bit 2.
struct ScanLine {
    net: NetScan,
    y_match: bool,
    tall: bool,
    /// The slots' save strobes, bit `i` for slot `i`.
    save: u16,
    /// The Y the comparator sees: OAM's second even latches.
    y: u8,
    /// OAM's first even latches and the sprite fetch's strobes (older traces
    /// lack them).
    oam: Option<OamLine>,
}

/// OAM's first even latches in a line of a trace, and the sprite fetch's
/// strobes.
struct OamLine {
    first: u8,
    fetch_read: bool,
    fetch_hold: bool,
}

#[expect(clippy::similar_names, reason = "TIMA and TMA")]
#[expect(clippy::cognitive_complexity, reason = "a group of fields at a time")]
#[expect(clippy::too_many_lines, reason = "a group of fields at a time")]
fn parse(text: &str) -> Option<Line> {
    let mut all: Vec<&str> = text.split_whitespace().collect();
    // The APU's fields follow the core's.
    let apu = if all.len() == apu::APU_LINE_FIELDS {
        let fields = all.split_off(core::CORE_LINE_FIELDS);
        Some(apu::parse(&fields)?)
    } else {
        None
    };
    // The core's fields follow the bus side's; IE is the 14th field.
    let core = if all.len() == core::CORE_LINE_FIELDS {
        let fields = all.split_off(bus::BUS_LINE_FIELDS);
        Some(core::parse(&fields, hex8(all.get(13)?)?)?)
    } else {
        None
    };
    // The bus side's fields follow OAM's and VRAM's; the data pins are
    // among those, the 6th from their end.
    let bus = if all.len() == bus::BUS_LINE_FIELDS {
        let fields = all.split_off(render::OAM_LINE_FIELDS);
        let d = u8::from_str_radix(all.get(render::OAM_LINE_FIELDS - 6)?, 16).ok()?;
        Some(bus::parse(&fields, d)?)
    } else {
        None
    };
    // Render's fields follow the clocks'.
    let render = if matches!(
        all.len(),
        render::RENDER_LINE_FIELDS | render::OAM_LINE_FIELDS
    ) {
        let fields = all.split_off(CLOCK_LINE_FIELDS);
        Some(render::parse(&fields)?)
    } else {
        None
    };
    // The clocks' three fields end the lines that have them.
    let (fields, clocks) = match *all.as_slice() {
        [ref fields @ .., inputs, state, made] if all.len() == CLOCK_LINE_FIELDS => {
            (fields, Some(parse_clocks(inputs, state, made)?))
        }
        ref fields => (fields, None),
    };
    let &[
        phase,
        addr,
        rdwr,
        data,
        div,
        tima,
        tma,
        tac,
        bits,
        _requests,
        acks,
        flags,
        read_value,
        ie,
        rose,
        dma_source,
        dma_bits,
        dma_count,
        dma_write,
        ref rest @ ..,
    ] = fields
    else {
        return None;
    };
    Some(Line {
        phase: phase.chars().next()?,
        addr: u16::from_str_radix(addr, 16).ok()?,
        rd: bit(rdwr, 0)?,
        wr: bit(rdwr, 1)?,
        data: hex8(data)?,
        div: u16::from_str_radix(div, 16).ok()?,
        tima: hex8(tima)?,
        tma: hex8(tma)?,
        tac: hex8(tac)?,
        int_timer: bit(bits, 0)?,
        bit7_before: bit(bits, 1)?,
        timer_clock: bit(bits, 2)?,
        write_tma: bit(bits, 3)?,
        write_tac: bit(bits, 4)?,
        acks: hex8(acks)?,
        flags: hex8(flags)?,
        read_value: hex8(read_value)?,
        ie: hex8(ie)?,
        rose: hex8(rose)?,
        dma: NetDma {
            source: hex8(dma_source)?,
            requested: bit(dma_bits, 0)?,
            start_1: bit(dma_bits, 1)?,
            start_2: bit(dma_bits, 2)?,
            enabled: bit(dma_bits, 3)?,
            done: bit(dma_bits, 4)?,
            run: bit(dma_bits, 5)?,
            count: hex8(dma_count)?,
        },
        dma_write: bit(dma_write, 0)?,
        dma_phi: match *rest {
            [] => None,
            [phi, ..] => Some(bit(phi, 0)?),
        },
        // The line timing's fields follow the DMA's clock (older traces lack
        // them).
        lcd: match *rest {
            [] | [_] => None,
            ref lcd => Some(parse_lcd(lcd)?),
        },
        clocks,
        render,
        bus,
        core,
        apu,
    })
}

/// The line timing's fields, after the DMA's clock: `None` for fields that
/// do not parse.
fn parse_lcd(fields: &[&str]) -> Option<LcdLine> {
    let [_, inputs, flops, lx, ly, lyc, enables, ref rest @ ..] = *fields else {
        return None;
    };
    Some(LcdLine {
        net: NetLcd {
            clk: bit(inputs, 0)?,
            wuvu: bit(flops, 0)?,
            vena: bit(flops, 1)?,
            wosu: bit(flops, 2)?,
            lx: hex8(lx)?,
            vclk: bit(flops, 3)?,
            nype: bit(flops, 4)?,
            ly: hex8(ly)?,
            line_153: bit(flops, 5)?,
            first_line: bit(flops, 6)?,
            mode1: bit(flops, 7)?,
            frame: false,
            line_parity: false,
            cpg_lx: false,
            lyc: hex8(lyc)?,
            lyc_match: bit(flops, 8)?,
            stat_enable: hex8(enables)?,
        },
        clk: bit(inputs, 0)?,
        ppu_reset: bit(inputs, 1)?,
        mode0: bit(inputs, 2)?,
        mode2: bit(inputs, 3)?,
        mode3: bit(inputs, 4)?,
        mode0_pulse: bit(inputs, 5).unwrap_or(false),
        read: match *rest {
            [] => None,
            [byte, ..] => Some(hex8(byte)?),
        },
        scan: match *rest {
            [] | [_] => None,
            [_, ref scan @ ..] => Some(parse_scan(scan)?),
        },
    })
}

/// The fields of a line that has the clocks'.
const CLOCK_LINE_FIELDS: usize = 37;

/// The clocks' fields: what they take in, their latches and resets, and the
/// clocks they make (see `BLOCKS` in `hdl/board/gameboy_tb.vhd`).
fn parse_clocks(inputs: &str, state: &str, made: &str) -> Option<ClockLine> {
    let inputs = ClockInputs {
        ck1: bit(inputs, 0)?,
        reset_pin_n: bit(inputs, 1)?,
        t1: bit(inputs, 2)?,
        t2: bit(inputs, 3)?,
        osc_ena: bit(inputs, 4)?,
        clk_ena: bit(inputs, 5)?,
        mreq: bit(inputs, 6)?,
        write: bit(inputs, 7)?,
        test_reset: bit(inputs, 8)?,
        div_16hz: bit(inputs, 9)?,
    };
    Some(ClockLine {
        inputs,
        net: NetClocks {
            clk: inputs.ck1 || !inputs.osc_ena,
            afur: bit(state, 0)?,
            alef: bit(state, 1)?,
            apuk: bit(state, 2)?,
            adyk: bit(state, 3)?,
            waiting: bit(state, 4)?,
            sys_reset: bit(state, 6)?,
            cpu_sys_reset: bit(state, 7)?,
        },
        outputs: ClockOutputs {
            clk_1mhz: bit(made, 0)?,
            clk_t4: bit(made, 1)?,
            data_phase: bit(made, 2)?,
            exec_phase: bit(made, 3)?,
            pch_phase: bit(made, 4)?,
            cpu_wr_sync: bit(made, 5)?,
            cpu_ext_cs_en: bit(made, 6)?,
            phi: bit(made, 7)?,
            osc_stable: bit(state, 5)?,
            reset: !inputs.reset_pin_n,
            reset2: bit(state, 8)?,
        },
    })
}

/// A byte in hexadecimal.
fn hex8(s: &str) -> Option<u8> {
    u8::from_str_radix(s, 16).ok()
}

/// Bit `i` of a field of bits, from the left.
fn bit(s: &str, i: usize) -> Option<bool> {
    s.as_bytes().get(i).map(|&b| b == b'1')
}

/// The OAM scan's fields of a trace line, after the read.
fn parse_scan(fields: &[&str]) -> Option<ScanLine> {
    let (scan, oam) = match *fields {
        [_, _, _, _, _] => (fields, None),
        [ref scan @ .., first, fetch] => (scan, Some((first, fetch))),
        _ => return None,
    };
    let [scan_bits, index, count, save, y] = *scan else {
        return None;
    };
    Some(ScanLine {
        net: NetScan {
            start_scan: bit(scan_bits, 0)?,
            anel: bit(scan_bits, 1)?,
            scanning: bit(scan_bits, 2)?,
            ceno: bit(scan_bits, 3)?,
            index: hex8(index)?,
            byba: bit(scan_bits, 4)?,
            doba: bit(scan_bits, 5)?,
            not_saving: bit(scan_bits, 6)?,
            count: hex8(count)?,
        },
        y_match: bit(scan_bits, 7)?,
        tall: bit(scan_bits, 8)?,
        save: u16::from_str_radix(save, 16).ok()?,
        y: hex8(y)?,
        oam: match oam {
            None => None,
            Some((first, fetch)) => Some(OamLine {
                first: hex8(first)?,
                fetch_read: bit(fetch, 0)?,
                fetch_hold: bit(fetch, 1)?,
            }),
        },
    })
}

/// The differences found, by kind, with the first few places of each, and
/// how often the traces exercise each rule.
#[derive(Default)]
struct Report {
    kinds: BTreeMap<String, (usize, Vec<String>)>,
    phases: usize,
    seen: BTreeMap<&'static str, usize>,
}

impl Report {
    fn saw(&mut self, what: &'static str) {
        *self.seen.entry(what).or_default() += 1;
    }

    fn differ(&mut self, kind: String, place: String) {
        let &mut (ref mut count, ref mut places) = self.kinds.entry(kind).or_default();
        *count += 1;
        if places.len() < 3 {
            places.push(place);
        }
    }
}

#[test]
#[ignore = "needs traces of the model: hdl/trace-blocks.sh"]
#[expect(
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr,
    reason = "a test, which reports what it finds"
)]
fn netlist_replay() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../hdl/build/blocks");
    let mut traces: Vec<_> = fs::read_dir(&dir)
        .expect("hdl/build/blocks: run hdl/trace-blocks.sh first")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "txt"))
        .collect();
    traces.sort();
    traces.retain(|p| chosen(p));

    let mut report = Report::default();
    for path in &traces {
        let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("?");
        let text = fs::read_to_string(path).expect("a trace");
        let lines: Vec<Line> = text
            .lines()
            .enumerate()
            .map(|(n, l)| parse(l).unwrap_or_else(|| panic!("{name}:{}: not a trace line", n + 1)))
            .collect();
        replay_timer(name, &lines, &mut report);
        replay_if(name, &lines, &mut report);
        replay_dma(name, &lines, &mut report);
        replay_lcd(name, &lines, &mut report);
        replay_scan(name, &lines, &mut report);
        replay_clocks(name, &lines, &mut report, false);
        replay_chip(name, path, &lines, &mut report);
        core::replay(name, &lines, &mut report);
        render::replay(name, &lines, &mut report);
        bus::replay(name, path, &lines, &mut report);
        apu::replay(name, &lines, &mut report);
    }
    // The traces from the power-on (`FROM=power-on`): the clocks, and the
    // whole chip from its power-on state where the trace has every block.
    let mut power_on: Vec<_> = fs::read_dir(dir.join("power-on"))
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "txt"))
        .collect();
    power_on.sort();
    power_on.retain(|p| chosen(p));
    assert!(
        !traces.is_empty() || !power_on.is_empty(),
        "no traces in hdl/build/blocks"
    );
    for path in &power_on {
        let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("?");
        let text = fs::read_to_string(path).expect("a trace");
        let lines: Vec<Line> = text
            .lines()
            .enumerate()
            .map(|(n, l)| parse(l).unwrap_or_else(|| panic!("{name}:{}: not a trace line", n + 1)))
            .collect();
        replay_clocks(name, &lines, &mut report, true);
        run_chip_from_power_on(&format!("power-on/{name}"), path, &lines, &mut report);
    }

    let summary = summary(traces.len() + power_on.len(), &report).expect("a String takes any text");
    eprintln!("{summary}");
    assert!(report.phases > 0, "no phases replayed");
    assert!(
        report.kinds.is_empty(),
        "the transcriptions part from the model:\n{summary}"
    );
}

/// Whether to replay the trace at `path`: REPLAY_ONLY=a,b replays only the
/// traces whose names contain a or b.
fn chosen(path: &Path) -> bool {
    let Ok(only) = std::env::var("REPLAY_ONLY") else {
        return true;
    };
    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    only.split(',').any(|part| name.contains(part))
}

/// What the replay exercised and where the transcriptions differ.
fn summary(traces: usize, report: &Report) -> Result<String, fmt::Error> {
    let mut summary = String::new();
    writeln!(
        summary,
        "{traces} traces, {} phases replayed",
        report.phases
    )?;
    writeln!(summary, "exercised:")?;
    for (what, count) in &report.seen {
        writeln!(summary, "{count:9}  {what}")?;
    }
    writeln!(summary, "differences:")?;
    for (kind, &(count, ref places)) in &report.kinds {
        writeln!(summary, "{count:7}  {kind}")?;
        for place in places {
            writeln!(summary, "           {place}")?;
        }
    }
    Ok(summary)
}
