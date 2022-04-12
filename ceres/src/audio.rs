use sdl2::{
    audio::{AudioQueue, AudioSpecDesired},
    Sdl,
};
use std::rc::Rc;

pub struct AudioRenderer {
    stream: Rc<AudioQueue<f32>>,
}

impl AudioRenderer {
    pub fn new(sdl_context: &Sdl) -> (Self, AudioCallbacks) {
        let audio_subsystem = sdl_context.audio().unwrap();

        let desired_spec = AudioSpecDesired {
            freq: Some(48000),
            channels: Some(2),
            samples: Some(512),
        };

        let queue = Rc::new(audio_subsystem.open_queue(None, &desired_spec).unwrap());
        let queue_copy = Rc::clone(&queue);

        queue.resume();

        (Self { stream: queue }, AudioCallbacks { queue: queue_copy })
    }

    pub fn play(&mut self) {
        self.stream.resume()
    }

    pub fn pause(&mut self) {
        self.stream.pause()
    }
}

pub struct AudioCallbacks {
    queue: Rc<AudioQueue<f32>>,
}

impl ceres_core::AudioCallbacks for AudioCallbacks {
    fn sample_rate(&self) -> u32 {
        48000
    }

    fn push_frame(&mut self, frame: ceres_core::Frame) {
        // TODO: why?
        if self.queue.as_ref().size() > 48000 / 2 {
            return;
        }

        self.queue
            .as_ref()
            .queue_audio(&[frame.left(), frame.right()])
            .unwrap()
    }
}
