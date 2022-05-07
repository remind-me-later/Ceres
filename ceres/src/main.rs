mod audio;
mod emulator;

use {argh::FromArgs, ceres_core::Model, emulator::Emulator, std::path::Path};

pub const CERES_STR: &str = "Ceres";
const DMG_BOOT_ROM_PATH: &str = concat!("bootroms/bin/", "dmg_boot.bin");
const MGB_BOOT_ROM_PATH: &str = concat!("bootroms/bin/", "mgb_boot.bin");
const CGB_BOOT_ROM_PATH: &str = concat!("bootroms/bin/", "cgb_boot_fast.bin");

#[derive(FromArgs)]
#[argh(description = "cli arguments")]
struct Args {
    #[argh(positional, description = "rom to emulate")]
    rom: String,

    #[argh(option, description = "bootrom to use")]
    bootrom: Option<String>,

    #[argh(option, description = "model to emulate [dmg, mgb, cgb]")]
    model: Option<String>,
}

fn main() {
    let opts: Args = argh::from_env();

    let model = opts.model.map_or(Model::Cgb, |s| match s.as_str() {
        "dmg" => Model::Dmg,
        "mgb" => Model::Mgb,
        "cgb" => Model::Cgb,
        _ => panic!("invalid model"),
    });

    let boot_rom_str = opts.bootrom.unwrap_or_else(|| {
        match model {
            Model::Dmg => DMG_BOOT_ROM_PATH,
            Model::Mgb => MGB_BOOT_ROM_PATH,
            Model::Cgb => CGB_BOOT_ROM_PATH,
        }
        .to_owned()
    });

    let boot_rom_path = Path::new(&boot_rom_str);
    let rom_path = Path::new(&opts.rom).to_path_buf();
    let emulator = Emulator::new(model, boot_rom_path, &rom_path);

    emulator.run();
}
