mod audio;
mod cli;
mod thread;

#[cfg(feature = "wgpu_renderer")]
pub mod wgpu_renderer;

#[cfg(feature = "game_genie")]
pub use ceres_core::GameGenieCode;

pub use ceres_core::{Button, Model, PX_HEIGHT, PX_WIDTH};
pub use ceres_core::ColorCorrectionMode;
pub use clap;
pub use cli::{
    AppOption, CERES_BIN, CERES_STYLIZED, Cli, ORGANIZATION, PixelPerfectOption, QUALIFIER,
    ShaderOption,
};
pub use thread::{Error, GbThread, PainterCallback, Pressable};

pub const PIXEL_BUFFER_SIZE: usize = 4 * PX_WIDTH as usize * PX_HEIGHT as usize;
