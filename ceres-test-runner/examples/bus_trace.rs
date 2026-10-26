//! Traces the CPU's memory accesses of a DMG from power-on, with the boot
//! ROM, in the format the readable VHDL model of the chip writes
//! (`hdl/board/gameboy_tb.vhd`, `BUS_TRACE`), so that the two can be
//! compared line by line (`hdl/compare-ceres.sh`):
//!
//! ```text
//! 1234 R FF44 90
//! 1235 W C000 3E
//! ```
//!
//! A line per access: the M-cycle it happened in, counted from the CPU's
//! first access, whether it read or wrote, the address and the byte. M-cycles
//! without an access (internal ones, HALT) leave no line.
//!
//! The chip makes at most one access per M-cycle, but Ceres's HALT reads PC
//! once without spending time on it: of two accesses in one M-cycle, the
//! trace keeps the last, and the number dropped goes to stderr.
//!
//! `--frames FILE` also writes every frame the LCD presents, as the model
//! does: `frame N`, then 144 lines of 160 digits, the shades 0 (white) to 3
//! (black). A frame is taken when STAT shows mode 1, which is when ceres
//! presents it (a few dots after LY reaches 144). The first frame after the
//! LCD is turned on is left out: the LCD does not show it, and the model does
//! not write it.
//!
//! `--pixels FILE` also writes what the pixel pipeline did, a line each:
//! `<line> P <x> <t>` for a pixel pushed to the LCD (column x) and
//! `<line> T <w|b> <t>` for a tile fetch begun (the window's or the
//! background's) and `<line> W <40|42|43|47|4B> <value> <t>` for a write to LCDC,
//! SCY, SCX, BGP or WX, with t the half dots since the line began on the
//! pipeline's clock; the model's testbench writes the same (`PIXELS`), for
//! `hdl/compare-pixels.sh`.
//!
//! `--cuts FILE` also writes what the blocks of the chip show each other, a
//! line per change: `<M-cycle> <name> <value>`, with IF (its five bits),
//! TIMA, DMA (1 while an OAM DMA copies), LY and MODE (STAT's mode bits) as
//! a read would see them, OAMB and VRAMB (1 while a CPU read of OAM, or of
//! VRAM, would give 0xFF), and DIV when the divider does not count on (a
//! write to DIV, STOP), with its value in M-cycles (the counter shifted
//! right by 2); the model's testbench writes the same (`CUTS`), for
//! `hdl/compare-cuts.sh`. Ceres runs its time in steps, sometimes of more
//! than an M-cycle (an instruction's internal ones), and sees the state at
//! the end of each: a change in a longer step gets a fourth field, the
//! step's first M-cycle, where it may have happened as well.
//!
//! `--bootrom FILE` runs another boot ROM, such as dmg-sim's quickboot, which
//! skips the logo but leaves the machine as the real one does.
//!
//! `--model MODEL` runs another Game Boy than the DMG-B (`dmg-b`): `cgb-c` or
//! `cgb-e`, for the CGB's model (branch `hdl_cgb`).
//!
//! Usage: `cargo run -p ceres-test-runner --example bus_trace -- <rom>
//! [--bootrom FILE] [--model MODEL] [--mcycles N] [--out FILE] [--frames FILE]
//! [--pixels FILE] [--cuts FILE]`

use anyhow::{Context as _, Result, bail};
use ceres_core::{
    ColorCorrectionMode, CutSample, CutState, Gb, GbBuilder, IoAccess, MemoryAccess, Model,
    PX_HEIGHT, PX_WIDTH, PpuEventKind,
};
use ceres_test_runner::{DummyAudioCallback, load_test_rom};
use std::{
    fs::File,
    io::{self, BufWriter, Write},
    path::{Path, PathBuf},
};

const USAGE: &str = "\
usage: bus_trace <rom> [--bootrom FILE] [--model MODEL] [--mcycles N] [--out FILE] [--frames FILE]
                 [--pixels FILE] [--cuts FILE]

<rom> is a path, or one under external/test-roms.
--mcycles: M-cycles to trace from the first access (1048576, a second, by
default).
--model: dmg-b (by default), cgb-c or cgb-e.";

/// 8 MHz units per half M-cycle of the normal speed: an M-cycle in double
/// speed.
const UNITS_PER_HALF: u64 = 4;

/// The name of the half M-cycle `half` (counted from the CPU's first access),
/// as the model's testbench writes it: whole M-cycles of the normal speed,
/// and ".5" for an M-cycle that starts in the second half of one, which
/// double speed has.
fn mcycle_name(half: u64) -> String {
    if half % 2 == 1 {
        format!("{}.5", half / 2)
    } else {
        (half / 2).to_string()
    }
}

/// STAT's mode 1, VBlank: ceres presents the frame as the mode becomes 1.
const STAT: u16 = 0xFF41;
const STAT_MODE_B: u8 = 0x03;
const STAT_MODE_VBLANK: u8 = 1;
const LCDC: u16 = 0xFF40;
const LCDC_ON_B: u8 = 0x80;

/// Reads `path` as given, else under the test ROMs directory.
fn read_rom(path: &Path) -> Result<Vec<u8>> {
    if path.exists() {
        std::fs::read(path).with_context(|| format!("can't read {}", path.display()))
    } else {
        load_test_rom(path)
    }
}

/// Writes a frame as the model does: on a DMG a digit per pixel, the shade,
/// 0 for the lightest of the four grays; on a CGB four hexadecimal digits,
/// the colour's 15 bits (red low), as the CGB's palettes hold it (ceres's
/// colour correction is off).
fn write_frame<W: Write>(out: &mut W, number: u64, rgba: &[u8], cgb: bool) -> io::Result<()> {
    writeln!(out, "frame {number}")?;
    let width = usize::from(PX_WIDTH);
    for row in rgba.chunks_exact(width * 4).take(usize::from(PX_HEIGHT)) {
        let pixels = row.as_chunks::<4>().0.iter();
        if cgb {
            for px in pixels {
                let colour = u16::from(px[0] >> 3)
                    | u16::from(px[1] >> 3) << 5
                    | u16::from(px[2] >> 3) << 10;
                write!(out, "{colour:04X}")?;
            }
        } else {
            let digits: Vec<u8> = pixels.map(|px| b'0' + (0xFF - px[0]) / 0x55).collect();
            out.write_all(&digits)?;
        }
        out.write_all(b"\n")?;
    }
    Ok(())
}

/// Writes the fields of `state` that differ from `last` (all of them if
/// there is none), as changed in the half M-cycles `since` to `half`. The
/// divider is written when it did not count on from `last`'s value, `units`
/// 8 MiHz units before: a T-cycle is two of them, one in double speed (and a
/// sample does not say which).
fn write_cuts<W: Write>(
    out: &mut W,
    (since, half): (u64, u64),
    state: CutState,
    last: Option<&CutSample>,
    units: u64,
) -> io::Result<()> {
    let old = last.map(|last| last.state);
    let changed = |f: fn(&CutState) -> u8| old.is_none_or(|old| f(&old) != f(&state));
    let mcycle = mcycle_name(half);
    let from = if since < half {
        format!(" {}", mcycle_name(since))
    } else {
        String::new()
    };
    if changed(|s| s.interrupt_flags) {
        writeln!(out, "{mcycle} IF {:02X}{from}", state.interrupt_flags)?;
    }
    if changed(|s| s.tima) {
        writeln!(out, "{mcycle} TIMA {:02X}{from}", state.tima)?;
    }
    if changed(|s| u8::from(s.dma)) {
        writeln!(out, "{mcycle} DMA {}{from}", u8::from(state.dma))?;
    }
    if changed(|s| s.ly) {
        writeln!(out, "{mcycle} LY {:02X}{from}", state.ly)?;
    }
    if changed(|s| s.stat_mode) {
        writeln!(out, "{mcycle} MODE {}{from}", state.stat_mode)?;
    }
    if changed(|s| u8::from(s.oam_blocked)) {
        writeln!(out, "{mcycle} OAMB {}{from}", u8::from(state.oam_blocked))?;
    }
    if changed(|s| u8::from(s.vram_blocked)) {
        writeln!(out, "{mcycle} VRAMB {}{from}", u8::from(state.vram_blocked))?;
    }
    let counted_by = |cycles: u64| {
        old.is_some_and(|old| {
            old.div
                .wrapping_add(u16::try_from(cycles & 0xFFFF).unwrap_or_default())
                == state.div
        })
    };
    let counted = counted_by(units / 2) || counted_by(units);
    if !counted {
        writeln!(out, "{mcycle} DIV {:04X}{from}", state.div >> 2)?;
    }
    Ok(())
}

/// Writes the cut samples taken since the last call, from the M-cycle of the
/// CPU's first access (`first`, in 8 MHz units) on. `last` is the last one
/// written.
fn write_cut_samples<W: Write>(
    out: &mut W,
    samples: Vec<CutSample>,
    first: Option<u64>,
    last: &mut Option<CutSample>,
) -> io::Result<()> {
    for sample in samples {
        // A sample comes at the end of the time it covers: it belongs to the
        // half M-cycle of its last unit.
        let Some(first) = first.filter(|&first| sample.clock_units > first) else {
            continue;
        };
        let half = (sample.clock_units - 1 - first) / UNITS_PER_HALF;
        let since = sample.since_units.saturating_sub(first) / UNITS_PER_HALF;
        let units = last.map_or(0, |last| sample.clock_units - last.clock_units);
        write_cuts(out, (since, half), sample.state, last.as_ref(), units)?;
        *last = Some(sample);
    }
    Ok(())
}

/// The Game Boy `model` with the cartridge `rom`, from power-on, with the
/// boot ROM `bootrom` (or ceres's own), and without colour correction.
fn power_on(rom: &Path, bootrom: Option<&Path>, model: Model) -> Result<Gb<DummyAudioCallback>> {
    let rom = read_rom(rom)?;
    let mut builder = GbBuilder::new(48000, DummyAudioCallback)
        .with_model(model)
        .with_run_bootrom(true)
        .with_rom(rom.into_boxed_slice())
        .context("can't load the ROM")?;
    if let Some(path) = bootrom {
        let data = std::fs::read(path).with_context(|| format!("can't read {}", path.display()))?;
        builder = builder.with_bootrom(data.into_boxed_slice());
    }
    let mut gb = builder.build();
    gb.set_color_correction_mode(ColorCorrectionMode::Disabled);
    Ok(gb)
}

fn trace<W: Write>(
    mut gb: Gb<DummyAudioCallback>,
    halves: u64,
    mut out: W,
    mut frames: Option<BufWriter<File>>,
    mut pixels: Option<BufWriter<File>>,
    mut cuts: Option<BufWriter<File>>,
) -> Result<()> {
    let cgb = gb.model().is_cgb_hardware();
    gb.set_bus_log(true);
    gb.set_ppu_trace(pixels.is_some());
    gb.set_cut_trace(cuts.is_some());

    let mut first = None;
    // The last access, written once the next one is in another M-cycle.
    let mut last: Option<(u64, IoAccess)> = None;
    let mut dropped = 0_u64;
    let mut frame = 0_u64;
    let mut in_vblank = false;
    let mut lcd_on = false;
    let mut unshown = true;
    // The last state written to `cuts`.
    let mut last_cut: Option<CutSample> = None;
    'run: loop {
        gb.step();
        if let Some(pixels) = pixels.as_mut() {
            for event in gb.take_ppu_trace() {
                let t = 2 * event.line_clock + i32::from(event.second_half);
                match event.kind {
                    PpuEventKind::Pixel { x } => writeln!(pixels, "{} P {x} {t}", event.line)?,
                    PpuEventKind::TileFetch { window } => {
                        let kind = if window { 'w' } else { 'b' };
                        writeln!(pixels, "{} T {kind} {t}", event.line)?;
                    }
                    PpuEventKind::Write { addr, value }
                        if matches!(addr, 0xFF40 | 0xFF42 | 0xFF43 | 0xFF47 | 0xFF4B) =>
                    {
                        writeln!(
                            pixels,
                            "{} W {:02X} {value:02X} {t}",
                            event.line,
                            addr & 0xFF
                        )?;
                    }
                    _ => {}
                }
            }
        }
        if let Some(frames) = frames.as_mut() {
            // A step is a fraction of a line: the frame is still the one
            // presented when the mode became 1. (LY is not enough: it
            // becomes 144 a few dots before, and a step can end between.)
            let on = gb.peek_mem(LCDC) & LCDC_ON_B != 0;
            if on && !lcd_on {
                unshown = true;
            }
            lcd_on = on;
            let vblank = gb.peek_mem(STAT) & STAT_MODE_B == STAT_MODE_VBLANK;
            if vblank && !in_vblank {
                if unshown {
                    unshown = false;
                } else {
                    write_frame(frames, frame, gb.pixel_data_rgba(), cgb)?;
                    frame += 1;
                }
            }
            in_vblank = vblank;
        }
        for access in gb.take_io_log() {
            let first = *first.get_or_insert(access.clock_units);
            // Rounded to the nearest half M-cycle: a speed switch can leave the
            // CPU's M-cycles off the grid by an 8 MiHz unit.
            let half = (access.clock_units - first + UNITS_PER_HALF / 2) / UNITS_PER_HALF;
            match last {
                Some((last_half, _)) if last_half == half => dropped += 1,
                Some((last_half, last_access)) => {
                    write_access(&mut out, last_half, &last_access)?;
                }
                None => {}
            }
            if half >= halves {
                break 'run;
            }
            last = Some((half, access));
        }
        if let Some(cuts) = cuts.as_mut() {
            write_cut_samples(cuts, gb.take_cut_trace(), first, &mut last_cut)?;
        }
        // The CPU can stop using the bus for good (the older Mooneye tests
        // end on opcode ED, which locks it up): stop by the clock as well.
        if first.is_some_and(|first| (gb.clock_units() - first) / UNITS_PER_HALF >= halves) {
            if let Some((last_half, last_access)) = last {
                write_access(&mut out, last_half, &last_access)?;
            }
            break;
        }
    }
    out.flush()?;
    for file in [cuts.as_mut(), frames.as_mut(), pixels.as_mut()]
        .into_iter()
        .flatten()
    {
        file.flush()?;
    }
    if dropped > 0 {
        eprintln!("{dropped} accesses dropped (two in one M-cycle)");
    }
    Ok(())
}

fn write_access<W: Write>(out: &mut W, half: u64, access: &IoAccess) -> io::Result<()> {
    let kind = if access.access == MemoryAccess::Write {
        'W'
    } else {
        'R'
    };
    writeln!(
        out,
        "{} {kind} {:04X} {:02X}",
        mcycle_name(half),
        access.addr,
        access.value
    )
}

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let mut rom = None;
    let mut mcycles = 1_048_576.0;
    let mut out = None;
    let mut bootrom = None;
    let mut model = Model::DmgB;
    let mut frames = None;
    let mut pixels = None;
    let mut cuts = None;
    while let Some(arg) = args.next() {
        let mut value = |name: &str| {
            args.next()
                .with_context(|| format!("{name} needs a value\n\n{USAGE}"))
        };
        match arg.as_str() {
            "--mcycles" => mcycles = value("--mcycles")?.parse::<f64>()?,
            "--out" => out = Some(PathBuf::from(value("--out")?)),
            "--frames" => frames = Some(PathBuf::from(value("--frames")?)),
            "--pixels" => pixels = Some(PathBuf::from(value("--pixels")?)),
            "--cuts" => cuts = Some(PathBuf::from(value("--cuts")?)),
            "--bootrom" => bootrom = Some(PathBuf::from(value("--bootrom")?)),
            "--model" => {
                model = match value("--model")?.as_str() {
                    "dmg-b" => Model::DmgB,
                    "cgb-c" => Model::CgbC,
                    "cgb-e" => Model::CgbE,
                    other => bail!("unknown model {other}\n\n{USAGE}"),
                }
            }
            "--help" | "-h" => {
                println!("{USAGE}");
                return Ok(());
            }
            _ if arg.starts_with("--") => bail!("unknown option {arg}\n\n{USAGE}"),
            _ => rom = Some(PathBuf::from(arg)),
        }
    }
    let rom = rom.with_context(|| format!("which ROM?\n\n{USAGE}"))?;
    let create = |path: PathBuf| {
        File::create(&path)
            .map(BufWriter::new)
            .with_context(|| format!("can't create {}", path.display()))
    };
    let frames = frames.map(create).transpose()?;
    let pixels = pixels.map(create).transpose()?;
    let cuts = cuts.map(create).transpose()?;
    // The cut-off, in half M-cycles (the model's last M-cycle can be one).
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a count of M-cycles"
    )]
    let halves = (mcycles * 2.0).round() as u64;
    let gb = power_on(&rom, bootrom.as_deref(), model)?;
    let traced = match out {
        Some(path) => {
            let file =
                File::create(&path).with_context(|| format!("can't create {}", path.display()))?;
            trace(gb, halves, BufWriter::new(file), frames, pixels, cuts)
        }
        None => trace(
            gb,
            halves,
            BufWriter::new(io::stdout().lock()),
            frames,
            pixels,
            cuts,
        ),
    };
    // A reader that has seen enough (`| head`) isn't an error.
    match traced {
        Err(e)
            if e.downcast_ref::<io::Error>()
                .is_some_and(|e| e.kind() == io::ErrorKind::BrokenPipe) =>
        {
            Ok(())
        }
        traced => traced,
    }
}
