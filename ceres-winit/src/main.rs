mod app;
mod video;

#[cfg(target_os = "macos")]
mod macos;

use app::App;
use clap::Parser;
use std::path::PathBuf;
use winit::event_loop::EventLoop;

const WIN_MULTIPLIER: u32 = 2;

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
            Self::Dmg => "dmg",
            Self::Mgb => "mgb",
            Self::Cgb => "cgb",
        }
    }

    fn iter() -> impl Iterator<Item = Self> {
        [Self::Dmg, Self::Mgb, Self::Cgb].into_iter()
    }
}

impl From<Model> for ceres_std::Model {
    fn from(model: Model) -> Self {
        match model {
            Model::Dmg => Self::Dmg,
            Model::Mgb => Self::Mgb,
            Model::Cgb => Self::Cgb,
        }
    }
}

#[derive(Default, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum ShaderOption {
    Nearest = 0,
    Scale2x = 1,
    Scale3x = 2,
    #[default]
    Lcd = 3,
    Crt = 4,
}

impl AppOption for ShaderOption {
    fn str(self) -> &'static str {
        match self {
            Self::Nearest => "nearest",
            Self::Scale2x => "scale2x",
            Self::Scale3x => "scale3x",
            Self::Lcd => "lcd",
            Self::Crt => "crt",
        }
    }

    fn iter() -> impl Iterator<Item = Self> {
        [
            Self::Nearest,
            Self::Scale2x,
            Self::Scale3x,
            Self::Lcd,
            Self::Crt,
        ]
        .into_iter()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, clap::ValueEnum)]
pub enum ScalingOption {
    PixelPerfect,
    #[default]
    Stretch,
}

impl AppOption for ScalingOption {
    fn str(self) -> &'static str {
        match self {
            Self::PixelPerfect => "pixel-perfect",
            Self::Stretch => "stretch",
        }
    }

    fn iter() -> impl Iterator<Item = Self> {
        [Self::PixelPerfect, Self::Stretch].into_iter()
    }
}

#[derive(Clone)]
pub enum CeresEvent {
    ChangeShader(ShaderOption),
    ChangeScaling(ScalingOption),
    OpenRomFile(PathBuf),
    ChangeSpeed(u32),
    TogglePause,
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
        short = 'x',
        long,
        help = "Shader used",
        default_value = ShaderOption::default().str(),
        value_enum,
        required = false
    )]
    shader_option: ShaderOption,
    #[arg(
        short = 's',
        long,
        help = "Pixel mode",
        default_value = ScalingOption::default().str(),
        value_enum,
        required = false
    )]
    scaling_option: ScalingOption,
}
fn main() -> anyhow::Result<()> {
    let args = Cli::parse();

    let main_event_loop = if cfg!(target_os = "macos") {
        use winit::platform::macos::EventLoopBuilderExtMacOS;
        EventLoop::<CeresEvent>::with_user_event()
            .with_default_menu(false)
            .build()?
    } else {
        EventLoop::<CeresEvent>::with_user_event().build()?
    };

    #[cfg(target_os = "macos")]
    {
        macos::set_event_proxy(main_event_loop.create_proxy());
        macos::create_menu_bar();
    }

    let project_dirs = directories::ProjectDirs::from(QUALIFIER, ORGANIZATION, CERES_STYLIZED)
        .ok_or_else(|| {
            anyhow::anyhow!("Failed to get project directories for '{}'", CERES_STYLIZED)
        })?;

    let mut main_window = App::new(
        project_dirs,
        args.model.into(),
        args.file.as_deref(),
        args.shader_option,
        args.scaling_option,
    )?;

    main_event_loop.run_app(&mut main_window)?;

    Ok(())
}
