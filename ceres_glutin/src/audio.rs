use {
    ceres_core::Sample,
    sdl2::{
        audio::{AudioQueue, AudioSpecDesired},
        Sdl,
    },
};

const FREQ: i32 = 48000;
const BUF_SIZE: usize = 1024;
const AUDIO_BUFFER_SIZE: usize = BUF_SIZE * 4;

pub struct Renderer {
    stream: AudioQueue<f32>,
    buf: [f32; AUDIO_BUFFER_SIZE],
    buf_pos: usize,
    freq: u32,
}

impl Renderer {
    pub fn new(sdl: &Sdl) -> Self {
        let audio_subsystem = sdl.audio().unwrap();

        let desired_spec = AudioSpecDesired {
            freq: Some(FREQ),
            channels: Some(2),
            samples: Some(BUF_SIZE as u16),
        };

        let queue = audio_subsystem.open_queue(None, &desired_spec).unwrap();

        let obtained_spec = queue.spec();
        let freq = obtained_spec.freq as u32;
        queue.resume();

        Self {
            stream: queue,
            buf: [f32::default(); AUDIO_BUFFER_SIZE],
            buf_pos: 0,
            freq,
        }
    }

    pub fn push_frame(&mut self, l: Sample, r: Sample) {
        let l = f32::from(l * 32) / 32768.0;
        let r = f32::from(r * 32) / 32768.0;

        self.buf[self.buf_pos] = l;
        self.buf[self.buf_pos + 1] = r;

        self.buf_pos += 2;

        if self.buf_pos == AUDIO_BUFFER_SIZE {
            self.buf_pos = 0;

            // we're running too fast, skip this batch
            if self.stream.size() / 4 > self.freq / 4 {
                return;
            }

            self.stream.queue_audio(&self.buf).unwrap();
        }
    }

    pub fn sample_rate(&self) -> u32 {
        self.freq
    }
}
