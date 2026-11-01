//! Runs a ROM on `NetChip` from the power-on, as the readable model's
//! testbench does (`hdl/run-tests.sh` with `MODEL=1`), and prints how its
//! test ended:
//!
//!   cargo run -p ceres-netlist --release --example run -- KIND ROM SECS [BOOT_ROM]
//!
//! KIND is how the ROM reports (mooneye, gbmicrotest or blargg), SECS the
//! time limit after the chip's reset, BOOT_ROM the boot ROM (by default
//! `hdl/build/fastboot.bin`, which `make -C hdl` builds).

use {
    ceres_netlist::run::{Outcome, Run, Test},
    std::{env, fs, path::Path, process::ExitCode, time::Instant},
};

const USAGE: &str = "usage: run KIND ROM SECS [BOOT_ROM] (KIND: mooneye, gbmicrotest or blargg)";

#[expect(clippy::print_stdout, clippy::print_stderr, reason = "a command")]
fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let (Some(kind), Some(rom), Some(secs)) = (args.first(), args.get(1), args.get(2)) else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    let test = match kind.as_str() {
        "mooneye" => Test::Mooneye,
        "gbmicrotest" => Test::Gbmicrotest,
        "blargg" => Test::Blargg,
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    let Ok(secs) = secs.parse::<f64>() else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    let fastboot = Path::new(env!("CARGO_MANIFEST_DIR")).join("../hdl/build/fastboot.bin");
    let boot = args.get(3).map_or(fastboot, Into::into);
    let (rom, boot_rom) = match (fs::read(rom), fs::read(&boot)) {
        (Ok(rom), Ok(boot_rom)) => (rom, boot_rom),
        (Err(e), _) => {
            eprintln!("{rom}: {e}");
            return ExitCode::from(2);
        }
        (_, Err(e)) => {
            eprintln!("{}: {e}", boot.display());
            return ExitCode::from(2);
        }
    };
    let started = Instant::now();
    let mut run = Run::new(rom, boot_rom);
    let outcome = run.until(test, secs);
    let serial = String::from_utf8_lossy(&run.serial);
    if !serial.is_empty() {
        eprintln!("serial: {serial}");
    }
    eprintln!(
        "{} edges in {:.1} s",
        run.edge,
        started.elapsed().as_secs_f64()
    );
    match outcome {
        Outcome::Pass(detail) => {
            println!("pass {detail}");
            ExitCode::SUCCESS
        }
        Outcome::Fail(detail) => {
            println!("fail {detail}");
            ExitCode::FAILURE
        }
        Outcome::Timeout => {
            println!("timeout");
            ExitCode::FAILURE
        }
    }
}
