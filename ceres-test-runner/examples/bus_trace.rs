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
//! `--bootrom FILE` runs another boot ROM, such as dmg-sim's quickboot, which
//! skips the logo but leaves the machine as the real one does.
//!
//! Usage: `cargo run -p ceres-test-runner --example bus_trace -- <rom>
//! [--bootrom FILE] [--mcycles N] [--out FILE]`

use anyhow::{Context as _, Result, bail};
use ceres_core::{GbBuilder, IoAccess, MemoryAccess, Model};
use ceres_test_runner::{DummyAudioCallback, load_test_rom};
use std::{
    fs::File,
    io::{self, BufWriter, Write},
    path::{Path, PathBuf},
};

const USAGE: &str = "\
usage: bus_trace <rom> [--bootrom FILE] [--mcycles N] [--out FILE]

<rom> is a path, or one under external/test-roms.
--mcycles: M-cycles to trace from the first access (1048576, a second, by
default).";

/// 8 MHz units per M-cycle at single speed.
const UNITS_PER_MCYCLE: u64 = 8;

/// Reads `path` as given, else under the test ROMs directory.
fn read_rom(path: &Path) -> Result<Vec<u8>> {
    if path.exists() {
        std::fs::read(path).with_context(|| format!("can't read {}", path.display()))
    } else {
        load_test_rom(path)
    }
}

fn trace<W: Write>(rom: &Path, bootrom: Option<&Path>, mcycles: u64, mut out: W) -> Result<()> {
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

    let mut first = None;
    // The last access, written once the next one is in another M-cycle.
    let mut last: Option<(u64, IoAccess)> = None;
    let mut dropped = 0_u64;
    loop {
        gb.step();
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
                if dropped > 0 {
                    eprintln!("{dropped} accesses dropped (two in one M-cycle)");
                }
                return Ok(());
            }
            last = Some((mcycle, access));
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
    while let Some(arg) = args.next() {
        let mut value = |name: &str| {
            args.next()
                .with_context(|| format!("{name} needs a value\n\n{USAGE}"))
        };
        match arg.as_str() {
            "--mcycles" => mcycles = value("--mcycles")?.parse()?,
            "--out" => out = Some(PathBuf::from(value("--out")?)),
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
    let traced = match out {
        Some(path) => {
            let file =
                File::create(&path).with_context(|| format!("can't create {}", path.display()))?;
            trace(&rom, bootrom.as_deref(), mcycles, BufWriter::new(file))
        }
        None => trace(
            &rom,
            bootrom.as_deref(),
            mcycles,
            BufWriter::new(io::stdout().lock()),
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
