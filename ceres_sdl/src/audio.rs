use {
    ceres_core::Sample,
    sdl2::{
        audio::{AudioQueue, AudioSpecDesired},
        Sdl,
    },
    std::ptr,
};

const FREQ: u32 = 96000;
const AUDIO_BUFFER_SIZE: usize = 512 * 2;

static mut AREND: *mut Renderer = ptr::null_mut();

pub struct Renderer {
    stream: AudioQueue<f32>,
    buf: [ceres_core::Sample; AUDIO_BUFFER_SIZE],
    buf_pos: usize,
}

impl Renderer {
    pub fn new(sdl: &Sdl) -> Box<Self> {
        let audio_subsystem = sdl.audio().unwrap();

        let desired_spec = AudioSpecDesired {
            freq: Some(FREQ as i32),
            channels: Some(2),
            samples: Some(512),
        };

        let queue = audio_subsystem.open_queue(None, &desired_spec).unwrap();

        queue.resume();

        let mut rend = Box::new(Self {
            stream: queue,
            buf: [ceres_core::Sample::default(); AUDIO_BUFFER_SIZE],
            buf_pos: 0,
        });

        unsafe {
            AREND = rend.as_mut();
        }

        rend
    }

    pub fn sample_rate(&self) -> u32 {
        FREQ
    }
}

pub fn apu_frame_callback(left: Sample, right: Sample) {
    unsafe {
        if (*AREND).stream.size() > FREQ / 2 {
            return;
        }

        (*AREND).buf[(*AREND).buf_pos] = left;
        (*AREND).buf[(*AREND).buf_pos + 1] = right;
        (*AREND).buf_pos += 2;

        if (*AREND).buf_pos == AUDIO_BUFFER_SIZE {
            (*AREND).stream.queue_audio(&(*AREND).buf).unwrap();
            (*AREND).buf_pos = 0;
        }
    }
}
