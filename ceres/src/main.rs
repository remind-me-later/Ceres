mod audio;
mod emulator;

use {
    ceres_core::Model,
    clap::{Arg, Command},
    emulator::Emulator,
    std::path::Path,
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

    let boot_rom_str = matches.value_of("boot").unwrap_or(match model {
        Model::Dmg => "BootROMs/build/bin/dmg_boot.bin",
        Model::Mgb => "BootROMs/build/bin/mgb_boot.bin",
        Model::Cgb => "BootROMs/build/bin/cgb_boot_fast.bin",
    });

    let boot_rom_path = Path::new(&boot_rom_str);

    let rom_path = matches
        .value_of("rom")
        .map(|s| Path::new(s).to_path_buf())
        .unwrap();

    let emulator = Emulator::new(model, boot_rom_path, &rom_path);

    emulator.run();
}
