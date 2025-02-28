mod audio;
mod thread;

pub use audio::{AudioState, Error as AudioError, Stream};
pub use thread::{Error as ThreadError, GbThread, PainterCallback};

pub type Gb = ceres_core::Gb<audio::AudioCallbackImpl>;
