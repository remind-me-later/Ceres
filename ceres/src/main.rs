mod app;
mod gb_area;
mod scene;

const SCREEN_MUL: u32 = 1;
const PX_WIDTH: u32 = ceres_core::PX_WIDTH as u32;
const PX_HEIGHT: u32 = ceres_core::PX_HEIGHT as u32;
const INIT_WIDTH: u32 = PX_WIDTH * SCREEN_MUL;
const INIT_HEIGHT: u32 = PX_HEIGHT * SCREEN_MUL;

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

#[derive(Default, Debug, Clone, Copy, PartialEq, clap::ValueEnum)]
pub enum Scaling {
    #[default]
    Nearest = 0,
    Scale2x = 1,
    Scale3x = 2,
}

impl Scaling {
    pub const ALL: [Scaling; 3] = [Scaling::Nearest, Scaling::Scale2x, Scaling::Scale3x];

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
    file: Option<std::path::PathBuf>,
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

pub fn main() -> iced::Result {
    let args = <crate::Cli as clap::Parser>::parse();

    iced::application(app::App::title, app::App::update, app::App::view)
        .subscription(app::App::subscription)
        .default_font(iced::Font {
            family: iced::font::Family::Monospace,
            ..Default::default()
        })
        .window_size(iced::Size {
            width: INIT_WIDTH as f32,
            height: INIT_HEIGHT as f32,
        })
        .resizable(true)
        .scale_factor(|_| 0.8)
        .theme(app::App::theme)
        .exit_on_close_request(true)
        .run_with(move || (app::App::new(&args).unwrap(), iced::Task::none()))
}
