use sdl2::{
    audio::{AudioQueue, AudioSpecDesired},
    Sdl,
};

pub struct AudioRenderer {
    stream: AudioQueue<f32>,
}

impl AudioRenderer {
    pub fn new(sdl_context: &Sdl) -> Self {
        let audio_subsystem = sdl_context.audio().unwrap();

        let desired_spec = AudioSpecDesired {
            freq: Some(48000),
            channels: Some(2),
            samples: Some(512),
        };

        let queue = audio_subsystem.open_queue(None, &desired_spec).unwrap();

        queue.resume();

        Self { stream: queue }
    }

    pub fn play(&mut self) {
        self.stream.resume()
    }

    pub fn pause(&mut self) {
        self.stream.pause()
    }
}

impl ceres_core::AudioCallbacks for AudioRenderer {
    fn sample_rate(&self) -> u32 {
        48000
    }

    fn push_frame(&mut self, frame: ceres_core::Frame) {
        // TODO: why?
        if self.stream.size() > 48000 {
            return;
        }

        self.stream
            .queue_audio(&[frame.left(), frame.right()])
            .unwrap()
    }
}
