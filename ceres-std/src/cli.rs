use crate::ShaderOption;
pub use clap;
use std::path::{Path, PathBuf};

pub const QUALIFIER: &str = "com.github";
pub const ORGANIZATION: &str = "remind-me-later";
pub const CERES_BIN: &str = "ceres";
pub const CERES_STYLIZED: &str = "Ceres";
const ABOUT: &str = "A (very experimental) Game Boy/Color emulator.";
const AFTER_HELP: &str = "GB bindings:

    | Gameboy | Emulator  |
    | ------- | --------- |
    | Dpad    | WASD      |
    | A       | K         |
    | B       | L         |
    | Start   | M         |
    | Select  | N         |
";

pub trait AppOption: Default + Clone + Copy + clap::ValueEnum {
    fn iter() -> impl Iterator<Item = Self>;
    fn str(self) -> &'static str;
}

#[derive(Default, Clone, Copy, clap::ValueEnum)]
enum Model {
    #[default]
    Cgb,
    Dmg,
    Mgb,
}

impl AppOption for Model {
    fn iter() -> impl Iterator<Item = Self> {
        [Self::Dmg, Self::Mgb, Self::Cgb].into_iter()
    }

    fn str(self) -> &'static str {
        match self {
            Self::Dmg => "dmg",
            Self::Mgb => "mgb",
            Self::Cgb => "cgb",
        }
    }
}

impl From<Model> for ceres_core::Model {
    #[inline]
    fn from(model: Model) -> Self {
        match model {
            Model::Dmg => Self::Dmg,
            Model::Mgb => Self::Mgb,
            Model::Cgb => Self::Cgb,
        }
    }
}

impl AppOption for ShaderOption {
    #[inline]
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

    #[inline]
    fn str(self) -> &'static str {
        match self {
            Self::Nearest => "nearest",
            Self::Scale2x => "scale2x",
            Self::Scale3x => "scale3x",
            Self::Lcd => "lcd",
            Self::Crt => "crt",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, clap::ValueEnum)]
pub enum PixelPerfectOption {
    PixelPerfect,
    #[default]
    Stretch,
}

impl AppOption for PixelPerfectOption {
    #[inline]
    fn iter() -> impl Iterator<Item = Self> {
        [Self::PixelPerfect, Self::Stretch].into_iter()
    }

    #[inline]
    fn str(self) -> &'static str {
        match self {
            Self::PixelPerfect => "pixel-perfect",
            Self::Stretch => "stretch",
        }
    }
}

#[derive(clap::Parser, Default)]
#[command(name = CERES_BIN, about = ABOUT, after_help = AFTER_HELP)]
pub struct Cli {
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
    #[arg(short, long, help = "Pixel perfect mode")]
    pixel_perfect: bool,
    #[arg(
        short,
        long,
        help = "Shader used",
        default_value = ShaderOption::default().str(),
        value_enum,
        required = false
    )]
    shader_option: ShaderOption,
    #[arg(
        long,
        help = "Enable execution tracing (prints disassembled instructions and registers to stderr)",
        default_value_t = false
    )]
    trace: bool,
    #[arg(
        long,
        help = "Trace buffer size (number of instructions to keep)",
        default_value_t = 1000,
        value_parser = clap::value_parser!(u32).range(1..=100000)
    )]
    trace_buffer_size: u32,
    #[arg(long, help = "Enable trace buffer collection", default_value_t = false)]
    trace_enable: bool,
    #[arg(long, help = "Export trace to JSON file on exit", value_name = "FILE")]
    trace_export: Option<PathBuf>,
}

impl Cli {
    #[must_use]
    #[inline]
    pub fn file(&self) -> Option<&Path> {
        self.file.as_deref()
    }

    #[must_use]
    #[inline]
    pub fn model(&self) -> ceres_core::Model {
        self.model.into()
    }

    #[must_use]
    #[inline]
    pub const fn pixel_perfect(&self) -> bool {
        self.pixel_perfect
    }

    #[must_use]
    #[inline]
    pub const fn shader_option(&self) -> ShaderOption {
        self.shader_option
    }

    #[must_use]
    #[inline]
    pub const fn trace(&self) -> bool {
        self.trace
    }

    #[must_use]
    #[inline]
    pub const fn trace_buffer_size(&self) -> u32 {
        self.trace_buffer_size
    }

    #[must_use]
    #[inline]
    pub const fn trace_enable(&self) -> bool {
        self.trace_enable
    }

    #[must_use]
    #[inline]
    pub fn trace_export(&self) -> Option<&Path> {
        self.trace_export.as_deref()
    }
}
