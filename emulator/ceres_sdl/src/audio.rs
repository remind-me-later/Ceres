use std::sync::Mutex;

use ceres_core::Gb;
use cpal::{BufferSize, SampleRate, StreamConfig};
use {
    cpal::traits::{DeviceTrait, HostTrait, StreamTrait},
    std::sync::Arc,
};

const BUFFER_SIZE: cpal::FrameCount = 512;
const SAMPLE_RATE: i32 = 48000;

pub struct Renderer {
    stream: cpal::Stream,
}

impl Renderer {
    pub fn new(gb: Arc<Mutex<Gb>>) -> Self {
        let host = cpal::default_host();
        let dev = host.default_output_device().unwrap();

        let config = StreamConfig {
            channels: 2,
            sample_rate: SampleRate(SAMPLE_RATE as u32),
            buffer_size: BufferSize::Fixed(BUFFER_SIZE),
        };

        let error_callback = |err| panic!("an AudioError occurred on stream: {err}");
        let data_callback = move |output: &mut [f32], _: &_| {
            fn tof32(s: i16) -> f32 {
                f32::from(s) / f32::from(i16::MAX)
            }

            // fn high_pass(capacitor: &mut f32, s: f32) -> f32 {
            //     let out: f32 = s - *capacitor;
            //     *capacitor = s - out * 0.999_958; // use 0.998943 for MGB&CGB

            //     out
            // }

            if let Ok(mut gb) = gb.lock() {
                let mut i = 0;
                let len = output.len();

                while i < len {
                    let (l, r) = gb.run_samples();
                    output[i] = tof32(l);
                    output[i + 1] = tof32(r);

                    i += 2;
                }
            }
        };

        let stream = dev
            .build_output_stream(&config, data_callback, error_callback)
            .unwrap();

        stream.play().expect("AudioError playing sound");

        Self { stream }
    }

    #[allow(dead_code)]
    pub fn resume(&mut self) {
        self.stream.play().unwrap();
    }

    #[allow(dead_code)]
    pub fn pause(&mut self) {
        self.stream.pause().unwrap();
    }

    #[inline]
    pub fn sample_rate() -> i32 {
        SAMPLE_RATE
    }
}
