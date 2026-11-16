//! The blocks wired together (`NetChip`), on the trace's clocks or from
//! the power-on on the board's pins.

use super::*;

/// What the chip takes in at the edge of line `edge` (the pins), and in the
/// phase it starts, which the next line ends (the joypad's lines, SIN,
/// SCK).
pub fn chip_inputs(edge: &Line, phase: &Line) -> Option<ChipInputs> {
    let pins = edge.clocks.as_ref()?.inputs;
    let bus = phase.bus.as_ref()?;
    Some(ChipInputs {
        ck1: pins.ck1,
        reset_pin_n: pins.reset_pin_n,
        t1: pins.t1,
        t2: pins.t2,
        test_reset: pins.test_reset,
        p10_13: bus.p10_13,
        sin: bus.sin,
        sck: bus.sck,
    })
}
/// The CPU reads in the phase `line` ends.
pub fn bus_read(line: &Line) -> bool {
    line.bus.as_ref().is_some_and(|b| b.read)
}
/// The chip's blocks wired together (`NetChip`): the clocks run the phases
/// the timer, IF, the DMA and the line timing step on, the divider's last
/// bit ends the power-on wait, the timer's interrupt, the STAT line and
/// VBlank rise into IF, the DMA runs on the clocks' `dma_phi`, LCDC holds
/// the line timing and the scan in reset and gives the scan the sprites'
/// height, the OAM scan runs on the line timing's clocks with what OAM's
/// latches hold, the latches follow the scan, the DMA and render's sprite
/// fetch, render and the sprite store run on what the others give them,
/// and give the STAT line mode 0, the APU runs on the clocks and the
/// divider, and the address pins' latches catch the core's request where
/// it pulses as an access starts; only the CPU's access (with what drives
/// its address's high byte and its decoder's request, from the core's
/// replay), the other sources, the acknowledges, OAM's RAM and odd byte
/// and VRAM's data come from the trace, which must have render's, the
/// core's and the APU's fields. Every read is compared with the
/// byte the core takes. LCDC's bits that the trace lacks (all but 7 and 2)
/// are taken from the first read of it, if it comes before a write. A step
/// is an edge and the phase it starts: the clocks are compared with the
/// edge's line, the others with the next, which ends the phase.
pub fn replay_chip(name: &str, trace: &Path, lines: &[Line], report: &mut Report) {
    let Some(start) = lines.iter().position(|l| l.clocks.is_some()) else {
        return;
    };
    let (Some(first), Some(second)) = (lines.get(start), lines.get(start + 1)) else {
        return;
    };
    let (Some(edge), Some(next)) = (first.clocks.as_ref(), second.clocks.as_ref()) else {
        return;
    };
    let (Some(lcd_edge), Some(lcd)) = (first.lcd.as_ref(), second.lcd.as_ref()) else {
        return;
    };
    let Some(scan) = lcd.scan.as_ref() else {
        return;
    };
    let Some(model_render) = second.render.as_ref().filter(|r| r.oam_vram.is_some()) else {
        return;
    };
    let Some(model_bus) = second.bus.as_ref() else {
        return;
    };
    let (Some(model_apu), Some(apu_inputs)) = (second.apu.as_ref(), apu::inputs(first, second))
    else {
        return;
    };
    let (Some(model_core), Some(core_inputs)) = (second.core.as_ref(), core::inputs(second)) else {
        return;
    };
    let mut net_core = model_core.net;
    let core_out = net_core.settle(&core_inputs);
    let Some((board, boot_rom)) = bus::board(trace, report) else {
        return;
    };
    let mut memories = ChipMemories {
        vram: vram_power_on(),
        board,
        boot_rom,
    };
    let mut chip = NetChip {
        clocks: NetClocks {
            cpu_sys_reset: next.net.cpu_sys_reset,
            ..edge.net
        },
        timer: net_timer(second),
        interrupts: NetIf {
            flags: second.flags,
            ie: second.ie,
        },
        dma: second.dma,
        dma_phi: false,
        // LCDC's bits that the trace has: the rest are known after a write.
        control: NetLcdControl {
            lcdc: if lcd.ppu_reset { 0 } else { 0x80 } | if scan.tall { 0x04 } else { 0 },
        },
        lcd: NetLcd {
            clk: lcd_edge.clk,
            ..lcd.net
        },
        scan: scan.net,
        wr_before: second.wr,
        data_before: second.data,
        core: net_core,
        core_out,
        core_low_before: 0,
        blocks_low_before: 0,
        blocks_high_before: 0,
        read_bus: None,
        if_latch: second.read_value,
        render: NetRender {
            md_kept: 0xFF,
            oam: render::oam_from(second).unwrap_or_default(),
            ..model_render.net
        },
        render_pins: PinLog::default(),
        bus: bus::start(model_bus, second),
        apu: apu::start_from(model_apu, apu_inputs),
        cpu_wr: false,
        int_serial: model_bus.net.count & 8 != 0,
        int_jp: model_bus.net.jp_history & 9 == 9,
    };
    chip.dma_phi = NetChip::dma_phi(chip.clocks.outputs(edge.inputs));
    // Render's wires settle from the model's state.
    let Some(at_start) = render::inputs(first, second, chip.control.lcdc) else {
        return;
    };
    chip.render.inputs = at_start;
    chip.render.settle_wires(&memories.vram);
    run_chip(
        name,
        lines,
        start + 1,
        chip,
        &mut memories,
        Start::Trace,
        report,
    );
}
/// A trace from the power-on (`FROM=power-on`) through `NetChip` from its
/// power-on state, on the board's pins (`ChipInputs::board`): nothing
/// comes from the trace, which only checks it, the board's pins too. Its
/// first line is the model at time 0, as its initial values have it,
/// before the crystal's first edge.
pub fn run_chip_from_power_on(name: &str, trace: &Path, lines: &[Line], report: &mut Report) {
    let (Some(time_0), Some(after)) = (lines.first(), lines.get(1)) else {
        return;
    };
    let (true, Some(pins)) = (time_0.apu.is_some(), chip_inputs(time_0, after)) else {
        return;
    };
    if pins != ChipInputs::board(0) {
        report.differ(
            "chip: the board's pins".into(),
            format!("{name}:1 ({:?}, model {pins:?})", ChipInputs::board(0)),
        );
    }
    let Some((board, boot_rom)) = bus::board(trace, report) else {
        return;
    };
    let mut memories = ChipMemories {
        vram: vram_power_on(),
        board,
        boot_rom,
    };
    run_chip(
        name,
        lines,
        1,
        NetChip::power_on(ChipInputs::board(0), &memories.vram),
        &mut memories,
        Start::PowerOn,
        report,
    );
}
/// Where `run_chip` starts the chip.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Start {
    /// From a line of the trace, on the trace's pins; LCDC's bits that the
    /// trace lacks are taken from the first read of it, if it comes before
    /// a write.
    Trace,
    /// From the power-on, on the board's pins.
    PowerOn,
}
/// `chip` from the start of line `first`'s phase to the trace's end, each
/// block compared with the model at the end of every phase, and the model's
/// taken after a difference.
#[expect(clippy::too_many_lines, reason = "the comparison of each block")]
pub fn run_chip(
    name: &str,
    lines: &[Line],
    first: usize,
    mut chip: NetChip,
    memories: &mut ChipMemories,
    start: Start,
    report: &mut Report,
) {
    let mut lcdc_known = start == Start::PowerOn;
    for n in first..lines.len() - 1 {
        let (line, ends) = (&lines[n], &lines[n + 1]);
        let (Some(traced), Some(clocks)) = (chip_inputs(line, ends), line.clocks.as_ref()) else {
            return;
        };
        let inputs = match start {
            Start::Trace => traced,
            Start::PowerOn => ChipInputs::board(n as u64),
        };
        // The trace has the pins' levels: SCK is the chip's own while its
        // internal clock drives it.
        let traced_by_board = ChipInputs {
            sck: if chip.bus.internal_clock {
                inputs.sck
            } else {
                traced.sck
            },
            ..traced
        };
        if inputs != traced_by_board {
            report.differ(
                "chip: the board's pins".into(),
                format!("{name}:{} ({inputs:?}, model {traced:?})", n + 1),
            );
        }
        let place = format!("{name}:{}", n + 1);
        let before = chip;
        let mut events = chip.edge(inputs, memories);
        report.phases += 1;
        compare_chip_core(
            &mut chip,
            &events,
            ends,
            &format!("{name}:{}", n + 2),
            report,
        );
        // LCDC's bits the trace lacks are known once the CPU writes it, or
        // reads it.
        lcdc_known |= chip.cpu_wr && events.core.addr == 0xFF40;
        if let (false, true, Some(taken)) = (
            lcdc_known,
            events.read.is_some() && events.core.addr == 0xFF40,
            ends.lcd.as_ref().and_then(|l| l.read),
        ) {
            events.read = Some(taken);
            chip.control.lcdc = taken;
            lcdc_known = true;
        }
        if chip.timer.int_timer && !before.timer.int_timer {
            report.saw("chip: the timer's interrupt rises into IF");
        }
        if chip.int_serial && !before.int_serial {
            report.saw("chip: the serial port's interrupt rises into IF");
        }
        if chip.int_jp && !before.int_jp {
            report.saw("chip: the joypad's interrupt rises into IF");
        }
        if chip.dma.enabled && !before.dma.enabled {
            report.saw("chip: a transfer starts on the clocks' dma_phi");
        }

        let (shown, outputs) =
            clocks_shown(before.clocks, chip.clocks, before.clock_inputs(inputs));
        if shown != clocks.net || outputs != clocks.outputs {
            report.differ(
                format!("chip: clocks, phase {}", line.phase),
                format!(
                    "{place} ({shown:?} {outputs:?}, model {:?} {:?})",
                    clocks.net, clocks.outputs
                ),
            );
            chip.clocks = NetClocks {
                cpu_sys_reset: ends
                    .clocks
                    .as_ref()
                    .map_or(clocks.net.cpu_sys_reset, |c| c.net.cpu_sys_reset),
                ..clocks.net
            };
        }
        let fields = compare_timer(&chip.timer, ends);
        if !fields.is_empty() {
            report.differ(
                format!("chip: timer, phase {}: {}", ends.phase, kind_of(&fields)),
                format!("{name}:{} ({})", n + 2, fields.join(", ")),
            );
            chip.timer = net_timer(ends);
        }
        if chip.interrupts.flags != ends.flags {
            report.differ(
                format!("chip: IF, phase {}", ends.phase),
                format!(
                    "{name}:{} ({:02X}, model {:02X})",
                    n + 2,
                    chip.interrupts.flags,
                    ends.flags
                ),
            );
            chip.interrupts.flags = ends.flags;
        }
        if chip.dma != ends.dma {
            report.differ(
                format!("chip: DMA, phase {}", ends.phase),
                format!("{name}:{} ({:?}, model {:?})", n + 2, chip.dma, ends.dma),
            );
            chip.dma = ends.dma;
        }
        compare_chip_ppu(name, n, line, ends, &mut chip, events, report);
        bus::compare(
            &mut chip.bus,
            &events.bus,
            ends,
            "chip",
            &format!("{name}:{}", n + 2),
            report,
        );
        if let Some(model) = ends.apu.as_ref() {
            apu::compare(
                &mut chip.apu,
                model,
                "chip: apu",
                ends.phase,
                &format!("{name}:{}", n + 2),
                report,
            );
        }
        if let Some(model) = ends.render.as_ref() {
            let differ = render::differences(&chip.render, &model.net);
            if !differ.is_empty() {
                report.differ(
                    format!("chip: render, phase {}: {}", ends.phase, differ.join(" ")),
                    format!("{name}:{}", n + 2),
                );
                render::resync(&mut chip.render, &model.net, &memories.vram);
            }
        }
    }
}
/// The core of `chip` after a step against the model at the end of the
/// phase (`ends`): its state, its outputs, and the data bus's bits pulled
/// strongly low that the chip gave it.
pub fn compare_chip_core(
    chip: &mut NetChip,
    events: &ChipEvents,
    ends: &Line,
    place: &str,
    report: &mut Report,
) {
    let Some(model) = ends.core.as_ref() else {
        return;
    };
    if events.data_low != model.data_low {
        report.differ(
            format!(
                "chip: the data bus's low bits, phase {}, {}",
                ends.phase,
                if bus_read(ends) { "a read" } else { "no read" }
            ),
            format!(
                "{place} ({:04X}: {:02X}, model {:02X}, data {:02X})",
                ends.addr, events.data_low, model.data_low, ends.data
            ),
        );
    }
    let outputs = core::output_differences(&events.core, ends);
    if !outputs.is_empty() {
        report.differ(
            format!(
                "chip: core outputs, phase {}: {}",
                ends.phase,
                outputs.join(" ")
            ),
            format!(
                "{place} (opcode {:02X} m{}: {:04X?}, model addr {:04X})",
                model.net.seq.opcode, model.net.seq.mcyc, events.core, ends.addr
            ),
        );
    }
    let state = core::differences(&chip.core, &model.net);
    if !state.is_empty() {
        report.differ(
            format!("chip: core, phase {}: {}", ends.phase, state.join(" ")),
            format!(
                "{place} (opcode {:02X} m{})",
                model.net.seq.opcode, model.net.seq.mcyc
            ),
        );
        core::resync(&mut chip.core, &model.net, ends.phase);
    }
}
/// What `chip`'s LCDC gives the line timing and the scan after the step at
/// `line`'s edge, against the model at the end of the phase (`ends`).
pub fn compare_chip_lcdc(
    line: &Line,
    ends: &Line,
    place: &str,
    chip: &mut NetChip,
    report: &mut Report,
) {
    let (Some(line_lcd), Some(lcd)) = (line.lcd.as_ref(), ends.lcd.as_ref()) else {
        return;
    };
    let Some(scan) = lcd.scan.as_ref() else {
        return;
    };
    let reset2 = ends.clocks.as_ref().is_some_and(|c| c.net.reset2());
    let (ppu_reset, tall) = (chip.control.ppu_reset(reset2), chip.control.tall());
    if ends.wr && ends.addr == 0xFF40 && !line.wr {
        report.saw("chip: a write of LCDC");
    }
    if ppu_reset != line_lcd.ppu_reset {
        report.saw(if ppu_reset {
            "chip: LCDC turns the LCD off"
        } else {
            "chip: LCDC turns the LCD on"
        });
    }
    if ppu_reset != lcd.ppu_reset || tall != scan.tall {
        report.differ(
            format!("chip: LCDC, phase {}", ends.phase),
            format!(
                "{place} (reset {ppu_reset} tall {tall}, model {} {})",
                lcd.ppu_reset, scan.tall
            ),
        );
        chip.control.lcdc = chip.control.lcdc & !0x84
            | if lcd.ppu_reset { 0 } else { 0x80 }
            | if scan.tall { 0x04 } else { 0 };
    }
}
/// The line timing, the OAM scan and OAM's latches of `chip` after the step
/// at line `n`'s edge, against the model at the end of the phase (`ends`).
pub fn compare_chip_ppu(
    name: &str,
    n: usize,
    line: &Line,
    ends: &Line,
    chip: &mut NetChip,
    events: ChipEvents,
    report: &mut Report,
) {
    let (Some(lcd_edge), Some(lcd)) = (line.lcd.as_ref(), ends.lcd.as_ref()) else {
        return;
    };
    let Some(scan) = lcd.scan.as_ref() else {
        return;
    };
    let place = format!("{name}:{}", n + 2);
    if events.stat_rose {
        report.saw("chip: the STAT line rises into IF");
    }
    if events.vblank_rose {
        report.saw("chip: VBlank rises into IF");
    }
    compare_chip_lcdc(line, ends, &place, chip, report);
    let mut model = lcd.net;
    model.clk = lcd_edge.clk;
    let differ = lcd_differences(&chip.lcd, &model);
    if !differ.is_empty() {
        report.differ(
            format!("chip: LCD, phase {}: {}", ends.phase, differ.join(" ")),
            place.clone(),
        );
        let (frame, line_parity, cpg_lx) = (chip.lcd.frame, chip.lcd.line_parity, chip.lcd.cpg_lx);
        chip.lcd = model;
        chip.lcd.frame = frame;
        chip.lcd.line_parity = line_parity;
        chip.lcd.cpg_lx = cpg_lx;
    }
    for (what, ours, theirs) in [
        ("the STAT line's rise", events.stat_rose, ends.rose & 2 != 0),
        ("VBlank's rise", events.vblank_rose, ends.rose & 1 != 0),
        ("mode 2", events.mode2, lcd.mode2),
    ] {
        if ours != theirs {
            report.differ(
                format!("chip: {what}, phase {}", ends.phase),
                format!("{place} ({ours}, model {theirs})"),
            );
        }
    }
    let read_in_h = ends.bus.as_ref().is_some_and(|b| b.read) && ends.phase == 'h';
    if events.read.is_none() && read_in_h {
        report.differ(
            format!(
                "chip: a read nothing answers, of {:04X}",
                ends.addr & 0xFFF0
            ),
            place.clone(),
        );
    }
    if let (Some(value), Some(taken)) = (events.read, lcd.read) {
        report.saw(match ends.addr {
            0xFF04..=0xFF07 => "chip: a read of the divider or the timer",
            0xFF0F => "chip: a read of IF",
            0xFFFF => "chip: a read of IE",
            0xFF10..=0xFF26 => "chip: a read of the APU's registers",
            0xFF27..=0xFF2F => "chip: a read of the APU's unused addresses",
            0xFF30..=0xFF3F => "chip: a read of wave RAM",
            0xFF03 | 0xFF08..=0xFF0E | 0xFF4C..=0xFF4F | 0xFF51..=0xFF7F | 0xFEA0..=0xFEFF => {
                "chip: a read nothing drives"
            }
            0x8000..=0x9FFF => "chip: a read of VRAM",
            0xFE00..=0xFE9F => "chip: a read of OAM",
            0xFF40 => "chip: a read of LCDC",
            0xFF41 | 0xFF44 | 0xFF45 => "chip: a read of STAT, LY or LYC",
            0xFF46 => "chip: a read of FF46",
            0xFF42 | 0xFF43 | 0xFF47..=0xFF49 | 0xFF4A | 0xFF4B => {
                "chip: a read of SCY, SCX, BGP, OBP0, OBP1, WY or WX"
            }
            0xFF80..=0xFFFE => "chip: a read of HRAM",
            0xFF00..=0xFF02 | 0xFF50 => "chip: a read of the joypad, the serial port or FF50",
            0x0000..=0x00FF if ends.bus.as_ref().is_some_and(|b| b.boot_sel) => {
                "chip: a read of the boot ROM"
            }
            _ => "chip: a read on the cartridge bus",
        });
        if value != taken {
            report.differ(
                format!("chip: a read of {:04X}", ends.addr),
                format!("{place} (reads {value:02X}, model {taken:02X})"),
            );
        }
    }
    let y_match = NetScan::y_match(scan.y, lcd.net.ly, scan.tall);
    if events.save != 0 {
        report.saw("chip: a save, with OAM's latches and the line timing's clocks");
    }
    if chip.scan != scan.net || y_match != scan.y_match || events.save != scan.save {
        report.differ(
            format!("chip: scan, phase {}", ends.phase),
            format!(
                "{place} ({:?} save {:03X}, model {:?} save {:03X})",
                chip.scan, events.save, scan.net, scan.save
            ),
        );
        chip.scan = scan.net;
    }
    let buses_known = oam_buses_known(chip, &events, report);
    render::compare_oam(&mut chip.render, ends, buses_known, "chip", &place, report);
}
/// Whether the model knows OAM's buses after the phase. Floating pins are
/// unknown (W) in the model wherever the board's keeper holds a 0 against
/// the pads' pull-ups, and the trace writes a byte with an unknown bit as
/// 00. OAM's buses take the pins in an OAM DMA from the cartridge's bus, and
/// through the data latches in a read of it.
pub fn oam_buses_known(chip: &NetChip, events: &ChipEvents, report: &mut Report) -> bool {
    let r = &chip.render.inputs;
    let from_pins = r.dma_addr_ext || (r.rd && r.non_vram_mreq && r.clk_t4);
    let known = !from_pins || events.bus.pins_driven || events.bus.pins.d == 0xFF;
    if !known {
        report.saw("chip: OAM's buses take floating pins (not compared)");
    }
    known
}
