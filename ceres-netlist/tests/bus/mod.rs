//! The replay of the bus side (`NetBus`, with the board's `NetBoard`)
//! through the traces that have its fields: the 11 after OAM's and VRAM's
//! (see `BLOCKS` in `hdl/board/gameboy_tb.vhd`). The board's cartridge is
//! the ROM that the trace's `.rom` file names, the boot ROM the model's
//! fast one (`hdl/build/fastboot.bin`).

use {
    super::{Line, Report, bit, hex8},
    ceres_netlist::bus::{BusInputs, BusOutputs, BusPins, NetBoard, NetBus},
    std::{fs, path::Path},
};

/// The fields of a line that has the bus side's.
pub const BUS_LINE_FIELDS: usize = 91;

/// The bus side in a line of a trace.
#[expect(clippy::struct_excessive_bools, reason = "the trace's levels")]
pub struct BusLine {
    pub mreq: bool,
    pub read: bool,
    pub write: bool,
    pub boot_sel: bool,
    pub pins: BusPins,
    pub net: NetBus,
    pub p10_13: u8,
    pub sin: bool,
    pub sck: bool,
    /// The address latches were open in the phase, however briefly.
    pub latch_seen: bool,
}

/// The bus side's fields, after OAM's and VRAM's (`d`, the data pins, is
/// among those).
pub fn parse(fields: &[&str], d: u8) -> Option<BusLine> {
    let &[
        bits,
        a,
        latched_addr,
        latched_data,
        p1,
        jp_read,
        joypad,
        p10_13,
        sb,
        serial,
        count,
        latch_seen,
    ] = fields
    else {
        return None;
    };
    let mut net = NetBus::power_on();
    net.unmapped = bit(bits, 6)?;
    net.latched_addr = u16::from_str_radix(latched_addr, 16).ok()?;
    net.latched_data = hex8(latched_data)?;
    net.p1 = hex8(p1)?;
    net.jp_read = hex8(jp_read)?;
    net.jp_history = hex8(joypad.get(..1)?)?;
    net.jp_wakeup = bit(joypad, 1)?;
    net.sb = hex8(sb)?;
    net.transfer = bit(serial, 0)?;
    net.internal_clock = bit(serial, 1)?;
    net.half = Some(bit(serial, 2)?);
    net.ser_out = bit(serial, 3)?;
    net.count = hex8(count)?;
    Some(BusLine {
        mreq: bit(bits, 0)?,
        read: bit(bits, 1)?,
        write: bit(bits, 2)?,
        boot_sel: bit(bits, 7)?,
        pins: BusPins {
            a: u16::from_str_radix(a, 16).ok()?,
            d,
            rd_n: bit(bits, 3)?,
            wr_n: bit(bits, 4)?,
            cs_n: bit(bits, 5)?,
        },
        net,
        p10_13: hex8(p10_13)?,
        sin: bit(serial, 5)?,
        sck: bit(serial, 4)?,
        latch_seen: bit(latch_seen, 0)?,
    })
}

/// What the bus side takes in during `line`'s phase.
fn inputs(line: &Line) -> Option<BusInputs> {
    let (bus, clocks) = (line.bus.as_ref()?, line.clocks.as_ref()?);
    let pins = clocks.inputs;
    let phase = line.phase;
    let from_vram = line.dma.source >> 5 == 0b100;
    Some(BusInputs {
        addr: line.addr,
        read: bus.read,
        write: bus.write,
        mreq: bus.mreq,
        data: line.data,
        // @hw die: BAPY..BYLY; BUVU BYXO BEDO; AFAS AREV APOV; AGUT AWOD
        // ABUZ; BELE BYJU BALY BOGA
        clk_t4: pins.clk_ena && matches!(phase, 'g' | 'h'),
        exec_phase: pins.clk_ena && phase != 'a',
        cpu_wr_sync: pins.write && matches!(phase, 'e' | 'f' | 'g'),
        cpu_ext_cs_en: (pins.t1 && !pins.t2) || (pins.mreq && !matches!(phase, 'a' | 'b')),
        clk_1mhz: phase == 'a' || !pins.osc_ena,
        starts_a: phase == 'a',
        reset2: clocks.net.reset2(),
        reset: !pins.reset_pin_n,
        dma_addr_ext: line.dma.run && !from_vram,
        dma_addr: u16::from_be_bytes([line.dma.source, line.dma.count]),
        div: line.div,
        p10_13: bus.p10_13,
        sin: bus.sin,
        sck: bus.sck,
        latch_glitch: latch_glitch(line),
    })
}

/// The flip-flops and latches of `net` that differ from `model`'s.
fn differences(net: &NetBus, model: &NetBus) -> Vec<&'static str> {
    let mut differ = Vec::new();
    for (name, differs) in [
        ("unmapped", net.unmapped != model.unmapped),
        ("latched_addr", net.latched_addr != model.latched_addr),
        (
            "latched_data",
            // Taken from floating pins: not compared (see `replay`).
            !net.latched_floating && net.latched_data != model.latched_data,
        ),
        ("p1", net.p1 != model.p1),
        ("jp_read", net.jp_read != model.jp_read),
        ("jp_history", net.jp_history != model.jp_history),
        ("jp_wakeup", net.jp_wakeup != model.jp_wakeup),
        ("sb", net.sb != model.sb),
        ("transfer", net.transfer != model.transfer),
        ("internal_clock", net.internal_clock != model.internal_clock),
        (
            "half",
            net.half.unwrap_or(false) != model.half.unwrap_or(false),
        ),
        ("count", net.count != model.count),
        ("ser_out", net.ser_out != model.ser_out),
    ] {
        if differs {
            differ.push(name);
        }
    }
    differ
}

/// Takes the model's flip-flops and latches, keeping HRAM and the edges'
/// memory.
const fn resync(net: &mut NetBus, model: &NetBus) {
    *net = NetBus {
        high_ram: net.high_ram,
        before: net.before,
        // The trace shows an unknown half clock as 0.
        half: net.half,
        ..*model
    };
}

/// Replays the bus side, phase by phase, from the trace's start (the
/// power-on's RAM), with the board running the trace's ROM. While nothing
/// drives the data pins, the board's keeper holds them against the pads'
/// pull-ups (its low bits resolve to 'W', and the trace prints a byte with
/// one as 00), and which byte it holds depends on the core's request
/// pulsing for a moment as accesses start: the pins and the data latches
/// are compared only while driven.
pub fn replay(name: &str, trace: &Path, lines: &[Line], report: &mut Report) {
    let Some(first) = lines.first().and_then(|l| l.bus.as_ref()) else {
        return;
    };
    let Some((mut board, boot_rom)) = board(trace, report) else {
        return;
    };
    let mut net = start(first, &lines[0]);
    for (n, line) in lines.iter().enumerate().skip(1) {
        let Some(i) = inputs(line) else {
            return;
        };
        let out = net.phase(&i, &mut board, &boot_rom);
        report.phases += 1;
        let place = format!("{name}:{}", n + 1);
        for (what, happened) in [
            (
                "bus: a write of the work RAM",
                !net.before.pins.wr_n && i.addr >> 13 == 6,
            ),
            (
                "bus: a read on the cartridge bus",
                i.read && out.non_vram_mreq && line.phase == 'h',
            ),
            (
                "bus: a write of HRAM",
                i.cpu_wr_sync && i.addr >= 0xFF80 && i.addr != 0xFFFF,
            ),
            (
                "bus: a serial transfer ends",
                out.int_serial && line.phase == 'a',
            ),
        ] {
            if happened {
                report.saw(what);
            }
        }
        compare(&mut net, &out, line, "bus", &place, report);
    }
}

/// The board at power-on with the trace's ROM, and the boot ROM's image.
pub fn board(trace: &Path, report: &mut Report) -> Option<(NetBoard, Vec<u8>)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let (Ok(rom_path), Ok(boot_rom)) = (
        fs::read_to_string(trace.with_extension("rom")),
        fs::read(root.join("hdl/build/fastboot.bin")),
    ) else {
        report.saw("bus: a trace without its ROM (not replayed)");
        return None;
    };
    let Ok(rom) = fs::read(root.join(rom_path.trim())) else {
        report.saw("bus: a ROM that is not there (not replayed)");
        return None;
    };
    Some((NetBoard::power_on(rom), boot_rom))
}

/// The bus side as `first` (the trace's first line, `line`) has it, the
/// half clock unknown.
pub const fn start(first: &BusLine, line: &Line) -> NetBus {
    let mut net = NetBus {
        half: None,
        ..first.net
    };
    net.before.pins = first.pins;
    net.before.data = line.data;
    net
}

/// The bus side after a phase (`out`, its outputs) against the model at
/// the end of `line`'s phase; after a difference it takes the model's.
pub fn compare(
    net: &mut NetBus,
    out: &BusOutputs,
    line: &Line,
    who: &str,
    place: &str,
    report: &mut Report,
) {
    let Some(model) = line.bus.as_ref() else {
        return;
    };
    {
        let place = place.to_owned();
        let differ = differences(net, &model.net);
        if !differ.is_empty() {
            report.differ(
                format!("{who}: bus, phase {}: {}", line.phase, differ.join(" ")),
                place.clone(),
            );
            resync(net, &model.net);
        }
        let ours = BusPins {
            d: if out.pins_driven {
                out.pins.d
            } else {
                model.pins.d
            },
            ..out.pins
        };
        if !out.pins_driven {
            report.saw("bus: the data pins float (not compared)");
        }
        if (ours, out.boot_sel) != (model.pins, model.boot_sel) {
            report.differ(
                format!("{who}: bus pins, phase {}", line.phase),
                format!(
                    "{place} ({:04X?} boot_sel {}, model {:04X?} {})",
                    out.pins, out.boot_sel, model.pins, model.boot_sel
                ),
            );
        }
        if let (Some(value), Some(theirs), 'h') = (
            out.data_out,
            line.lcd.as_ref().and_then(|l| l.read),
            line.phase,
        ) {
            report.saw("bus: a read the bus side drives");
            if value != theirs && model.read {
                report.differ(
                    format!("{who}: bus, a read of {:04X}", line.addr & 0xFF00),
                    format!("{place} (reads {value:02X}, model {theirs:02X})"),
                );
            }
        }
    }
}

/// The address the external bus's latches took in `line`'s phase if they
/// opened without a steady request: the core's pulse.
pub fn latch_glitch(line: &Line) -> Option<u16> {
    let bus = line.bus.as_ref()?;
    (bus.latch_seen && !(bus.mreq && line.addr >> 13 != 0b100)).then_some(bus.net.latched_addr)
}
