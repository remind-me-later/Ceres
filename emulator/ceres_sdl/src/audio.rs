use std::sync::Mutex;

use cpal::{BufferSize, StreamConfig};
use {
    ceres_core::Sample,
    cpal::{
        traits::{DeviceTrait, HostTrait, StreamTrait},
        SampleRate,
    },
    dasp_ring_buffer::Bounded,
    std::sync::Arc,
};

const BUFFER_SIZE: cpal::FrameCount = 512 * 2;
const RING_BUFFER_SIZE: usize = BUFFER_SIZE as usize * 16;
const SAMPLE_RATE: i32 = 48000;

pub struct Renderer {
    ring_buffer: Arc<Mutex<Bounded<Box<[Sample]>>>>,
    stream: cpal::Stream,
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
        let data_callback = move |output: &mut [Sample], _: &_| {
            if let Ok(mut buf) = ring_buffer_arc.lock() {
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

    pub fn push_frame(&mut self, l: Sample, r: Sample) {
        if let Ok(mut buf) = self.ring_buffer.lock() {
            buf.push(l);
            buf.push(r);
        }
    }
}

impl ceres_core::Audio for Renderer {
    fn play(&mut self, l: Sample, r: Sample) {
        self.push_frame(l, r);
    }
}
