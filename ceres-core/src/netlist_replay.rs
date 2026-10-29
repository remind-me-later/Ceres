//! Replays the readable model's traces of the divider, the timer, IF and OAM
//! DMA (`hdl/trace-blocks.sh`, in `hdl/build/blocks`) through their
//! transcriptions, `NetTimer`, `NetIf` and `NetDma`, phase by phase, and
//! reports where a transcription parts from the model. The proofs against
//! ceres (`timing/netlist_proofs.rs`, `interrupts/netlist_proofs.rs`,
//! `netlist_proofs.rs`) rest on these transcriptions; this checks them.
//!
//! Each block starts from the model's state at a phase boundary. Then every
//! phase gets the trace's inputs (the CPU's access, the interrupt requests
//! and acknowledges) and its state is compared with the model's at the end
//! of that phase. After a difference the transcription takes the model's
//! state again, so that one run lists every kind of difference.
//!
//! `cargo test -p ceres-core --release netlist_replay -- --ignored`

use {
    crate::{
        interrupts::netlist::{NetIf, lowest},
        memory::netlist::{NetDma, Phase as DmaPhase},
        timing::netlist::{Access, NetTimer, Phase, Reg},
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
}

#[expect(clippy::similar_names, reason = "TIMA and TMA")]
fn parse(text: &str) -> Option<Line> {
    let fields: Vec<&str> = text.split_whitespace().collect();
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
        ref dma_phi @ ..,
    ] = fields.as_slice()
    else {
        return None;
    };
    let hex8 = |s: &str| u8::from_str_radix(s, 16).ok();
    let bit = |s: &str, i: usize| s.as_bytes().get(i).map(|&b| b == b'1');
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
        dma_phi: match *dma_phi {
            [] => None,
            [phi] => Some(bit(phi, 0)?),
            _ => return None,
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

const TIMER_PHASES: [(char, Option<Phase>); 8] = [
    ('b', Some(Phase::B)),
    ('c', Some(Phase::C)),
    ('d', Some(Phase::D)),
    ('e', Some(Phase::E)),
    ('f', Some(Phase::F)),
    ('g', Some(Phase::G)),
    ('h', Some(Phase::H)),
    // The A edge.
    ('a', None),
];

/// The timer as the model has it at the end of a phase.
const fn net_timer(line: &Line) -> NetTimer {
    NetTimer {
        count: line.div,
        tima: line.tima,
        tma: line.tma,
        tac: line.tac,
        int_timer: line.int_timer,
        bit7_before: line.bit7_before,
        timer_clock: line.timer_clock,
        write_tma: line.write_tma,
        write_tac: line.write_tac,
        latched_data: line.data,
        // The model, like dmg-sim, counts on the race.
        enable_race_counts: true,
    }
}

const fn timer_reg(addr: u16) -> Option<Reg> {
    match addr {
        0xFF04 => Some(Reg::Div),
        0xFF05 => Some(Reg::Tima),
        0xFF06 => Some(Reg::Tma),
        0xFF07 => Some(Reg::Tac),
        _ => None,
    }
}

/// The M-cycle's access to the timer, as `NetTimer` takes it: a write in E
/// to G, with the data at the end of G.
fn timer_access(cycle: &[Line], place: &str, report: &mut Report) -> Access {
    let mut access = Access::None;
    for line in cycle {
        let Some(reg) = timer_reg(line.addr).filter(|_| line.wr) else {
            continue;
        };
        if !matches!(line.phase, 'e' | 'f' | 'g') {
            report.differ(
                format!("timer: a write in phase {}", line.phase),
                place.to_owned(),
            );
        }
        if line.phase == 'g' {
            access = Access::Write(reg, line.data);
        }
    }
    if let Access::Write(_, data) = access {
        for line in cycle
            .iter()
            .filter(|l| matches!(l.phase, 'e' | 'f') && l.wr)
        {
            if line.data != data {
                report.differ(
                    format!(
                        "timer: the data bus changes during the write (in {})",
                        line.phase
                    ),
                    place.to_owned(),
                );
            }
        }
    }
    access
}

fn compare_timer(net: &NetTimer, line: &Line) -> Vec<String> {
    let mut fields = Vec::new();
    let mut check = |name: &str, got: u32, want: u32| {
        if got != want {
            fields.push(format!("{name} {got:X}, model {want:X}"));
        }
    };
    check("DIV", net.count.into(), line.div.into());
    check("TIMA", net.tima.into(), line.tima.into());
    check("TMA", net.tma.into(), line.tma.into());
    check("TAC", net.tac.into(), line.tac.into());
    check("int_timer", net.int_timer.into(), line.int_timer.into());
    check(
        "bit7_before",
        net.bit7_before.into(),
        line.bit7_before.into(),
    );
    check(
        "timer_clock",
        net.timer_clock.into(),
        line.timer_clock.into(),
    );
    check("write_tma", net.write_tma.into(), line.write_tma.into());
    check("write_tac", net.write_tac.into(), line.write_tac.into());
    fields
}

fn replay_timer(name: &str, lines: &[Line], report: &mut Report) {
    let Some(start) = lines.iter().position(|l| l.phase == 'a') else {
        return;
    };
    let mut net = net_timer(&lines[start]);
    for (n, cycle) in lines[start + 1..].as_chunks::<8>().0.iter().enumerate() {
        let first = start + 2 + 8 * n;
        let place = format!("{name}:{first}");
        if cycle
            .iter()
            .zip(TIMER_PHASES)
            .any(|(l, (p, _))| l.phase != p)
        {
            report.differ("timer: the phases are out of order".to_owned(), place);
            return;
        }
        let access = timer_access(cycle, &place, report);
        let reloading = net.int_timer;
        match access {
            Access::Write(Reg::Div, _) => report.saw("timer: a write of DIV"),
            Access::Write(Reg::Tima, _) if reloading => {
                report.saw("timer: a write of TIMA as it reloads");
            }
            Access::Write(Reg::Tima, _) => report.saw("timer: a write of TIMA"),
            Access::Write(Reg::Tma, _) if reloading => {
                report.saw("timer: a write of TMA as TIMA reloads");
            }
            Access::Write(Reg::Tma, _) => report.saw("timer: a write of TMA"),
            Access::Write(Reg::Tac, _) => report.saw("timer: a write of TAC"),
            Access::None | Access::Read(_) => {}
        }
        for (k, (line, (letter, phase))) in cycle.iter().zip(TIMER_PHASES).enumerate() {
            let (tima, int_timer) = (net.tima, net.int_timer);
            match phase {
                Some(phase) => net.phase(phase, access),
                None => net.edge_a(),
            }
            report.phases += 1;
            if net.int_timer && !int_timer {
                report.saw("timer: an overflow");
            }
            if phase.is_some()
                && matches!(access, Access::Write(Reg::Div | Reg::Tac, _))
                && net.tima == tima.wrapping_add(1)
            {
                report.saw("timer: TIMA counts as DIV or TAC is written");
            }
            let fields = compare_timer(&net, line);
            if !fields.is_empty() {
                let kind = format!("timer, phase {letter}: {}", kind_of(&fields));
                report.differ(
                    kind,
                    format!("{name}:{} ({})", first + k, fields.join(", ")),
                );
                net = net_timer(line);
            }
        }
    }
}

/// The names of the fields that differ, for grouping.
fn kind_of(fields: &[String]) -> String {
    fields
        .iter()
        .map(|f| f.split(' ').next().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("+")
}

fn replay_if(name: &str, lines: &[Line], report: &mut Report) {
    let Some(start) = lines
        .iter()
        .skip(1)
        .position(|l| l.phase == 'e')
        .map(|i| i + 1)
    else {
        return;
    };
    let mut net = NetIf {
        flags: lines[start - 1].flags,
        ie: lines[start - 1].ie,
    };
    for (n, cycle) in lines[start..].as_chunks::<8>().0.iter().enumerate() {
        let first = start + 1 + 8 * n;
        let place = format!("{name}:{first}");
        if cycle
            .iter()
            .zip("efghabcd".chars())
            .any(|(l, p)| l.phase != p)
        {
            report.differ("IF: the phases are out of order".to_owned(), place);
            return;
        }
        // The latches close at the start of E: the read's, and the
        // requests' for the entry's choice.
        net.ie = lines[start + 8 * n - 1].ie;
        let at_e = net.flags;
        let selected = lowest(net.ie & at_e);
        let acks = cycle.iter().fold(0, |acks, l| acks | l.acks);
        if acks != 0 {
            if acks != selected {
                report.differ(
                    "IF: the entry takes another interrupt".to_owned(),
                    format!("{place} (takes {selected:02X}, model {acks:02X})"),
                );
            }
            if cycle
                .iter()
                .any(|l| l.acks != 0 && !matches!(l.phase, 'g' | 'h'))
            {
                report.differ(
                    "IF: an acknowledge outside G and H".to_owned(),
                    place.clone(),
                );
            }
        }
        if cycle.iter().any(|l| l.rd && l.addr == 0xFF0F) && cycle[3].read_value != at_e {
            report.differ(
                "IF: a read reads other flags".to_owned(),
                format!(
                    "{place} (reads {at_e:02X}, model {:02X})",
                    cycle[3].read_value
                ),
            );
        }
        if acks != 0 {
            report.saw("IF: an entry");
        }
        if cycle.iter().any(|l| l.rd && l.addr == 0xFF0F) {
            report.saw("IF: a read");
        }
        for (k, line) in cycle.iter().enumerate() {
            let write = (line.wr && line.addr == 0xFF0F).then_some(line.data);
            if write.is_some() && line.phase == 'e' {
                report.saw("IF: a write");
            }
            if line.rose != 0 && write.is_some() {
                report.saw("IF: a source rises as IF is written");
            }
            if line.rose & line.acks != 0 {
                report.saw("IF: a source rises as it is acknowledged");
            }
            net.phase(line.rose, write, line.acks);
            report.phases += 1;
            if net.flags != line.flags {
                report.differ(
                    format!("IF, phase {}: the flags", line.phase),
                    format!(
                        "{name}:{} ({:02X}, model {:02X})",
                        first + k,
                        net.flags,
                        line.flags
                    ),
                );
                net.flags = line.flags;
            }
        }
    }
}

const DMA_PHASES: [(char, DmaPhase); 8] = [
    ('a', DmaPhase::A),
    ('b', DmaPhase::B),
    ('c', DmaPhase::C),
    ('d', DmaPhase::D),
    ('e', DmaPhase::E),
    ('f', DmaPhase::F),
    ('g', DmaPhase::G),
    ('h', DmaPhase::H),
];

fn replay_dma(name: &str, lines: &[Line], report: &mut Report) {
    let Some(start) = lines
        .iter()
        .skip(1)
        .position(|l| l.phase == 'a')
        .map(|i| i + 1)
    else {
        return;
    };
    let mut net = lines[start - 1].dma;
    for (n, cycle) in lines[start..].as_chunks::<8>().0.iter().enumerate() {
        let first = start + 1 + 8 * n;
        if cycle.iter().zip(DMA_PHASES).any(|(l, (p, _))| l.phase != p) {
            report.differ(
                "DMA: the phases are out of order".to_owned(),
                format!("{name}:{first}"),
            );
            return;
        }
        if cycle.iter().any(|l| l.wr && l.addr == 0xFF46) {
            report.saw(if net.enabled {
                "DMA: a write of FF46 while a transfer runs"
            } else {
                "DMA: a write of FF46"
            });
        }
        for (k, (line, (letter, phase))) in cycle.iter().zip(DMA_PHASES).enumerate() {
            let write = (line.wr && line.addr == 0xFF46).then_some(line.data);
            let was = net;
            // The clock's edges, where the trace has it; else the phases'.
            match (line.dma_phi, lines[start + 8 * n + k - 1].dma_phi) {
                (Some(phi), Some(before)) => {
                    if phi && !before {
                        net.rise(write.is_some());
                    } else if !phi && before {
                        net.fall();
                    } else if phi && letter == 'e' {
                        report.saw("DMA: dma_phi stays high in E (HALT)");
                    } else {
                        // No edge.
                    }
                    net.settle(write);
                }
                _ => net.phase(phase, write),
            }
            report.phases += 1;
            if net.enabled && !was.enabled {
                report.saw("DMA: a transfer starts");
            }
            if was.enabled && !net.enabled {
                report.saw("DMA: a transfer ends");
            }
            let low = line
                .dma_phi
                .map_or(matches!(letter, 'e' | 'f' | 'g' | 'h'), |phi| !phi);
            let strobe = net.oam_write().is_some() && low && matches!(letter, 'e' | 'f' | 'g');
            if net != line.dma || strobe != line.dma_write {
                report.differ(
                    format!("DMA, phase {letter}"),
                    format!(
                        "{name}:{} ({net:?} strobe {strobe}, model {:?} strobe {})",
                        first + k,
                        line.dma,
                        line.dma_write
                    ),
                );
                net = line.dma;
            }
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
    assert!(!traces.is_empty(), "no traces in hdl/build/blocks");

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
    }

    let summary = summary(traces.len(), &report).expect("a String takes any text");
    eprintln!("{summary}");
    assert!(report.phases > 0, "no phases replayed");
    assert!(
        report.kinds.is_empty(),
        "the transcriptions part from the model:\n{summary}"
    );
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
