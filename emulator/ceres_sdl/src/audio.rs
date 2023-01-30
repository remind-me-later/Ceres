use std::sync::Mutex;

use cpal::{BufferSize, SampleRate, StreamConfig};
use {
    cpal::traits::{DeviceTrait, HostTrait, StreamTrait},
    dasp_ring_buffer::Bounded,
    std::sync::Arc,
};

const BUFFER_SIZE: cpal::FrameCount = 512;
const RING_BUFFER_SIZE: usize = BUFFER_SIZE as usize * 32;
const SAMPLE_RATE: i32 = 48000;

pub struct Renderer {
    ring_buffer: Arc<Mutex<Bounded<Box<[f32]>>>>,
    stream: cpal::Stream,
    capacitor: f32,
}

impl Renderer {
    pub fn new() -> Self {
        let host = cpal::default_host();
        let dev = host.default_output_device().unwrap();

        let config = StreamConfig {
            channels: 2,
            sample_rate: SampleRate(SAMPLE_RATE as u32),
            buffer_size: BufferSize::Fixed(BUFFER_SIZE),
        };

        let ring_buffer = Arc::new(Mutex::new(Bounded::from(
            vec![Default::default(); RING_BUFFER_SIZE].into_boxed_slice(),
        )));

        let error_callback = |err| panic!("an AudioError occurred on stream: {err}");
        let ring_buffer_arc = Arc::clone(&ring_buffer);
        let data_callback = move |output: &mut [f32], _: &_| {
            if let Ok(mut buf) = ring_buffer_arc.lock() {
                if buf.len() < output.len() {
                    println!("underrun");
                }

                output
                    .iter_mut()
                    .zip(buf.drain())
                    .for_each(|(out_sample, gb_sample)| *out_sample = gb_sample);
            }
        };

        let stream = dev
            .build_output_stream(&config, data_callback, error_callback)
            .unwrap();

        stream.play().expect("AudioError playing sound");

        Self {
            ring_buffer,
            stream,
            capacitor: 0.0,
        }
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
    #[allow(clippy::unused_self)]
    pub fn sample_rate(&self) -> i32 {
        SAMPLE_RATE
    }

    pub fn push_frame(&mut self, l: ceres_core::Sample, r: ceres_core::Sample) {
        fn tof32(s: i16) -> f32 {
            f32::from(s) / f32::from(i16::MAX)
        }

        fn high_pass(capacitor: &mut f32, s: f32) -> f32 {
            let out: f32 = s - *capacitor;

            // capacitor slowly charges to 'in' via their difference
            *capacitor = s - out * 0.999_958; // use 0.998943 for MGB&CGB

            out
        }

        if let Ok(mut buf) = self.ring_buffer.lock() {
            // if buf.is_full() {
            //     return;
            // }

            let l = high_pass(&mut self.capacitor, tof32(l));
            let r = high_pass(&mut self.capacitor, tof32(r));

            buf.push(l);
            buf.push(r);
        }
    }
}

impl ceres_core::Audio for Renderer {
    fn play(&mut self, l: ceres_core::Sample, r: ceres_core::Sample) {
        self.push_frame(l, r);
    }
}
