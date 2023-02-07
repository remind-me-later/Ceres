use ceres_core::Gb;
use sdl2::{
    audio::{AudioCallback, AudioDevice, AudioSpecDesired},
    Sdl,
};
use std::sync::Arc;
use std::sync::Mutex;

const BUFFER_SIZE: u16 = 1024;
const SAMPLE_RATE: i32 = 48000;

struct Cb {
    gb: Arc<Mutex<Gb>>,
}

impl AudioCallback for Cb {
    type Channel = ceres_core::Sample;

    fn callback(&mut self, b: &mut [Self::Channel]) {
        if let Ok(mut gb) = self.gb.lock() {
            b.chunks_exact_mut(2).for_each(|w| {
                let (l, r) = gb.run_samples();
                w[0] = l;
                w[1] = r;
            });
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
            channels: Some(2),
            samples: Some(BUFFER_SIZE),
        };

        let device = audio_subsystem
            .open_playback(None, &desired_spec, |_| Cb { gb })
            .unwrap();

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
