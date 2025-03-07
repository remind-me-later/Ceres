mod audio;
mod thread;

pub use ceres_core::Button;
pub use ceres_core::Model;
pub use ceres_core::PX_HEIGHT;
pub use ceres_core::PX_WIDTH;

pub use thread::{Error, GbThread, PainterCallback};

pub const PIXEL_BUFFER_SIZE: usize = 4 * PX_WIDTH as usize * PX_HEIGHT as usize;
