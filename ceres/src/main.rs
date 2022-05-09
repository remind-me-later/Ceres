mod audio;
mod emulator;
mod video;

use {argh::FromArgs, ceres_core::Model, emulator::Emulator, std::path::Path};

pub const CERES_STR: &str = "Ceres";

#[derive(FromArgs)]
#[argh(description = "cli arguments")]
struct Args {
    #[argh(positional, description = "rom to emulate")]
    rom: String,

    #[argh(option, short = 'm', description = "model to emulate [dmg, mgb, cgb]")]
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

    let rom_path = Path::new(&opts.rom).to_path_buf();
    let emulator = Emulator::new(model, &rom_path);

    emulator.run();
}
