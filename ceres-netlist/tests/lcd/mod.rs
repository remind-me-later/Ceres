//! The line timing, the OAM scan and OAM's latches, replayed phase by
//! phase.

use super::*;

/// The flip-flops of the line timing that differ from the model's (the
/// LCD's pins' are not traced).
pub fn lcd_differences(net: &NetLcd, model: &NetLcd) -> Vec<&'static str> {
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
pub fn lcd_write(
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
pub fn replay_lcd(name: &str, lines: &[Line], report: &mut Report) {
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
        for now in [
            net.int_stat(before.mode0),
            net.int_stat(before.mode0 || lcd.mode0_pulse),
            net.int_stat(lcd.mode0),
        ] {
            // The last two: mode 0, which rises after the edge's flip-flops,
            // for a moment only if a sprite at X 167 stops mode 3 again.
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
pub fn replay_scan(name: &str, lines: &[Line], report: &mut Report) {
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
pub fn replay_oam_latches(before: &Line, after: &Line, place: &str, report: &mut Report) {
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
