mod app;
mod screen;

use eframe::egui;

use app::App;
use clap::Parser;
use std::path::PathBuf;

const QUALIFIER: &str = "com";
const ORGANIZATION: &str = "remind-me-later";
const CERES_BIN: &str = "ceres";
const CERES_STYLIZED: &str = "Ceres";
const ABOUT: &str = "A (very experimental) Game Boy/Color emulator.";
const AFTER_HELP: &str = "GB bindings:

    | Gameboy | Emulator  |
    | ------- | --------- |
    | Dpad    | WASD      |
    | A       | K         |
    | B       | L         |
    | Start   | M         |
    | Select  | N         |

Other binsings:

    | System       | Emulator |
    | ------------ | -------- |
    | Fullscreen   | F        |
    | Scale filter | Z        |
";

trait AppOption: Default + Clone + Copy + clap::ValueEnum {
    fn str(self) -> &'static str;
    fn iter() -> impl Iterator<Item = Self>;
}

#[derive(Default, Clone, Copy, clap::ValueEnum)]
enum Model {
    Dmg,
    Mgb,
    #[default]
    Cgb,
}

impl AppOption for Model {
    fn str(self) -> &'static str {
        match self {
            Model::Dmg => "dmg",
            Model::Mgb => "mgb",
            Model::Cgb => "cgb",
        }
    }

    fn iter() -> impl Iterator<Item = Self> {
        [Model::Dmg, Model::Mgb, Model::Cgb].into_iter()
    }
}

impl From<Model> for ceres_core::Model {
    fn from(model: Model) -> ceres_core::Model {
        match model {
            Model::Dmg => ceres_core::Model::Dmg,
            Model::Mgb => ceres_core::Model::Mgb,
            Model::Cgb => ceres_core::Model::Cgb,
        }
    }
}

#[derive(Default, Clone, Copy, PartialEq, clap::ValueEnum)]
pub enum ShaderOption {
    Nearest = 0,
    Scale2x = 1,
    Scale3x = 2,
    #[default]
    Lcd = 3,
    Crt = 4,
}

impl ShaderOption {
    #[must_use]
    pub fn next(self) -> Self {
        match self {
            ShaderOption::Nearest => ShaderOption::Scale2x,
            ShaderOption::Scale2x => ShaderOption::Scale3x,
            ShaderOption::Scale3x => ShaderOption::Lcd,
            ShaderOption::Lcd => ShaderOption::Crt,
            ShaderOption::Crt => ShaderOption::Nearest,
        }
    }
}

impl AppOption for ShaderOption {
    fn str(self) -> &'static str {
        match self {
            ShaderOption::Nearest => "nearest",
            ShaderOption::Scale2x => "scale2x",
            ShaderOption::Scale3x => "scale3x",
            ShaderOption::Lcd => "lcd",
            ShaderOption::Crt => "crt",
        }
    }

    fn iter() -> impl Iterator<Item = Self> {
        [
            ShaderOption::Nearest,
            ShaderOption::Scale2x,
            ShaderOption::Scale3x,
            ShaderOption::Lcd,
            ShaderOption::Crt,
        ]
        .into_iter()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, clap::ValueEnum)]
pub enum PixelMode {
    PixelPerfect,
    #[default]
    FitWindow,
}

impl AppOption for PixelMode {
    fn str(self) -> &'static str {
        match self {
            PixelMode::PixelPerfect => "pixel-perfect",
            PixelMode::FitWindow => "fit-window",
        }
    }

    fn iter() -> impl Iterator<Item = Self> {
        [PixelMode::PixelPerfect, PixelMode::FitWindow].into_iter()
    }
}

#[derive(clap::Parser)]
#[command(name = CERES_BIN, about = ABOUT, after_help = AFTER_HELP)]
struct Cli {
    #[arg(
        help = "Game Boy/Color ROM file to emulate.",
        long_help = "Game Boy/Color ROM file to emulate. Extension doesn't matter, the \
           emulator will check the file is a valid Game Boy ROM reading its \
           header. Doesn't accept compressed (zip) files.",
        required = false
    )]
    file: Option<PathBuf>,
    #[arg(
        short,
        long,
        help = "Game Boy model to emulate",
        default_value = Model::default().str(),
        value_enum,
        required = false
    )]
    model: Model,
    #[arg(
        short,
        long,
        help = "Shader used",
        default_value = ShaderOption::default().str(),
        value_enum,
        required = false
    )]
    shader_option: ShaderOption,
}

fn main() -> anyhow::Result<()> {
    let args = Cli::parse();
    let project_dirs = directories::ProjectDirs::from(QUALIFIER, ORGANIZATION, CERES_STYLIZED)
        .ok_or_else(|| anyhow::anyhow!("couldn't get project directories"))?;

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([
                f32::from(ceres_core::PX_WIDTH),
                f32::from(ceres_core::PX_HEIGHT) + 22.0,
            ])
            .with_min_inner_size([
                f32::from(ceres_core::PX_WIDTH),
                f32::from(ceres_core::PX_HEIGHT) + 22.0,
            ]),
        renderer: eframe::Renderer::Wgpu,
        vsync: true,
        depth_buffer: 0,
        stencil_buffer: 0,
        centered: true,
        ..Default::default()
    };
    eframe::run_native(
        CERES_STYLIZED,
        options,
        Box::new(move |cc| {
            Ok(Box::new(App::new(
                cc,
                args.model.into(),
                project_dirs,
                args.file.as_deref(),
                args.shader_option,
            )?))
        }),
    )
    .map_err(Into::into)
}
