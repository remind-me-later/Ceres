//! The lockstep oracle: runs a ROM on ceres and on `NetChip` (ceres-netlist's
//! transcription of the readable model of the chip) side by side from
//! power-on, compares the CPU's memory accesses one by one, and stops at the
//! first that differs:
//!
//! ```text
//! 1234 R FF44 90
//! ```
//!
//! An access is the M-cycle it happened in, counted from the CPU's first
//! access, whether it read or wrote, the address and the byte, as
//! `bus_trace` writes ceres's and the model's testbench its own
//! (`hdl/compare-ceres.sh`, which compares their files). Here nothing is
//! written down: both run in one process, and the comparison ends where
//! they part, with the accesses before it and the CPU's registers on both
//! sides.
//!
//! The boot ROM is `hdl/build/fastboot.bin` (or `--bootrom FILE`). The
//! model's board can zero VRAM at power-on (`VRAM_ZERO`), as ceres does;
//! here it does, unless `--vram-arbitrary` keeps the board's arbitrary
//! bytes. OAM powers up FF on the chip, 0 in ceres: a ROM that reads it
//! before it writes it differs there.
//!
//! Usage: `cargo run -p ceres-test-runner --release --example lockstep --
//! <rom> [--bootrom FILE] [--mcycles N] [--vram-arbitrary] [--context N]`

use anyhow::{Context as _, Result, bail};
use ceres_core::{Gb, GbBuilder, IoAccess, MemoryAccess, Model};
use ceres_netlist::run::{CpuAccess, Run};
use ceres_test_runner::{DummyAudioCallback, load_test_rom};
use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    process::ExitCode,
    time::Instant,
};

const USAGE: &str = "\
usage: lockstep <rom> [--bootrom FILE] [--mcycles N] [--vram-arbitrary] [--context N]

<rom> is a path, or one under external/test-roms.
--mcycles: M-cycles to compare from the first access (1048576, a second, by
default).
--context: accesses shown before the first that differs (8 by default).";

/// 8 MHz units per half M-cycle of the normal speed.
const UNITS_PER_HALF: u64 = 4;

/// An access as both sides give it.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Access {
    mcycle: u64,
    write: bool,
    addr: u16,
    value: u8,
}

impl std::fmt::Display for Access {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let kind = if self.write { 'W' } else { 'R' };
        write!(
            f,
            "{} {kind} {:04X} {:02X}",
            self.mcycle, self.addr, self.value
        )
    }
}

impl From<CpuAccess> for Access {
    fn from(a: CpuAccess) -> Self {
        Self {
            mcycle: a.mcycle,
            write: a.write,
            addr: a.addr,
            value: a.value,
        }
    }
}

/// Ceres from power-on, its accesses as `bus_trace` numbers them.
struct Ceres {
    gb: Gb<DummyAudioCallback>,
    /// `clock_units` of the first access.
    first: Option<u64>,
    /// The last access, given once the next is in another M-cycle: of two
    /// in one M-cycle (ceres's HALT reads PC once without spending time on
    /// it), the last counts.
    last: Option<Access>,
    ready: VecDeque<Access>,
    /// Accesses dropped so.
    dropped: u64,
}

impl Ceres {
    fn new(rom: &[u8], bootrom: &[u8]) -> Result<Self> {
        let mut gb = GbBuilder::new(48000, DummyAudioCallback)
            .with_model(Model::DmgB)
            .with_run_bootrom(true)
            .with_rom(rom.into())
            .context("can't load the ROM")?
            .with_bootrom(bootrom.into())
            .build();
        gb.set_bus_log(true);
        Ok(Self {
            gb,
            first: None,
            last: None,
            ready: VecDeque::new(),
            dropped: 0,
        })
    }

    fn take(&mut self, access: &IoAccess) {
        let first = *self.first.get_or_insert(access.clock_units);
        let half = (access.clock_units - first + UNITS_PER_HALF / 2) / UNITS_PER_HALF;
        let this = Access {
            mcycle: half / 2,
            write: access.access == MemoryAccess::Write,
            addr: access.addr,
            value: access.value,
        };
        match self.last {
            Some(last) if last.mcycle == this.mcycle => self.dropped += 1,
            Some(last) => self.ready.push_back(last),
            None => {}
        }
        self.last = Some(this);
    }

    /// The next access, if it comes before M-cycle `limit` (the CPU can
    /// stop accessing for good: the older Mooneye tests end on the opcode
    /// ED, which locks it).
    fn next(&mut self, limit: u64) -> Option<Access> {
        while self.ready.is_empty() {
            let elapsed = self
                .first
                .map_or(0, |first| self.gb.clock_units().saturating_sub(first));
            if elapsed / (2 * UNITS_PER_HALF) > limit {
                return self.last.take();
            }
            self.gb.step();
            for access in self.gb.take_io_log() {
                self.take(&access);
            }
        }
        self.ready.pop_front()
    }

    fn registers(&self) -> String {
        let gb = &self.gb;
        format!(
            "AF={:04X} BC={:04X} DE={:04X} HL={:04X} SP={:04X} PC={:04X}",
            gb.cpu_af(),
            gb.cpu_bc(),
            gb.cpu_de(),
            gb.cpu_hl(),
            gb.cpu_sp(),
            gb.cpu_pc()
        )
    }
}

/// `NetChip` from power-on.
struct Chip {
    run: Run,
}

impl Chip {
    /// The next access, if it comes before M-cycle `limit`.
    fn next(&mut self, limit: u64) -> Option<Access> {
        while self.run.mcycle.unwrap_or(0) <= limit {
            if let Some(access) = self.run.step() {
                return Some(access.into());
            }
        }
        None
    }

    fn registers(&self) -> String {
        let r = &self.run.chip.core.regs;
        let pair = |h: u8, l: u8| u16::from_be_bytes([h, l]);
        let f = self.run.chip.core.alu.flags.bits();
        format!(
            "AF={:04X} BC={:04X} DE={:04X} HL={:04X} SP={:04X} PC={:04X}",
            pair(r.a.q, f),
            pair(r.b.q, r.c.q),
            pair(r.d.q, r.e.q),
            pair(r.h.q, r.l.q),
            pair(r.sph.q, r.spl.q),
            pair(r.pch.q, r.pcl.q)
        )
    }
}

/// Reads `path` as given, else under the test ROMs directory.
fn read_rom(path: &Path) -> Result<Vec<u8>> {
    if path.exists() {
        std::fs::read(path).with_context(|| format!("can't read {}", path.display()))
    } else {
        load_test_rom(path)
    }
}

struct Options {
    rom: PathBuf,
    bootrom: PathBuf,
    mcycles: u64,
    vram_zero: bool,
    context: usize,
}

fn options() -> Result<Options> {
    let mut args = std::env::args().skip(1);
    let mut rom = None;
    let mut bootrom = Path::new(env!("CARGO_MANIFEST_DIR")).join("../hdl/build/fastboot.bin");
    let mut mcycles = 1_048_576;
    let mut vram_zero = true;
    let mut context = 8;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--bootrom" => bootrom = args.next().context(USAGE)?.into(),
            "--mcycles" => mcycles = args.next().context(USAGE)?.parse().context(USAGE)?,
            "--vram-arbitrary" => vram_zero = false,
            "--context" => context = args.next().context(USAGE)?.parse().context(USAGE)?,
            _ if arg.starts_with("--") || rom.is_some() => bail!(USAGE),
            _ => rom = Some(PathBuf::from(arg)),
        }
    }
    Ok(Options {
        rom: rom.context(USAGE)?,
        bootrom,
        mcycles,
        vram_zero,
        context,
    })
}

#[expect(clippy::print_stdout, clippy::print_stderr, reason = "a command")]
fn main() -> Result<ExitCode> {
    let options = options()?;
    let rom = read_rom(&options.rom)?;
    let bootrom = std::fs::read(&options.bootrom)
        .with_context(|| format!("can't read {}", options.bootrom.display()))?;
    let mut ceres = Ceres::new(&rom, &bootrom)?;
    let vram = if options.vram_zero {
        Box::new([0; 0x2000])
    } else {
        ceres_netlist::render::vram_power_on()
    };
    let mut chip = Chip {
        run: Run::with_vram(rom, bootrom, vram),
    };

    let started = Instant::now();
    let mut before = VecDeque::new();
    let mut compared = 0_u64;
    let limit = options.mcycles;
    loop {
        let (ours, theirs) = (ceres.next(limit), chip.next(limit));
        let show = |access: Option<Access>| access.map_or_else(|| "none".into(), |a| a.to_string());
        if ours != theirs {
            println!("ceres and the chip part at access {compared}:");
            for access in &before {
                println!("    {access}");
            }
            println!("  ceres {}", show(ours));
            println!("  chip  {}", show(theirs));
            // Each side's own step: ceres's an instruction's, the chip's a
            // phase's, so PC can be a byte apart.
            println!("registers, as each side's step left them:");
            println!("  ceres {}", ceres.registers());
            println!("  chip  {}", chip.registers());
            eprintln!(
                "{compared} accesses the same, in {:.1} s",
                started.elapsed().as_secs_f64()
            );
            return Ok(ExitCode::FAILURE);
        }
        let Some(theirs) = theirs else {
            break;
        };
        compared += 1;
        if theirs.mcycle >= limit {
            break;
        }
        before.push_back(theirs);
        if before.len() > options.context {
            before.pop_front();
        }
    }
    println!(
        "the same: {compared} accesses, {} M-cycles ({} of ceres's dropped)",
        options.mcycles, ceres.dropped
    );
    eprintln!("in {:.1} s", started.elapsed().as_secs_f64());
    Ok(ExitCode::SUCCESS)
}
