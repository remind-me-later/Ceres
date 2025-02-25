mod audio;
mod thread;

pub use audio::{Error as AudioError, AudioState, Stream};
pub use thread::{Error as ThreadError, GbThread, PainterCallback};

pub type Gb = ceres_core::Gb<audio::AudioCallbackImpl>;
