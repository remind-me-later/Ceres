#![warn(clippy::all)]
mod ceres_glfw;

use ceres_glfw::CeresGlfw;

fn main() {
    env_logger::init();

    let args = ceres_cli::ArgumentParser::new();
    let sav_path = args.sav_path;

    let ceres_glfw = CeresGlfw::new(args.model, args.cartridge, args.boot_rom)
        .unwrap_or_else(|error| ceres_cli::error::print(error));

    let cartridge = ceres_glfw.run();

    ceres_cli::ArgumentParser::save_data(&sav_path, &cartridge);
}
