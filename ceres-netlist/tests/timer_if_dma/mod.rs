//! The timer, IF and OAM DMA, replayed phase by phase.

use super::*;

/// The timer's M-cycle: B to H, then the A edge.
pub const TIMER_PHASES: [(char, Phase); 8] = [
    ('b', Phase::B),
    ('c', Phase::C),
    ('d', Phase::D),
    ('e', Phase::E),
    ('f', Phase::F),
    ('g', Phase::G),
    ('h', Phase::H),
    ('a', Phase::A),
];
/// The timer as the model has it at the end of a phase.
pub const fn net_timer(line: &Line) -> NetTimer {
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
pub const fn timer_reg(addr: u16) -> Option<Reg> {
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
pub fn timer_access(cycle: &[Line], place: &str, report: &mut Report) -> Access {
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
pub fn compare_timer(net: &NetTimer, line: &Line) -> Vec<String> {
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
pub fn replay_timer(name: &str, lines: &[Line], report: &mut Report) {
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
            net.phase(phase, access);
            report.phases += 1;
            if net.int_timer && !int_timer {
                report.saw("timer: an overflow");
            }
            if phase != Phase::A
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
pub fn kind_of(fields: &[String]) -> String {
    fields
        .iter()
        .map(|f| f.split(' ').next().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("+")
}
pub fn replay_if(name: &str, lines: &[Line], report: &mut Report) {
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
pub const DMA_PHASES: [(char, Phase); 8] = [
    ('a', Phase::A),
    ('b', Phase::B),
    ('c', Phase::C),
    ('d', Phase::D),
    ('e', Phase::E),
    ('f', Phase::F),
    ('g', Phase::G),
    ('h', Phase::H),
];
pub fn replay_dma(name: &str, lines: &[Line], report: &mut Report) {
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
