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
//! (black). A frame is taken when LY reaches 144, which is when ceres
//! presents it. The first frame after the LCD is turned on is left out: the
//! LCD does not show it, and the model does not write it.
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
//! Usage: `cargo run -p ceres-test-runner --example bus_trace -- <rom>
//! [--bootrom FILE] [--mcycles N] [--out FILE] [--frames FILE] [--pixels FILE]
//! [--cuts FILE]`

use anyhow::{Context as _, Result, bail};
use ceres_core::{
    CutSample, CutState, GbBuilder, IoAccess, MemoryAccess, Model, PX_HEIGHT, PX_WIDTH,
    PpuEventKind,
};
use ceres_test_runner::{DummyAudioCallback, load_test_rom};
use std::{
    fs::File,
    io::{self, BufWriter, Write},
    path::{Path, PathBuf},
};

const USAGE: &str = "\
usage: bus_trace <rom> [--bootrom FILE] [--mcycles N] [--out FILE] [--frames FILE] [--pixels FILE]
                 [--cuts FILE]

<rom> is a path, or one under external/test-roms.
--mcycles: M-cycles to trace from the first access (1048576, a second, by
default).";

/// 8 MHz units per M-cycle at single speed.
const UNITS_PER_MCYCLE: u64 = 8;

/// The register that holds the line being drawn, and the first line of
/// VBlank, where ceres presents the frame.
const LY: u16 = 0xFF44;
const LCDC: u16 = 0xFF40;
const LCDC_ON_B: u8 = 0x80;
const FIRST_VBLANK_LINE: u8 = 144;

/// Reads `path` as given, else under the test ROMs directory.
fn read_rom(path: &Path) -> Result<Vec<u8>> {
    if path.exists() {
        std::fs::read(path).with_context(|| format!("can't read {}", path.display()))
    } else {
        load_test_rom(path)
    }
}

/// Writes a frame as the model does, a digit per pixel: the shade, 0 for the
/// lightest of the DMG's four grays.
fn write_frame<W: Write>(out: &mut W, number: u64, rgba: &[u8]) -> io::Result<()> {
    writeln!(out, "frame {number}")?;
    let width = usize::from(PX_WIDTH);
    for row in rgba.chunks_exact(width * 4).take(usize::from(PX_HEIGHT)) {
        let digits: Vec<u8> = row
            .as_chunks::<4>()
            .0
            .iter()
            .map(|px| b'0' + (0xFF - px[0]) / 0x55)
            .collect();
        out.write_all(&digits)?;
        out.write_all(b"\n")?;
    }
    Ok(())
}

/// Writes the fields of `state` that differ from `last` (all of them if
/// there is none), as changed in the M-cycles `since` to `mcycle`. The
/// divider is written when it did not count on from `last`'s value,
/// `cycles` T-cycles before.
fn write_cuts<W: Write>(
    out: &mut W,
    (since, mcycle): (u64, u64),
    state: CutState,
    last: Option<&CutSample>,
    cycles: u64,
) -> io::Result<()> {
    let old = last.map(|last| last.state);
    let changed = |f: fn(&CutState) -> u8| old.is_none_or(|old| f(&old) != f(&state));
    let from = if since < mcycle {
        format!(" {since}")
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
    let counted = old.is_some_and(|old| {
        old.div
            .wrapping_add(u16::try_from(cycles & 0xFFFF).unwrap_or_default())
            == state.div
    });
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
        // M-cycle of its last unit.
        let Some(first) = first.filter(|&first| sample.clock_units > first) else {
            continue;
        };
        let mcycle = (sample.clock_units - 1 - first) / UNITS_PER_MCYCLE;
        let since = sample.since_units.saturating_sub(first) / UNITS_PER_MCYCLE;
        let cycles = last.map_or(0, |last| (sample.clock_units - last.clock_units) / 2);
        write_cuts(out, (since, mcycle), sample.state, last.as_ref(), cycles)?;
        *last = Some(sample);
    }
    Ok(())
}

fn trace<W: Write>(
    rom: &Path,
    bootrom: Option<&Path>,
    mcycles: u64,
    mut out: W,
    mut frames: Option<BufWriter<File>>,
    mut pixels: Option<BufWriter<File>>,
    mut cuts: Option<BufWriter<File>>,
) -> Result<()> {
    let rom = read_rom(rom)?;
    let mut builder = GbBuilder::new(48000, DummyAudioCallback)
        .with_model(Model::DmgB)
        .with_run_bootrom(true)
        .with_rom(rom.into_boxed_slice())
        .context("can't load the ROM")?;
    if let Some(path) = bootrom {
        let data = std::fs::read(path).with_context(|| format!("can't read {}", path.display()))?;
        builder = builder.with_bootrom(data.into_boxed_slice());
    }
    let mut gb = builder.build();
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
    loop {
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
            // presented when LY reached 144.
            let on = gb.peek_mem(LCDC) & LCDC_ON_B != 0;
            if on && !lcd_on {
                unshown = true;
            }
            lcd_on = on;
            let vblank = gb.peek_mem(LY) >= FIRST_VBLANK_LINE;
            if vblank && !in_vblank {
                if unshown {
                    unshown = false;
                } else {
                    write_frame(frames, frame, gb.pixel_data_rgba())?;
                    frame += 1;
                }
            }
            in_vblank = vblank;
        }
        for access in gb.take_io_log() {
            let first = *first.get_or_insert(access.clock_units);
            let mcycle = (access.clock_units - first) / UNITS_PER_MCYCLE;
            match last {
                Some((last_mcycle, _)) if last_mcycle == mcycle => dropped += 1,
                Some((last_mcycle, last_access)) => {
                    write_access(&mut out, last_mcycle, &last_access)?;
                }
                None => {}
            }
            if mcycle >= mcycles {
                out.flush()?;
                if let Some(cuts) = cuts.as_mut() {
                    cuts.flush()?;
                }
                if let Some(frames) = frames.as_mut() {
                    frames.flush()?;
                }
                if let Some(pixels) = pixels.as_mut() {
                    pixels.flush()?;
                }
                if dropped > 0 {
                    eprintln!("{dropped} accesses dropped (two in one M-cycle)");
                }
                return Ok(());
            }
            last = Some((mcycle, access));
        }
        if let Some(cuts) = cuts.as_mut() {
            write_cut_samples(cuts, gb.take_cut_trace(), first, &mut last_cut)?;
        }
    }
}

fn write_access<W: Write>(out: &mut W, mcycle: u64, access: &IoAccess) -> io::Result<()> {
    let kind = if access.access == MemoryAccess::Write {
        'W'
    } else {
        'R'
    };
    writeln!(
        out,
        "{mcycle} {kind} {:04X} {:02X}",
        access.addr, access.value
    )
}

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let mut rom = None;
    let mut mcycles = 1_048_576;
    let mut out = None;
    let mut bootrom = None;
    let mut frames = None;
    let mut pixels = None;
    let mut cuts = None;
    while let Some(arg) = args.next() {
        let mut value = |name: &str| {
            args.next()
                .with_context(|| format!("{name} needs a value\n\n{USAGE}"))
        };
        match arg.as_str() {
            "--mcycles" => mcycles = value("--mcycles")?.parse()?,
            "--out" => out = Some(PathBuf::from(value("--out")?)),
            "--frames" => frames = Some(PathBuf::from(value("--frames")?)),
            "--pixels" => pixels = Some(PathBuf::from(value("--pixels")?)),
            "--cuts" => cuts = Some(PathBuf::from(value("--cuts")?)),
            "--bootrom" => bootrom = Some(PathBuf::from(value("--bootrom")?)),
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
    let traced = match out {
        Some(path) => {
            let file =
                File::create(&path).with_context(|| format!("can't create {}", path.display()))?;
            trace(
                &rom,
                bootrom.as_deref(),
                mcycles,
                BufWriter::new(file),
                frames,
                pixels,
                cuts,
            )
        }
        None => trace(
            &rom,
            bootrom.as_deref(),
            mcycles,
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
