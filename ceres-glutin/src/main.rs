#![warn(clippy::all)]
mod ceres_glutin;
mod error;

use ceres_core::{BootRom, Cartridge, Model};
use ceres_glutin::CeresGlfw;
use clap::{Arg, Command};
use error::Error;
use simplelog::*;
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::Path,
    process::exit,
};

pub const CERES_STR: &str = "ceres";

fn main() {
    CombinedLogger::init(vec![
        TermLogger::new(
            LevelFilter::Warn,
            Config::default(),
            TerminalMode::Mixed,
            ColorChoice::Auto,
        ),
        // WriteLogger::new(
        //     LevelFilter::Trace,
        //     Config::default(),
        //     File::create("ceres.log").unwrap(),
        // ),
    ])
    .unwrap();

    let matches = Command::new(CERES_STR)
        .about("GameBoy/Color emulator")
        .arg(
            Arg::new("rom")
                .value_name("ROM")
                .help("Cartridge ROM to emulate")
                .takes_value(true)
                .required(true),
        )
        .arg(
            Arg::new("info")
                .short('i')
                .long("info")
                .help("Print ROM information and exit"),
        )
        .arg(
            Arg::new("model")
                .short('m')
                .long("model")
                .help("GameBoy model to emulate")
                .possible_values(["dmg", "mgb", "cgb"])
                .takes_value(true)
                .ignore_case(true),
        )
        .arg(
            Arg::new("boot")
                .short('b')
                .long("boot")
                .help("Boot ROM to emulate")
                .takes_value(true),
        )
        .get_matches();

    let rom_string = matches.value_of("rom").unwrap();

    let rom_path = Path::new(&rom_string);
    let rom_buf = read_file(rom_path)
        .unwrap_or_else(|e| error::print(e))
        .into_boxed_slice();

    let sav_path = rom_path.with_extension("sav");
    let ram = if let Ok(sav_buf) = read_file(&sav_path) {
        Some(sav_buf.into_boxed_slice())
    } else {
        None
    };

    let cartridge = Cartridge::new(rom_buf, ram).unwrap_or_else(|e| error::print(e));

    if matches.is_present("info") {
        println!("{}", cartridge);
        exit(0);
    }

    let model = matches.value_of("model").map(|s| match s {
        "dmg" => Model::Dmg,
        "mgb" => Model::Mgb,
        "cgb" => Model::Cgb,
        _ => unreachable!("invalid model"),
    });

    let boot_rom = if let Some(boot_rom_str) = matches.value_of("boot") {
        let boot_rom_path = Path::new(&boot_rom_str);
        let boot_rom_buf = read_file(boot_rom_path)
            .unwrap_or_else(|e| error::print(e))
            .into_boxed_slice();

        let boot_rom = BootRom::new(boot_rom_buf);

        Some(boot_rom)
    } else {
        None
    };

    let ceres_glfw =
        CeresGlfw::new(model, cartridge, boot_rom).unwrap_or_else(|error| error::print(error));

    ceres_glfw.run(sav_path);
}

fn read_file(path: &Path) -> Result<Vec<u8>, Error> {
    let mut f = File::open(path).map_err(|_| Error::new("no file found"))?;
    let metadata = fs::metadata(&path).map_err(|_| Error::new("unable to read metadata"))?;
    let mut buffer = vec![0; metadata.len() as usize];
    f.read(&mut buffer)
        .map_err(|_| Error::new("buffer overflow"))?;

    Ok(buffer)
}

pub fn save_data(sav_path: &Path, cartridge: &Cartridge) {
    let mut f = File::create(sav_path)
        .unwrap_or_else(|_| error::print(Error::new("unable to open save file")));

    f.write_all(cartridge.ram())
        .unwrap_or_else(|_| error::print(Error::new("buffer overflow")));
}
