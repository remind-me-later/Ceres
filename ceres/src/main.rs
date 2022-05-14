#![warn(clippy::pedantic)]
#![allow(clippy::cast_lossless)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::similar_names)]

mod audio;
mod emulator;
mod video;

use {ceres_core::Model, emulator::Emulator, std::path::Path};

const CERES_STR: &str = "Ceres";
const HELP: &str = "TODO";

fn main() {
    let args = match parse_args() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Error: {}.", e);
            std::process::exit(1);
        }
    };

    let model = args.model.map_or(Model::Cgb, |s| match s.as_str() {
        "dmg" => Model::Dmg,
        "mgb" => Model::Mgb,
        "cgb" => Model::Cgb,
        _ => panic!("invalid model"),
    });

    let rom_path = Path::new(&args.rom).to_path_buf();
    let emulator = Emulator::new(model, &rom_path);

    emulator.run();
}

struct AppArgs {
    rom: String,
    model: Option<String>,
}

fn parse_args() -> Result<AppArgs, pico_args::Error> {
    let mut pargs = pico_args::Arguments::from_env();

    // Help has a higher priority and should be handled separately.
    if pargs.contains(["-h", "--help"]) {
        print!("{}", HELP);
        std::process::exit(0);
    }

    let args = AppArgs {
        // Parses an optional value that implements `FromStr`.
        model: pargs.opt_value_from_str(["-m", "--model"])?,
        // Parses an optional value from `&str` using a specified function.
        rom: pargs.free_from_str()?,
    };

    // It's up to the caller what to do with the remaining arguments.
    let remaining = pargs.finish();
    if !remaining.is_empty() {
        eprintln!("Warning: unused arguments left: {:?}.", remaining);
    }

    Ok(args)
}
