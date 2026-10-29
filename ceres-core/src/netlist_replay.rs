//! Replays the readable model's traces of the divider, the timer, IF, OAM
//! DMA, the line timing and the OAM scan (`hdl/trace-blocks.sh`, in
//! `hdl/build/blocks`) through their transcriptions, `NetTimer`, `NetIf`,
//! `NetDma`, `NetLcd` and `NetScan`, phase by phase, and reports where a
//! transcription parts from the model. The proofs against ceres
//! (`timing/netlist_proofs.rs`, `interrupts/netlist_proofs.rs`,
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
//! `cargo test -p ceres-core --release netlist_replay -- --ignored`

use {
    crate::{
        interrupts::netlist::{NetIf, lowest},
        memory::netlist::{NetDma, Phase as DmaPhase},
        ppu::{NetLcd, NetOamLatches, NetScan, OamInputs, ScanClocks},
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
    /// The line timing (older traces lack it).
    lcd: Option<LcdLine>,
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
        ref rest @ ..,
    ] = fields.as_slice()
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
        lcd: match *rest {
            [] | [_] => None,
            [_, inputs, flops, lx, ly, lyc, enables, ref rest @ ..] => Some(LcdLine {
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
                read: match *rest {
                    [] => None,
                    [byte, ..] => Some(hex8(byte)?),
                },
                scan: match *rest {
                    [] | [_] => None,
                    [_, ref scan @ ..] => Some(parse_scan(scan)?),
                },
            }),
            _ => return None,
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

/// The flip-flops of the line timing that differ from the model's (the
/// LCD's pins' are not traced).
fn lcd_differences(net: &NetLcd, model: &NetLcd) -> Vec<&'static str> {
    let fields: [(&str, bool); 15] = [
        ("clk", net.clk == model.clk),
        ("wuvu", net.wuvu == model.wuvu),
        ("vena", net.vena == model.vena),
        ("wosu", net.wosu == model.wosu),
        ("lx", net.lx == model.lx),
        ("vclk", net.vclk == model.vclk),
        ("nype", net.nype == model.nype),
        ("ly", net.ly == model.ly),
        ("line_153", net.line_153 == model.line_153),
        ("first_line", net.first_line == model.first_line),
        ("mode1", net.mode1 == model.mode1),
        ("lyc", net.lyc == model.lyc),
        ("lyc_match", net.lyc_match == model.lyc_match),
        ("stat_enable", net.stat_enable == model.stat_enable),
        ("-", true),
    ];
    fields
        .iter()
        .filter(|&&(_, same)| !same)
        .map(|&(name, _)| name)
        .collect()
}

/// The CPU's write, through LYC's and the enables' latches, which follow
/// the data bus while the strobe is up: as it opens they take the bus as it
/// was (precharged, FF), and then the byte written (STAT's write glitch).
/// Whether the interrupt's line rose, from `level`.
fn lcd_write(
    net: &mut NetLcd,
    line: &Line,
    previous: &Line,
    mode0: bool,
    level: &mut bool,
    report: &mut Report,
) -> bool {
    let mut rose = false;
    let opens = !previous.wr;
    for addr in [0xFF45, 0xFF41] {
        if !(line.wr && line.addr == addr) {
            continue;
        }
        for data in [opens.then_some(previous.data), Some(line.data)]
            .into_iter()
            .flatten()
        {
            if addr == 0xFF45 {
                net.lyc = data;
            } else {
                net.stat_enable = data & 0x78;
            }
            let now = net.int_stat(mode0);
            rose |= now && !*level;
            *level = now;
        }
        if opens {
            report.saw(if addr == 0xFF45 {
                "LCD: an LYC write"
            } else {
                "LCD: a STAT write"
            });
        }
    }
    rose
}

/// A line of a trace has the flip-flops as the edge that began its phase
/// left them, and the clock as the next edge (the one that ends the phase)
/// leaves it: `clk` is that edge's.
fn replay_lcd(name: &str, lines: &[Line], report: &mut Report) {
    let Some(start) = lines.first().and_then(|l| l.lcd.as_ref()) else {
        return;
    };
    let mut net = start.net;
    net.clk = !start.clk;
    let mut before = start;
    // The core's data bus is precharged high and only pulled low while a
    // read drives it, from E until the read takes it in the middle of H:
    // the read takes the AND of what the register showed in E to H.
    let mut bus: Option<u8> = None;
    for (n, line) in lines.iter().enumerate().skip(1) {
        let Some(lcd) = line.lcd.as_ref() else {
            return;
        };
        let place = format!("{name}:{}", n + 1);
        // The clock's edge, unless the LCD's reset holds the clock's
        // flip-flops.
        let mut rose = false;
        if before.clk == net.clk {
            report.differ("LCD: a phase without an edge".to_owned(), place.clone());
        } else if before.ppu_reset {
            net.clk = before.clk;
        } else {
            rose = net.edge(before.mode0);
        }
        let mut level = net.int_stat(before.mode0);
        rose |= lcd_write(
            &mut net,
            line,
            &lines[n - 1],
            before.mode0,
            &mut level,
            report,
        );
        if lcd.ppu_reset {
            net.reset();
        }
        for now in [net.int_stat(before.mode0), net.int_stat(lcd.mode0)] {
            // The second: mode 0, which rises after the edge's flip-flops.
            rose |= now && !level;
            level = now;
        }
        report.phases += 1;

        let model_rose = line.rose & 2 != 0;
        if model_rose {
            report.saw("LCD: the STAT line rises");
        }
        if rose != model_rose {
            report.differ(
                format!("LCD, phase {}: the STAT line's rise", line.phase),
                format!("{place} (rose {rose}, model {model_rose})"),
            );
        }
        let mut model = lcd.net;
        model.clk = before.clk;
        let differ = lcd_differences(&net, &model);
        if !differ.is_empty() {
            report.differ(
                format!("LCD, phase {}: {}", line.phase, differ.join(" ")),
                place.clone(),
            );
            let (frame, line_parity, cpg_lx) = (net.frame, net.line_parity, net.cpg_lx);
            net = model;
            net.frame = frame;
            net.line_parity = line_parity;
            net.cpg_lx = cpg_lx;
        }
        let shown = net.read(line.addr, lcd.mode2, lcd.mode3);
        bus = match (line.rd, line.phase) {
            (true, 'e') => shown,
            (true, 'f' | 'g' | 'h') => bus.zip(shown).map(|(a, b)| a & b),
            _ => None,
        };
        if line.rd
            && line.phase == 'h'
            && let (Some(value), Some(taken)) = (bus, lcd.read)
        {
            report.saw(match line.addr {
                0xFF41 => "LCD: a STAT read",
                0xFF44 => "LCD: an LY read",
                _ => "LCD: an LYC read",
            });
            if value != taken {
                report.differ(
                    format!("LCD: a read of {:04X}", line.addr),
                    format!("{place} (reads {value:02X}, model {taken:02X})"),
                );
            }
        }
        if net.line_153 && !before.net.line_153 {
            report.saw("LCD: line 153's LY reset");
        }
        if net.mode1 && !before.net.mode1 {
            report.saw("LCD: VBlank starts");
        }
        before = lcd;
    }
}

/// The OAM scan, edge by edge, on the line timing's traced clocks.
fn replay_scan(name: &str, lines: &[Line], report: &mut Report) {
    let Some(start) = lines.first().and_then(|l| l.lcd.as_ref()) else {
        return;
    };
    let Some(mut net) = start.scan.as_ref().map(|s| s.net) else {
        return;
    };
    let mut before = start;
    for (n, line) in lines.iter().enumerate().skip(1) {
        let Some(after) = line.lcd.as_ref() else {
            return;
        };
        let (Some(scan_before), Some(scan)) = (before.scan.as_ref(), after.scan.as_ref()) else {
            return;
        };
        let place = format!("{name}:{}", n + 1);
        if before.ppu_reset {
            net.reset();
        } else {
            let clocks = ScanClocks {
                rising: before.clk,
                wuvu_before: before.net.wuvu,
                wuvu_after: after.net.wuvu,
                wosu_before: before.net.wosu,
                vclk: before.net.vclk,
                ly: before.net.ly,
            };
            net.edge(clocks, scan_before.y, scan_before.tall);
        }
        if after.ppu_reset {
            net.reset();
        }
        report.phases += 1;
        let y_match = NetScan::y_match(scan.y, after.net.ly, scan.tall);
        let save = net
            .save(after.net.wosu, scan.y, after.net.ly, scan.tall)
            .map_or(0, |slot| 1 << slot);
        if save != 0 {
            report.saw("scan: a save");
        }
        if net.stop_scan() && !scan_before.net.stop_scan() {
            report.saw("scan: a stop");
        }
        if scan.tall != scan_before.tall && scan.net.scanning {
            report.saw("scan: LCDC bit 2 changes");
        }
        if net != scan.net || y_match != scan.y_match || save != scan.save {
            report.differ(
                format!("scan, phase {}", line.phase),
                format!(
                    "{place} ({net:?} y_match {y_match} save {save:03X}, model {:?} y_match {} save {:03X})",
                    scan.net, scan.y_match, scan.save
                ),
            );
            net = scan.net;
        }
        if let Some(previous) = lines.get(n - 1) {
            replay_oam_latches(previous, line, &place, report);
        }
        before = after;
    }
}

/// OAM's latches of the Y, in a phase: `NetOamLatches`, with the trace's
/// first latches where they are open (the RAM's byte is not traced). The
/// CPU's read strobe is not traced either: a phase where the CPU addresses
/// OAM is left out.
fn replay_oam_latches(before: &Line, after: &Line, place: &str, report: &mut Report) {
    let (Some(lcd_before), Some(lcd)) = (before.lcd.as_ref(), after.lcd.as_ref()) else {
        return;
    };
    let (Some(scan_before), Some(scan)) = (lcd_before.scan.as_ref(), lcd.scan.as_ref()) else {
        return;
    };
    let (Some(oam_before), Some(oam), Some(dma_phi)) =
        (scan_before.oam.as_ref(), scan.oam.as_ref(), after.dma_phi)
    else {
        return;
    };
    if before.addr >> 8 == 0xFE || after.addr >> 8 == 0xFE {
        report.saw("OAM: the CPU addresses OAM (left out)");
        return;
    }
    let inputs = OamInputs {
        scanning: scan.net.scanning,
        run: after.dma.run,
        dma_phi,
        wuvu: lcd.net.wuvu,
        wosu: lcd.net.wosu,
        fetch_read: oam.fetch_read,
        fetch_hold: oam.fetch_hold,
    };
    let mut latches = NetOamLatches {
        first: oam_before.first,
        second: scan_before.y,
    };
    latches.settle(inputs, oam.first);
    report.phases += 1;
    if inputs.run && inputs.scanning {
        report.saw("OAM: OAM DMA hides OAM from the scan");
    }
    if inputs.precharge() && latches.second != scan_before.y {
        report.saw("OAM: the second latches take a new Y");
    }
    let model = NetOamLatches {
        first: oam.first,
        second: scan.y,
    };
    if latches != model || inputs.mode2() != lcd.mode2 {
        report.differ(
            format!("OAM latches, phase {}", after.phase),
            format!(
                "{place} ({latches:?} mode 2 {}, model {model:?} mode 2 {})",
                inputs.mode2(),
                lcd.mode2
            ),
        );
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
        replay_lcd(name, &lines, &mut report);
        replay_scan(name, &lines, &mut report);
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
