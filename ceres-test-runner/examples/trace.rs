//! Traces a ROM instruction by instruction, in Gameboy Doctor's format
//! (<https://github.com/robert/gameboy-doctor>): a line per instruction with
//! the CPU's state before it runs.
//!
//! ```text
//! A:01 F:B0 B:00 C:13 D:00 E:D8 H:01 L:4D SP:FFFE PC:0100 PCMEM:00,C3,13,02
//! ```
//!
//! - `--timing` appends ` CY:<clock> LY:<line> DOT:<dot>`: the clock in
//!   half dots (8 MHz units) since the machine was made, and where the
//!   display is on the timing unit's clock, in decimal.
//! - `--io` adds a line per access the instruction made to an I/O register,
//!   `  R FF44=90 CY:… LY:… DOT:…` or `  W …`, after its state line.
//! - `--state FILE` starts from a BESS state; SameBoy saves and loads the
//!   same format, for a diff from the same moment. The model must be the
//!   one the state was saved with.
//! - `--skip-boot` starts from the state the boot ROM leaves, as Doctor's
//!   logs do.
//!
//! While HALT holds the CPU only its first line is written, and an
//! interrupt dispatch shows as the jump to its vector. Doctor's reference
//! logs also expect LY to read 0x90 all the time, which Ceres doesn't fake:
//! they match a trace up to the first LY read.
//!
//! A line is about 80 bytes, so a frame takes a few megabytes.
//!
//! Usage: `cargo run -p ceres-test-runner --example trace -- <rom> [options]`

use anyhow::{Context as _, Result, bail};
use ceres_core::{Gb, GbBuilder, IoAccess, MemoryAccess, Model};
use ceres_test_runner::{DummyAudioCallback, load_test_rom};
use std::{
    fs::File,
    io::{self, BufWriter, Write},
    path::{Path, PathBuf},
};

const USAGE: &str = "\
usage: trace <rom> [--model dmg|mgb|cgb|cgbc] [--state FILE] [--skip-boot]
             [--frames N] [--max-lines N] [--timing] [--io] [--out FILE]

<rom> is a path, or one under external/test-roms. --model cgb is a CGB-E.
--frames: frames to trace (1 by default).";

struct Options {
    rom: PathBuf,
    model: Model,
    state: Option<PathBuf>,
    skip_boot: bool,
    frames: u32,
    max_lines: Option<u64>,
    timing: bool,
    io: bool,
    out: Option<PathBuf>,
}

impl Options {
    fn parse() -> Result<Self> {
        let mut args = std::env::args().skip(1);
        let mut rom = None;
        let mut options = Self {
            rom: PathBuf::new(),
            model: Model::CgbE,
            state: None,
            skip_boot: false,
            frames: 1,
            max_lines: None,
            timing: false,
            io: false,
            out: None,
        };
        while let Some(arg) = args.next() {
            let mut value = |name: &str| {
                args.next()
                    .with_context(|| format!("{name} needs a value\n\n{USAGE}"))
            };
            match arg.as_str() {
                "--model" => {
                    options.model = match value("--model")?.as_str() {
                        "dmg" => Model::DmgB,
                        "mgb" => Model::Mgb,
                        "cgb" => Model::CgbE,
                        "cgbc" => Model::CgbC,
                        model => bail!("unknown model {model}\n\n{USAGE}"),
                    };
                }
                "--state" => options.state = Some(value("--state")?.into()),
                "--skip-boot" => options.skip_boot = true,
                "--frames" => options.frames = value("--frames")?.parse()?,
                "--max-lines" => options.max_lines = Some(value("--max-lines")?.parse()?),
                "--timing" => options.timing = true,
                "--io" => options.io = true,
                "--out" => options.out = Some(value("--out")?.into()),
                "--help" | "-h" => {
                    println!("{USAGE}");
                    std::process::exit(0);
                }
                _ if arg.starts_with("--") => bail!("unknown option {arg}\n\n{USAGE}"),
                _ => rom = Some(PathBuf::from(arg)),
            }
        }
        options.rom = rom.with_context(|| format!("which ROM?\n\n{USAGE}"))?;
        Ok(options)
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

fn build(options: &Options) -> Result<Gb<DummyAudioCallback>> {
    let rom = read_rom(&options.rom)?;
    let mut gb = GbBuilder::new(48000, DummyAudioCallback)
        .with_model(options.model)
        .with_run_bootrom(!options.skip_boot)
        .with_rom(rom.into_boxed_slice())
        .with_context(|| format!("can't load {}", options.rom.display()))?
        .build();
    if let Some(path) = &options.state {
        let state =
            std::fs::read(path).with_context(|| format!("can't read {}", path.display()))?;
        // As of the Unix epoch: the cartridge's clock doesn't advance, so
        // a trace doesn't depend on when it runs.
        gb.load_state(&state, 0).map_err(|e| {
            anyhow::anyhow!(
                "can't load {}: {e} (is --model the model it was saved with?)",
                path.display()
            )
        })?;
    }
    Ok(gb)
}

/// Writes trace lines.
struct Tracer<W: Write> {
    out: W,
    timing: bool,
    lines: u64,
}

impl<W: Write> Tracer<W> {
    /// The CPU's state before the instruction at PC.
    fn state(&mut self, gb: &Gb<DummyAudioCallback>) -> io::Result<()> {
        let pc = gb.cpu_pc();
        let mem = |offset: u16| gb.peek_mem(pc.wrapping_add(offset));
        write!(
            self.out,
            "A:{:02X} F:{:02X} B:{:02X} C:{:02X} D:{:02X} E:{:02X} H:{:02X} L:{:02X} \
             SP:{:04X} PC:{pc:04X} PCMEM:{:02X},{:02X},{:02X},{:02X}",
            gb.cpu_a(),
            gb.cpu_f(),
            gb.cpu_b(),
            gb.cpu_c(),
            gb.cpu_d(),
            gb.cpu_e(),
            gb.cpu_h(),
            gb.cpu_l(),
            gb.cpu_sp(),
            mem(0),
            mem(1),
            mem(2),
            mem(3),
        )?;
        if self.timing {
            let ppu = gb.ppu_state();
            write!(
                self.out,
                " CY:{} LY:{} DOT:{}",
                gb.clock_units(),
                ppu.line,
                ppu.half_dot / 2
            )?;
        }
        self.end_line()
    }

    fn io(&mut self, access: &IoAccess) -> io::Result<()> {
        let kind = if access.access == MemoryAccess::Write {
            'W'
        } else {
            'R'
        };
        write!(
            self.out,
            "  {kind} {:04X}={:02X} CY:{} LY:{} DOT:{}",
            access.addr,
            access.value,
            access.clock_units,
            access.line,
            access.half_dot / 2
        )?;
        self.end_line()
    }

    fn end_line(&mut self) -> io::Result<()> {
        self.lines += 1;
        writeln!(self.out)
    }
}

fn trace<W: Write>(options: &Options, gb: &mut Gb<DummyAudioCallback>, out: W) -> Result<()> {
    let mut tracer = Tracer {
        out,
        timing: options.timing,
        lines: 0,
    };
    let max_lines = options.max_lines.unwrap_or(u64::MAX);
    gb.set_io_log(options.io);

    tracer.state(gb)?;
    let mut frames = 0;
    let mut was_halted = gb.cpu_is_halted();
    while frames < options.frames && tracer.lines < max_lines {
        frames += u32::from(gb.step());
        for access in gb.take_io_log() {
            tracer.io(&access)?;
        }
        let halted = gb.cpu_is_halted();
        if !(halted && was_halted) {
            tracer.state(gb)?;
        }
        was_halted = halted;
    }
    tracer.out.flush()?;
    Ok(())
}

fn main() -> Result<()> {
    let options = Options::parse()?;
    let mut gb = build(&options)?;
    let traced = match &options.out {
        Some(path) => {
            let file =
                File::create(path).with_context(|| format!("can't create {}", path.display()))?;
            trace(&options, &mut gb, BufWriter::new(file))
        }
        None => trace(&options, &mut gb, BufWriter::new(io::stdout().lock())),
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
