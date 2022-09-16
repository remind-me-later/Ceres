use {
    ceres_core::Sample,
    sdl2::{
        audio::{AudioQueue, AudioSpecDesired},
        Sdl,
    },
};

const FREQ: i32 = 48000;
const BUF_SIZE: u16 = 512 * 2;

pub struct Renderer {
    stream: AudioQueue<f32>,
    buf: [f32; BUF_SIZE as usize],
    buf_pos: usize,
}

impl Renderer {
    pub fn new(sdl: &Sdl) -> Self {
        let audio_subsystem = sdl.audio().unwrap();

        let desired_spec = AudioSpecDesired {
            freq: Some(FREQ),
            channels: Some(2),
            samples: Some(BUF_SIZE),
        };

        let queue = audio_subsystem.open_queue(None, &desired_spec).unwrap();

        queue.resume();

        Self {
            stream: queue,
            buf: [Default::default(); BUF_SIZE as usize],
            buf_pos: 0,
        }
    }

    #[inline]
    pub fn push_frame(&mut self, l: Sample, r: Sample) {
        let l = f32::from(l * 32) / 32768.0;
        let r = f32::from(r * 32) / 32768.0;

        self.buf[self.buf_pos] = l;
        self.buf[self.buf_pos + 1] = r;

        self.buf_pos += 2;

        if self.buf_pos == BUF_SIZE as usize {
            self.buf_pos = 0;
            self.stream.queue_audio(&self.buf).unwrap();
        }
    }

    #[inline]
    #[allow(clippy::unused_self)]
    pub fn sample_rate(&self) -> i32 {
        FREQ
    }
}

impl ceres_core::Audio for Renderer {
    fn play(&mut self, l: Sample, r: Sample) {
        self.push_frame(l, r);
    }
}
