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
//! With `--test` (mooneye, gbmicrotest or blargg), the comparison also ends
//! where the chip shows the test's result, as `ceres_netlist::run` reads it.
//!
//! The pictures are compared as well, as `hdl/compare-frames.sh` compares
//! the model's and ceres's: ceres's frame as STAT's mode becomes 1
//! (`bus_trace --frames`), the chip's as the model's testbench reads its LCD
//! pins (`lcd` in `hdl/board/gameboy_tb.vhd`, after dmg-sim's
//! `vid_dump.sv`). Neither takes the first frame after the LCD is turned on,
//! and the frames of one colour before the first picture are left out on
//! each side. A picture that differs does not stop the comparison of the
//! accesses: the report gives the first, with its first pixels, and how
//! many differ. `--ceres-frames FILE` and `--chip-frames FILE` write each
//! side's frames as the model's testbench does (`FRAMES`).
//!
//! Usage: `cargo run -p ceres-test-runner --release --example lockstep --
//! <rom> [--bootrom FILE] [--mcycles N] [--test KIND] [--vram-arbitrary]
//! [--context N] [--ceres-frames FILE] [--chip-frames FILE]`

use anyhow::{Context as _, Result, bail};
use ceres_core::{ColorCorrectionMode, Gb, GbBuilder, IoAccess, MemoryAccess, Model};
use ceres_netlist::{
    chip::NetChip,
    run::{CpuAccess, Outcome, Run, Test},
};
use ceres_test_runner::{DummyAudioCallback, load_test_rom};
use std::{
    collections::VecDeque,
    fs::File,
    io::{BufWriter, Write as _},
    path::{Path, PathBuf},
    process::ExitCode,
    time::Instant,
};

const USAGE: &str = "\
usage: lockstep <rom> [--bootrom FILE] [--mcycles N] [--test KIND] [--vram-arbitrary] [--context N]
       [--ceres-frames FILE] [--chip-frames FILE]

<rom> is a path, or one under external/test-roms.
--mcycles: M-cycles to compare from the first access (1048576, a second, by
default).
--test: mooneye, gbmicrotest or blargg; ends where the chip shows the result.
--context: accesses shown before the first that differs (8 by default).
--ceres-frames, --chip-frames: write that side's frames, as the model does.";

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

const WIDTH: usize = 160;
const HEIGHT: usize = 144;

/// A frame: the DMG's shades, 0 (white) to 3 (black), line by line.
type Frame = Box<[u8]>;

/// A pixel that differs: x, y, ceres's shade, the chip's.
type Pixel = (usize, usize, u8, u8);

fn blank() -> Frame {
    vec![0; WIDTH * HEIGHT].into_boxed_slice()
}

/// One side's frames: written to a file if asked, and the pictures not yet
/// compared, after the frames of one colour at the start.
struct Frames {
    file: Option<BufWriter<File>>,
    taken: u64,
    /// Before the first picture, and the frames of one colour left out.
    before_picture: bool,
    left_out: u64,
    pictures: VecDeque<Frame>,
}

impl Frames {
    fn new(path: Option<&Path>) -> Result<Self> {
        let file = path
            .map(|p| File::create(p).with_context(|| format!("can't write {}", p.display())))
            .transpose()?;
        Ok(Self {
            file: file.map(BufWriter::new),
            taken: 0,
            before_picture: true,
            left_out: 0,
            pictures: VecDeque::new(),
        })
    }

    fn take(&mut self, frame: Frame) -> Result<()> {
        if let Some(file) = self.file.as_mut() {
            writeln!(file, "frame {}", self.taken)?;
            for line in frame.as_chunks::<WIDTH>().0 {
                let digits: Vec<u8> = line.iter().map(|&shade| b'0' + shade).collect();
                file.write_all(&digits)?;
                file.write_all(b"\n")?;
            }
        }
        self.taken += 1;
        if self.before_picture && frame.iter().all(|&shade| shade == frame[0]) {
            self.left_out += 1;
        } else {
            self.before_picture = false;
            self.pictures.push_back(frame);
        }
        Ok(())
    }

    fn flush(&mut self) -> Result<()> {
        if let Some(file) = self.file.as_mut() {
            file.flush()?;
        }
        Ok(())
    }
}

/// The pictures compared so far.
#[derive(Default)]
struct Pictures {
    compared: u64,
    differing: u64,
    /// The first that differs: its number, its pixels that differ, the
    /// first of them (x, y, ceres's shade, the chip's).
    first: Option<(u64, usize, Vec<Pixel>)>,
    /// The last that differs, and its pixels that differ.
    last: Option<(u64, usize)>,
}

impl Pictures {
    /// Compares the pictures both sides have.
    fn compare(&mut self, ceres: &mut Frames, chip: &mut Frames) {
        while !ceres.pictures.is_empty() && !chip.pictures.is_empty() {
            let (Some(ours), Some(theirs)) =
                (ceres.pictures.pop_front(), chip.pictures.pop_front())
            else {
                break;
            };
            let number = self.compared;
            self.compared += 1;
            let differ: Vec<Pixel> = ours
                .iter()
                .zip(theirs.iter())
                .enumerate()
                .filter(|(_, (a, b))| a != b)
                .map(|(i, (&a, &b))| (i % WIDTH, i / WIDTH, a, b))
                .collect();
            if differ.is_empty() {
                continue;
            }
            self.differing += 1;
            self.last = Some((number, differ.len()));
            if self.first.is_none() {
                let shown = differ.iter().take(8).copied().collect();
                self.first = Some((number, differ.len(), shown));
            }
        }
    }

    #[expect(clippy::print_stdout, reason = "a command")]
    fn report(&self, ceres: &Frames, chip: &Frames) {
        println!(
            "frames: {} on ceres, {} on the chip; {} and {} of one colour at the start left out",
            ceres.taken, chip.taken, ceres.left_out, chip.left_out
        );
        let Some((number, pixels, shown)) = &self.first else {
            println!("the same pictures: {}", self.compared);
            return;
        };
        println!("picture {number} differs in {pixels} pixels:");
        for (x, y, ours, theirs) in shown {
            println!("    x={x} y={y} ceres {ours} chip {theirs}");
        }
        if let Some((last, pixels)) = self.last {
            println!(
                "{} of {} pictures differ; the last, {last}, in {pixels} pixels",
                self.differing, self.compared
            );
        }
    }
}

/// The LCD's pins as the chip drives them after a phase.
#[derive(Clone, Copy, Default)]
#[expect(clippy::struct_excessive_bools, reason = "the pins' levels")]
struct LcdPins {
    cp: bool,
    st: bool,
    cpl: bool,
    cpg: bool,
    s: bool,
    ld: u8,
}

impl LcdPins {
    /// As `hdl/rtl/lcd_timing.vhd` and `render.vhd` drive them: while the
    /// LCD is off, CPL follows the divider's bit 6.
    const fn of(chip: &NetChip) -> Self {
        let (cp, st, ld) = chip.render.pins();
        let lcd = &chip.lcd;
        let lcd_on = chip.control.lcdc & 0x80 != 0;
        Self {
            cp,
            st,
            cpl: if lcd_on {
                lcd.vclk
            } else {
                chip.timer.count & 0x40 != 0
            },
            cpg: lcd.cpg_lx | lcd.vclk,
            s: lcd.first_line,
            ld,
        }
    }
}

/// The LCD as the model's testbench reads it (`lcd` in
/// `hdl/board/gameboy_tb.vhd`): a line's pixels on the falls of CP (ST high
/// at a fall starts the line again) and of ST, and on the rise of CPL,
/// which latches the line; S starts the frame, which is whole when its last
/// line is latched. A pixel is the shade LD held as its strobe changed. Like
/// the testbench, it sees the pins' pulses within a phase.
struct Lcd {
    pins: LcdPins,
    line: [u8; WIDTH],
    frame: Frame,
    x: usize,
    /// `HEIGHT`: no frame is being taken, until S starts one.
    y: usize,
    enabled: bool,
}

impl Lcd {
    fn new() -> Self {
        Self {
            pins: LcdPins::default(),
            line: [0; WIDTH],
            frame: blank(),
            x: 0,
            y: HEIGHT,
            enabled: false,
        }
    }

    const fn take_pixel(&mut self, ld: u8) {
        if self.x < WIDTH {
            self.line[self.x] = ld;
            self.x += 1;
        }
    }

    /// After a phase of `chip`: the frame, if it became whole. Render's
    /// pins first, as they changed within the phase, then the line timing's.
    fn phase(&mut self, chip: &NetChip) -> Option<Frame> {
        let log = &chip.render_pins;
        let mut whole = None;
        for &(cp, st, ld) in &log.changes[..log.len] {
            whole = whole.or_else(|| {
                self.change(LcdPins {
                    cp,
                    st,
                    ld,
                    ..self.pins
                })
            });
        }
        whole.or_else(|| self.change(LcdPins::of(chip)))
    }

    /// The pins as they change: the frame, if it became whole.
    fn change(&mut self, now: LcdPins) -> Option<Frame> {
        let was = self.pins;
        self.pins = now;
        let mut whole = None;
        if now.s && !was.s {
            self.y = 0;
        }
        if was.cp && !now.cp {
            self.take_pixel(was.ld);
            if now.st {
                self.x = 0;
            }
        }
        if was.st && !now.st {
            self.take_pixel(was.ld);
        }
        if now.cpl && !was.cpl {
            self.take_pixel(was.ld);
            if self.enabled && self.y < HEIGHT {
                // Fewer than 160 pixels repeat, as in vid_dump.sv.
                let row = &mut self.frame[self.y * WIDTH..][..WIDTH];
                for (i, pixel) in row.iter_mut().enumerate() {
                    *pixel = if self.x > 0 { self.line[i % self.x] } else { 0 };
                }
                self.y += 1;
                if self.y == HEIGHT {
                    whole = Some(self.frame.clone());
                }
            }
        }
        if was.cpl && !now.cpl {
            // The first frame after the LCD is turned on is not shown, nor
            // one cut off as the LCD went off.
            if now.cpg && !self.enabled {
                self.y = HEIGHT;
            }
            self.enabled = now.cpg;
        }
        whole
    }
}

/// Ceres from power-on, its accesses as `bus_trace` numbers them.
#[expect(clippy::struct_excessive_bools, reason = "the frame taker's levels")]
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
    /// Its frames: the LCD was on after the last step, STAT showed mode 1,
    /// a frame is under way (S started it), and the next frame is not
    /// shown. S does not start the first frame after the LCD is turned on,
    /// so the testbench takes it only as the rest of a frame the LCD cut
    /// off, if it did not see the LCD off: if CPL did not fall meanwhile.
    /// CPL follows the divider's bit 6 (DIV's bit 0) while the LCD is off,
    /// and the line clock, low at first, once it is on: it falls too if the
    /// LCD is turned on as DIV's bit 0 is high. An LCD off for a moment
    /// within a frame (gbmicrotest's `lcd_off_unsafe` right before `lcd_on`)
    /// goes unseen, and the next frame is shown.
    frames: Frames,
    lcd_on: bool,
    in_vblank: bool,
    in_frame: bool,
    unshown: bool,
    div_bit0: bool,
    seen_off: bool,
}

impl Ceres {
    fn new(rom: &[u8], bootrom: &[u8], frames: Frames) -> Result<Self> {
        let mut gb = GbBuilder::new(48000, DummyAudioCallback)
            .with_model(Model::DmgB)
            .with_run_bootrom(true)
            .with_rom(rom.into())
            .context("can't load the ROM")?
            .with_bootrom(bootrom.into())
            .build();
        gb.set_color_correction_mode(ColorCorrectionMode::Disabled);
        gb.set_bus_log(true);
        Ok(Self {
            gb,
            first: None,
            last: None,
            ready: VecDeque::new(),
            dropped: 0,
            frames,
            lcd_on: false,
            in_vblank: false,
            in_frame: false,
            unshown: true,
            div_bit0: false,
            seen_off: true,
        })
    }

    /// The frame ceres presents as STAT's mode becomes 1, after a step
    /// (a fraction of a line), as `bus_trace --frames` takes it.
    fn take_frame(&mut self) -> Result<()> {
        let on = self.gb.peek_mem(0xFF40) & 0x80 != 0;
        let div_bit0 = self.gb.peek_mem(0xFF04) & 1 != 0;
        let vblank = self.gb.peek_mem(0xFF41) & 3 == 1;
        if on && self.in_vblank && !vblank {
            self.in_frame = true;
        }
        // The step's write to LCDC is its last M-cycle: a fall of the bit
        // within the step that turns the LCD off comes before it, and within
        // the step that turns it on, before it or, the bit high at the
        // write, as CPL leaves it.
        if !on && self.lcd_on {
            self.seen_off = false;
        } else if !on && self.div_bit0 && !div_bit0 {
            self.seen_off = true;
        }
        if on && !self.lcd_on {
            self.unshown = !self.in_frame || self.seen_off || self.div_bit0 || div_bit0;
        }
        self.lcd_on = on;
        self.div_bit0 = div_bit0;
        if vblank && !self.in_vblank {
            self.in_frame = false;
            if self.unshown {
                self.unshown = false;
            } else {
                let mut frame = blank();
                let rgba = self.gb.pixel_data_rgba();
                for (shade, px) in frame.iter_mut().zip(rgba.as_chunks::<4>().0) {
                    *shade = (0xFF - px[0]) / 0x55;
                }
                self.frames.take(frame)?;
            }
        }
        self.in_vblank = vblank;
        Ok(())
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
    fn next(&mut self, limit: u64) -> Result<Option<Access>> {
        while self.ready.is_empty() {
            let elapsed = self
                .first
                .map_or(0, |first| self.gb.clock_units().saturating_sub(first));
            if elapsed / (2 * UNITS_PER_HALF) > limit {
                return Ok(self.last.take());
            }
            self.gb.step();
            self.take_frame()?;
            for access in self.gb.take_io_log() {
                self.take(&access);
            }
        }
        Ok(self.ready.pop_front())
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
    /// The test whose result ends the comparison, and the result once shown.
    test: Option<Test>,
    outcome: Option<Outcome>,
    /// Its LCD, and the frames it shows.
    lcd: Lcd,
    frames: Frames,
}

impl Chip {
    /// The next access, if it comes before M-cycle `limit` and the test's
    /// result.
    fn next(&mut self, limit: u64) -> Result<Option<Access>> {
        while self.run.mcycle.unwrap_or(0) <= limit && self.outcome.is_none() {
            let access = self.run.step();
            if let Some(frame) = self.lcd.phase(&self.run.chip) {
                self.frames.take(frame)?;
            }
            if let Some(test) = self.test {
                self.outcome = self.run.result(test);
            }
            if let Some(access) = access {
                return Ok(Some(access.into()));
            }
        }
        Ok(None)
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
    test: Option<Test>,
    vram_zero: bool,
    context: usize,
    ceres_frames: Option<PathBuf>,
    chip_frames: Option<PathBuf>,
}

fn options() -> Result<Options> {
    let mut args = std::env::args().skip(1);
    let mut rom = None;
    let mut bootrom = Path::new(env!("CARGO_MANIFEST_DIR")).join("../hdl/build/fastboot.bin");
    let mut mcycles = 1_048_576;
    let mut test = None;
    let mut vram_zero = true;
    let mut context = 8;
    let mut ceres_frames = None;
    let mut chip_frames = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--bootrom" => bootrom = args.next().context(USAGE)?.into(),
            "--mcycles" => mcycles = args.next().context(USAGE)?.parse().context(USAGE)?,
            "--test" => {
                test = Some(match args.next().context(USAGE)?.as_str() {
                    "mooneye" => Test::Mooneye,
                    "gbmicrotest" => Test::Gbmicrotest,
                    "blargg" => Test::Blargg,
                    _ => bail!(USAGE),
                });
            }
            "--vram-arbitrary" => vram_zero = false,
            "--context" => context = args.next().context(USAGE)?.parse().context(USAGE)?,
            "--ceres-frames" => ceres_frames = Some(args.next().context(USAGE)?.into()),
            "--chip-frames" => chip_frames = Some(args.next().context(USAGE)?.into()),
            _ if arg.starts_with("--") || rom.is_some() => bail!(USAGE),
            _ => rom = Some(PathBuf::from(arg)),
        }
    }
    Ok(Options {
        rom: rom.context(USAGE)?,
        bootrom,
        mcycles,
        test,
        vram_zero,
        context,
        ceres_frames,
        chip_frames,
    })
}

#[expect(clippy::print_stdout, clippy::print_stderr, reason = "a command")]
fn main() -> Result<ExitCode> {
    let options = options()?;
    let rom = read_rom(&options.rom)?;
    let bootrom = std::fs::read(&options.bootrom)
        .with_context(|| format!("can't read {}", options.bootrom.display()))?;
    let mut ceres = Ceres::new(
        &rom,
        &bootrom,
        Frames::new(options.ceres_frames.as_deref())?,
    )?;
    let vram = if options.vram_zero {
        Box::new([0; 0x2000])
    } else {
        ceres_netlist::render::vram_power_on()
    };
    let mut chip = Chip {
        run: Run::with_vram(rom, bootrom, vram),
        test: options.test,
        outcome: None,
        lcd: Lcd::new(),
        frames: Frames::new(options.chip_frames.as_deref())?,
    };
    let mut pictures = Pictures::default();

    let started = Instant::now();
    let mut before = VecDeque::new();
    let mut compared = 0_u64;
    let limit = options.mcycles;
    loop {
        let theirs = chip.next(limit)?;
        if chip.outcome.is_some() && theirs.is_none() {
            break;
        }
        let ours = ceres.next(limit)?;
        pictures.compare(&mut ceres.frames, &mut chip.frames);
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
            pictures.report(&ceres.frames, &chip.frames);
            ceres.frames.flush()?;
            chip.frames.flush()?;
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
    let until = match &chip.outcome {
        Some(Outcome::Pass(_)) => "the result (pass)".to_owned(),
        Some(Outcome::Fail(_)) => "the result (fail)".to_owned(),
        Some(Outcome::Timeout) | None => format!("{} M-cycles", options.mcycles),
    };
    println!(
        "the same: {compared} accesses, up to {until} ({} of ceres's dropped)",
        ceres.dropped
    );
    pictures.compare(&mut ceres.frames, &mut chip.frames);
    pictures.report(&ceres.frames, &chip.frames);
    ceres.frames.flush()?;
    chip.frames.flush()?;
    eprintln!("in {:.1} s", started.elapsed().as_secs_f64());
    Ok(if pictures.differing == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}
