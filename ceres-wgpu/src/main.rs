use app::App;
use clap::builder::EnumValueParser;
use winit::event_loop::EventLoop;
use {
    anyhow::Context,
    clap::{Arg, Command},
    std::path::PathBuf,
};

mod app;
mod audio;
mod video;

extern crate alloc;

const CERES_BIN: &str = "ceres";
const CERES_STYLIZED: &str = "Ceres";
const ABOUT: &str = "A (very experimental) Game Boy/Color emulator.";
const AFTER_HELP: &str = "GB bindings:

    | Gameboy | Emulator  |
    | ------- | --------- |
    | Dpad    | WASD      |
    | A       | K         |
    | B       | L         |
    | Start   | Return    |
    | Select  | Backspace |
    
Other binsings:

    | System       | Emulator |
    | ------------ | -------- |
    | Fullscreen   | F        |
    | Scale filter | Z        |
";

const SCREEN_MUL: u32 = 3;

#[derive(Default, Clone, Copy)]
enum Model {
    Dmg,
    Mgb,
    #[default]
    Cgb,
}

impl clap::ValueEnum for Model {
    fn value_variants<'a>() -> &'a [Self] {
        &[Self::Dmg, Self::Mgb, Self::Cgb]
    }

    fn to_possible_value(&self) -> Option<clap::builder::PossibleValue> {
        Some(match self {
            Self::Dmg => clap::builder::PossibleValue::new("dmg"),
            Self::Mgb => clap::builder::PossibleValue::new("mgb"),
            Self::Cgb => clap::builder::PossibleValue::new("cgb"),
        })
    }
}

impl From<Model> for ceresc::Model {
    fn from(model: Model) -> ceresc::Model {
        match model {
            Model::Dmg => ceresc::Model::Dmg,
            Model::Mgb => ceresc::Model::Mgb,
            Model::Cgb => ceresc::Model::Cgb,
        }
    }
}

#[derive(Default, Clone, Copy)]
pub enum Scaling {
    #[default]
    Nearest = 0,
    Scale2x = 1,
    Scale3x = 2,
}

impl Scaling {
    pub fn next(self) -> Self {
        match self {
            Scaling::Nearest => Scaling::Scale2x,
            Scaling::Scale2x => Scaling::Scale3x,
            Scaling::Scale3x => Scaling::Nearest,
        }
    }
}

impl clap::ValueEnum for Scaling {
    fn value_variants<'a>() -> &'a [Self] {
        &[Self::Nearest, Self::Scale2x, Self::Scale3x]
    }

    fn to_possible_value(&self) -> Option<clap::builder::PossibleValue> {
        Some(match self {
            Self::Nearest => clap::builder::PossibleValue::new("nearest"),
            Self::Scale2x => clap::builder::PossibleValue::new("scale2x"),
            Self::Scale3x => clap::builder::PossibleValue::new("scale3x"),
        })
    }
}

fn main() -> anyhow::Result<()> {
    let args = Command::new(CERES_BIN)
        .bin_name(CERES_BIN)
        .about(ABOUT)
        .after_help(AFTER_HELP)
        .arg(
            Arg::new("file")
                .required(true)
                .help("Game Boy/Color ROM file to emulate.")
                .long_help(
                    "Game Boy/Color ROM file to emulate. Extension doesn't matter, the \
           emulator will check the file is a valid Game Boy ROM reading its \
           header. Doesn't accept compressed (zip) files.",
                ),
        )
        .arg(
            Arg::new("model")
                .short('m')
                .long("model")
                .help("Game Boy model to emulate")
                .value_parser(EnumValueParser::<Model>::new())
                .default_value("cgb")
                .required(false),
        )
        .arg(
            Arg::new("scaling")
                .short('s')
                .long("scaling")
                .help("Scaling algorithm used")
                .value_parser(EnumValueParser::<Scaling>::new())
                .default_value("nearest")
                .required(false),
        )
        .get_matches();

    let model = *args
        .get_one::<Model>("model")
        .context("couldn't get model string")?;

    let scaling = *args
        .get_one::<Scaling>("scaling")
        .context("couldn't get scaling string")?;

    let rom_path = args
        .get_one::<String>("file")
        .map(PathBuf::from)
        .context("no path provided")?;

    let event_loop = EventLoop::new()?;
    let mut app = App::new(model.into(), rom_path, scaling)?;
    event_loop.run_app(&mut app)?;

    Ok(())
}
