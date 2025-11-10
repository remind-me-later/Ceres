mod audio;
mod thread;
pub mod trace_export;

#[cfg(feature = "cli")]
pub mod cli;

#[cfg(feature = "wgpu_renderer")]
pub mod wgpu_renderer;

#[cfg(feature = "game_genie")]
pub use ceres_core::GameGenieCode;

pub use ceres_core::ColorCorrectionMode;
pub use ceres_core::{Button, Model, PX_HEIGHT, PX_WIDTH};
pub use thread::{Error, GbThread, Pressable};

pub const PIXEL_BUFFER_SIZE: usize = 4 * PX_WIDTH as usize * PX_HEIGHT as usize;

#[derive(Default, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
pub enum ShaderOption {
    Crt,
    Lcd,
    #[default]
    Nearest,
    Scale2x,
    Scale3x,
}
