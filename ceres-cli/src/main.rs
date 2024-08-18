use app::App;
use clap::Parser;
use std::path::PathBuf;
use winit::{dpi::PhysicalSize, event_loop::EventLoop};

mod app;
mod audio;
mod video;

extern crate alloc;

const PX_WIDTH: u32 = ceres_core::PX_WIDTH as u32;
const PX_HEIGHT: u32 = ceres_core::PX_HEIGHT as u32;
const INIT_WIDTH: u32 = PX_WIDTH * SCREEN_MUL;
const INIT_HEIGHT: u32 = PX_HEIGHT * SCREEN_MUL;

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

const SCREEN_MUL: u32 = 3;

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

#[derive(Default, Clone, Copy, clap::ValueEnum)]
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

#[derive(clap::Parser)]
#[command(name = CERES_BIN, about = ABOUT, after_help = AFTER_HELP)]
struct Cli {
    #[arg(
        help = "Game Boy/Color ROM file to emulate.",
        long_help = "Game Boy/Color ROM file to emulate. Extension doesn't matter, the \
           emulator will check the file is a valid Game Boy ROM reading its \
           header. Doesn't accept compressed (zip) files.",
        required = true
    )]
    file: PathBuf,
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

    let event_loop = EventLoop::new()?;

    let window_attributes = winit::window::Window::default_attributes()
        .with_title(CERES_STYLIZED)
        .with_inner_size(PhysicalSize {
            width: INIT_WIDTH,
            height: INIT_HEIGHT,
        })
        .with_min_inner_size(PhysicalSize {
            width: PX_WIDTH,
            height: PX_HEIGHT,
        });

    // The template will match only the configurations supporting rendering
    // to windows.
    //
    // XXX We force transparency only on macOS, given that EGL on X11 doesn't
    // have it, but we still want to show window. The macOS situation is like
    // that, because we can query only one config at a time on it, but all
    // normal platforms will return multiple configs, so we can find the config
    // with transparency ourselves inside the `reduce`.
    let template = glutin::config::ConfigTemplateBuilder::new().with_transparency(false);

    let display_builder =
        glutin_winit::DisplayBuilder::new().with_window_attributes(Some(window_attributes));

    let mut app = App::new(
        args.model.into(),
        args.file,
        args.scaling,
        template,
        display_builder,
    )?;

    event_loop.run_app(&mut app)?;

    Ok(())
}
