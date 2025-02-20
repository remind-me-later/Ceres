mod app;
mod gb_thread;
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

#[derive(Default, Clone, Copy, clap::ValueEnum)]
enum Model {
    Dmg,
    Mgb,
    #[default]
    Cgb,
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
pub enum Scaling {
    #[default]
    Nearest = 0,
    Scale2x = 1,
    Scale3x = 2,
}

impl Scaling {
    #[must_use]
    pub fn next(self) -> Self {
        match self {
            Scaling::Nearest => Scaling::Scale2x,
            Scaling::Scale2x => Scaling::Scale3x,
            Scaling::Scale3x => Scaling::Nearest,
        }
    }
}

impl std::fmt::Display for Scaling {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Scaling::Nearest => write!(f, "Nearest"),
            Scaling::Scale2x => write!(f, "Scale2x"),
            Scaling::Scale3x => write!(f, "Scale3x"),
        }
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
        default_value = "cgb",
        value_enum,
        required = false
    )]
    model: Model,
    #[arg(
        short,
        long,
        help = "Scaling algorithm used",
        default_value = "nearest",
        value_enum,
        required = false
    )]
    scaling: Scaling,
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
                args.scaling,
            )?))
        }),
    )
    .map_err(Into::into)
}
