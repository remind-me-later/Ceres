pub mod error;

pub const CERES_STR: &str = "ceres";

use ceres_core::{BootRom, Cartridge, Model};
use error::Error;
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    process::exit,
};

// TODO: write help
const HELP: &str = "\
App
USAGE:
  app [OPTIONS] --number NUMBER [INPUT]
FLAGS:
  -h, --help            Prints help information
OPTIONS:
  --number NUMBER       Sets a number
  --opt-number NUMBER   Sets an optional number
  --width WIDTH         Sets width [default: 10]
  --output PATH         Sets an output path
ARGS:
  <INPUT>
";

pub struct ArgumentParser {
    pub cartridge: ceres_core::Cartridge,
    pub model: ceres_core::Model,
    pub boot_rom: Option<ceres_core::BootRom>,
    pub sav_path: PathBuf,
}

impl ArgumentParser {
    fn read_file_to_byte_vec(path: &Path) -> Result<Vec<u8>, Error> {
        let mut f = File::open(path).map_err(|_| Error::new("no file found"))?;
        let metadata = fs::metadata(&path).map_err(|_| Error::new("unable to read metadata"))?;
        let mut buffer = vec![0; metadata.len() as usize];
        f.read(&mut buffer)
            .map_err(|_| Error::new("buffer overflow"))?;

        Ok(buffer)
    }

    pub fn save_data(sav_path: &Path, cartridge: &Cartridge) {
        let mut f =
            File::create(sav_path).unwrap_or_else(|_| error::print(Error::new("no file found")));

        f.write_all(cartridge.ram())
            .unwrap_or_else(|_| error::print(Error::new("buffer overflow")));
    }

    pub fn new() -> Self {
        let mut pargs = pico_args::Arguments::from_env();

        // Help has a higher priority and should be handled separately.
        if pargs.contains(["-h", "--help"]) {
            print!("{}", HELP);
            std::process::exit(0);
        }

        let rom_string: String = pargs.free_from_str().unwrap_or_else(|e| error::print(e));
        let rom_path = Path::new(&rom_string);
        let rom_buf = Self::read_file_to_byte_vec(rom_path)
            .unwrap_or_else(|e| error::print(e))
            .into_boxed_slice();

        let sav_path = rom_path.with_extension("sav");
        let ram = if let Ok(sav_buf) = Self::read_file_to_byte_vec(&sav_path) {
            Some(sav_buf.into_boxed_slice())
        } else {
            None
        };

        let cartridge =
            ceres_core::Cartridge::new(rom_buf, ram).unwrap_or_else(|e| error::print(e));

        if pargs.contains("--rom-info") {
            println!("{}", cartridge);
            exit(0);
        }

        let model = if let Some(model_str) = pargs
            .opt_value_from_str::<&str, String>("--model")
            .unwrap_or_else(|e| error::print(e))
        {
            match model_str.as_ref() {
                "dmg" => Model::Dmg,
                "mgb" => Model::Mgb,
                "cgb" => Model::Cgb,
                _ => error::print(format!(
                    "invalid model flag: {model_str}, recognized flags are: dmg, mgb, cgb"
                )),
            }
        } else {
            Model::Cgb
        };

        let boot_rom = if let Some(boot_rom_str) = pargs
            .opt_value_from_str::<&str, String>("--boot-rom")
            .unwrap_or_else(|e| error::print(e))
        {
            let boot_rom_path = Path::new(&boot_rom_str);
            let boot_rom_buf = Self::read_file_to_byte_vec(boot_rom_path)
                .unwrap_or_else(|e| error::print(e))
                .into_boxed_slice();

            let boot_rom = BootRom::new(boot_rom_buf);

            Some(boot_rom)
        } else {
            None
        };

        Self {
            cartridge,
            model,
            boot_rom,
            sav_path,
        }
    }
}
