#![warn(clippy::all)]
mod audio;
mod emulator;
mod error;

use ceres_core::{BootRom, Cartridge, Model};
use clap::{Arg, Command};
use emulator::Emulator;
use error::Error;
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::Path,
};

pub const CERES_STR: &str = "Ceres";

fn main() {
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

    let (cartridge, sav_path) = {
        let rom_path = matches
            .value_of("rom")
            .map(|s| Path::new(s).to_path_buf())
            .unwrap();

        let rom_buf = read_file(&rom_path)
            .unwrap_or_else(|e| error::print(e))
            .into_boxed_slice();

        let sav_path = rom_path.with_extension("sav");
        let ram = if let Ok(sav_buf) = read_file(&sav_path) {
            Some(sav_buf.into_boxed_slice())
        } else {
            None
        };

        let cartridge = Cartridge::new(rom_buf, ram).unwrap_or_else(|e| error::print(e));

        // if matches.is_present("info") {
        //     println!("{}", cartridge.unwrap());
        //     exit(0);
        // }
        (cartridge, sav_path)
    };

    let model = if let Some(model_str) = matches.value_of("model") {
        match model_str {
            "dmg" => Model::Dmg,
            "mgb" => Model::Mgb,
            "cgb" => Model::Cgb,
            _ => unreachable!("invalid model"),
        }
    } else {
        Model::Cgb
    };

    let boot_rom_str = matches.value_of("boot").unwrap_or_else(|| match model {
        Model::Dmg => "BootROMs/build/bin/dmg_boot.bin",
        Model::Mgb => "BootROMs/build/bin/mgb_boot.bin",
        Model::Cgb => "BootROMs/build/bin/cgb_boot_fast.bin",
    });

    let boot_rom = {
        let boot_rom_path = Path::new(&boot_rom_str);
        let boot_rom_buf = read_file(boot_rom_path)
            .unwrap_or_else(|e| error::print(format!("could not load boot ROM {}", e)))
            .into_boxed_slice();

        BootRom::new(boot_rom_buf)
    };

    let emulator =
        Emulator::new(model, cartridge, boot_rom).unwrap_or_else(|error| error::print(error));

    emulator.run(sav_path);
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
