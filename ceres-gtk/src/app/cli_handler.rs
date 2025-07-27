use ceres_std::{Model, ScalingOption, ShaderOption};
use std::path::PathBuf;

#[derive(Clone)]
pub struct CliOptions {
    pub file: Option<PathBuf>,
    pub model: Model,
    pub shader_option: ShaderOption,
    pub scaling_option: ScalingOption,
}

impl Default for CliOptions {
    fn default() -> Self {
        Self {
            file: None,
            model: Model::Cgb,
            shader_option: ShaderOption::Nearest,
            scaling_option: ScalingOption::Stretch,
        }
    }
}

impl CliOptions {
    pub fn parse_from_args(args: &[String]) -> Self {
        let mut options = Self::default();
        let mut i = 1; // Skip program name

        while i < args.len() {
            match args[i].as_str() {
                "-m" | "--model" => {
                    if i + 1 < args.len() {
                        options.model = match args[i + 1].as_str() {
                            "dmg" => Model::Dmg,
                            "mgb" => Model::Mgb,
                            "cgb" => Model::Cgb,
                            _ => Model::Cgb,
                        };
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
                "-s" | "--shader-option" => {
                    if i + 1 < args.len() {
                        options.shader_option = match args[i + 1].as_str() {
                            "nearest" => ShaderOption::Nearest,
                            "scale2x" => ShaderOption::Scale2x,
                            "scale3x" => ShaderOption::Scale3x,
                            "lcd" => ShaderOption::Lcd,
                            "crt" => ShaderOption::Crt,
                            _ => ShaderOption::Nearest,
                        };
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
                "-p" | "--pixel-mode" => {
                    if i + 1 < args.len() {
                        options.scaling_option = match args[i + 1].as_str() {
                            "pixel-perfect" => ScalingOption::PixelPerfect,
                            "stretch" => ScalingOption::Stretch,
                            _ => ScalingOption::Stretch,
                        };
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
                "-h" | "--help" => {
                    Self::print_help();
                    std::process::exit(0);
                }
                arg if !arg.starts_with('-') => {
                    // Assume it's a file path
                    options.file = Some(PathBuf::from(arg));
                    i += 1;
                }
                _ => {
                    i += 1;
                }
            }
        }

        options
    }

    fn print_help() {
        println!("Ceres - A (very experimental) Game Boy/Color emulator.");
        println!();
        println!("USAGE:");
        println!("    ceres [OPTIONS] [FILE]");
        println!();
        println!("ARGS:");
        println!("    <FILE>    Game Boy/Color ROM file to emulate");
        println!();
        println!("OPTIONS:");
        println!(
            "    -m, --model <MODEL>              Game Boy model to emulate [default: cgb] [possible values: dmg, mgb, cgb]"
        );
        println!(
            "    -s, --shader-option <SHADER>     Shader used [default: nearest] [possible values: nearest, scale2x, scale3x, lcd, crt]"
        );
        println!(
            "    -p, --pixel-mode <MODE>          Pixel mode [default: stretch] [possible values: pixel-perfect, stretch]"
        );
        println!("    -h, --help                       Print help");
        println!();
        println!("GB bindings:");
        println!();
        println!("    | Gameboy | Emulator  |");
        println!("    | ------- | --------- |");
        println!("    | Dpad    | WASD      |");
        println!("    | A       | K         |");
        println!("    | B       | L         |");
        println!("    | Start   | M         |");
        println!("    | Select  | N         |");
        println!();
        println!("Other bindings:");
        println!();
        println!("    | System       | Emulator |");
        println!("    | ------------ | -------- |");
        println!("    | Fullscreen   | F        |");
        println!("    | Scale filter | Z        |");
    }
}
