use cpal::traits::StreamTrait;

use {alloc::sync::Arc, ceres_core::Gb, parking_lot::Mutex};

const BUFFER_SIZE: cpal::FrameCount = 512;
const SAMPLE_RATE: i32 = 48000;

pub struct Renderer {
    stream: cpal::Stream,
    volume: Arc<Mutex<f32>>,
}

impl Renderer {
    pub fn new(gb: Arc<Mutex<Gb>>) -> Self {
        use cpal::traits::{DeviceTrait, HostTrait};

        let host = cpal::default_host();
        let dev = host
            .default_output_device()
            .expect("cpal couldn't get default output device");

        let config = cpal::StreamConfig {
            channels: 2,
            sample_rate: cpal::SampleRate(SAMPLE_RATE as u32),
            buffer_size: cpal::BufferSize::Fixed(BUFFER_SIZE),
        };

        let volume = Arc::new(Mutex::new(1.0));

        let stream = {
            let volume = Arc::clone(&volume);
            let error_callback = |err| eprintln!("an AudioError occurred on stream: {err}");
            let data_callback = move |b: &mut [f32], _: &_| {
                let volume = *volume.lock();
                let mut gb = gb.lock();

                b.chunks_exact_mut(2).for_each(|w| {
                    let (l, r) = gb.run_samples();
                    w[0] = (l as f32 / i16::MAX as f32) * volume;
                    w[1] = (r as f32 / i16::MAX as f32) * volume;
                });
            };

            dev.build_output_stream(&config, data_callback, error_callback, None)
                .unwrap()
        };

        stream.play().unwrap();

        Self { stream, volume }
    }

    pub fn pause(&mut self) {
        self.stream.pause().unwrap();
    }

    pub fn resume(&mut self) {
        self.stream.play().unwrap();
    }

    pub fn volume(&self) -> &Arc<Mutex<f32>> {
        &self.volume
    }

    pub const fn sample_rate() -> i32 {
        SAMPLE_RATE
    }
}
