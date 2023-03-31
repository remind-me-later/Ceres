use alloc::sync::Arc;
use ceres_core::Gb;
use parking_lot::Mutex;
use sdl2::{
    audio::{AudioCallback, AudioDevice, AudioSpecDesired},
    Sdl,
};

const BUFFER_SIZE: u16 = 512;
const SAMPLE_RATE: i32 = 48000;

struct Cb {
    gb: Arc<Mutex<Gb>>,
}

impl AudioCallback for Cb {
    type Channel = ceres_core::Sample;

    fn callback(&mut self, b: &mut [Self::Channel]) {
        let mut gb = self.gb.lock();

        let mut i = 0;
        let len = b.len();

        while i < len {
            let (l, r) = gb.run_samples();
            b[i] = l;
            b[i + 1] = r;

            i += 2;
        }
    }
}

pub struct Renderer {
    device: AudioDevice<Cb>,
}

impl Renderer {
    pub fn new(sdl_context: &Sdl, gb: Arc<Mutex<Gb>>) -> Self {
        let audio_subsystem = sdl_context.audio().unwrap();

        let desired_spec = AudioSpecDesired {
            freq: Some(SAMPLE_RATE),
            channels: Some(2),          // mono
            samples: Some(BUFFER_SIZE), // default sample size
        };

        let device = audio_subsystem
            .open_playback(None, &desired_spec, |_| Cb { gb })
            .unwrap();

        // Start playback
        device.resume();

        Self { device }
    }

    #[allow(dead_code)]
    pub fn resume(&mut self) {
        self.device.resume();
    }

    #[allow(dead_code)]
    pub fn pause(&mut self) {
        self.device.pause();
    }

    #[inline]
    pub fn sample_rate() -> i32 {
        SAMPLE_RATE
    }
}
