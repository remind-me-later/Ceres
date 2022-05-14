use sdl2::{
    audio::{AudioQueue, AudioSpecDesired},
    Sdl,
};

const FREQ: u32 = 41800;
const AUDIO_BUFFER_SIZE: usize = 512 * 2;

pub struct Renderer {
    stream: AudioQueue<f32>,
    buf: [ceres_core::Sample; AUDIO_BUFFER_SIZE],
    buf_pos: usize,
}

impl Renderer {
    pub fn new(sdl: &Sdl) -> Self {
        let audio_subsystem = sdl.audio().unwrap();

        let desired_spec = AudioSpecDesired {
            freq: Some(FREQ as i32),
            channels: Some(2),
            samples: Some(512),
        };

        let queue = audio_subsystem.open_queue(None, &desired_spec).unwrap();

        queue.resume();

        Self {
            stream: queue,
            buf: [ceres_core::Sample::default(); AUDIO_BUFFER_SIZE],
            buf_pos: 0,
        }
    }
}

impl ceres_core::AudioCallbacks for Renderer {
    fn sample_rate(&self) -> u32 {
        FREQ
    }

    fn push_frame(&mut self, frame: ceres_core::Frame) {
        // TODO: wtf???
        if self.stream.size() > FREQ {
            return;
        }

        self.buf[self.buf_pos] = frame.left();
        self.buf[self.buf_pos + 1] = frame.right();
        self.buf_pos += 2;

        if self.buf_pos == AUDIO_BUFFER_SIZE {
            self.stream.queue_audio(&self.buf).unwrap();
            self.buf_pos = 0;
        }
    }
}
