use cpal::traits::StreamTrait;
use dasp_ring_buffer::Bounded;

use {std::sync::Arc, std::sync::Mutex};

// Buffer size is the number of samples per channel per callback
const BUFFER_SIZE: cpal::FrameCount = 512;
const RING_BUFFER_SIZE: usize = BUFFER_SIZE as usize * 8;
const SAMPLE_RATE: i32 = 48000;

// RingBuffer is a wrapper around a bounded ring buffer
// that implements the AudioCallback trait
#[derive(Clone)]
pub struct RingBuffer {
    buffer: Arc<Mutex<Bounded<[ceres_core::Sample; RING_BUFFER_SIZE]>>>,
}

impl ceres_core::AudioCallback for RingBuffer {
    fn audio_sample(&self, l: ceres_core::Sample, r: ceres_core::Sample) {
        if let Ok(mut buffer) = self.buffer.lock() {
            buffer.push(l);
            buffer.push(r);
        }
    }
}

// Stream is not Send, so we can't use it directly in the renderer struct
pub struct Renderer {
    stream: cpal::Stream,
    volume: Arc<Mutex<f32>>,
    ring_buffer: RingBuffer,
}

impl Default for Renderer {
    fn default() -> Self {
        Self::new()
    }
}

impl Renderer {
    pub fn new() -> Self {
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

        let ring_buffer = Arc::new(Mutex::new(Bounded::from(
            [Default::default(); RING_BUFFER_SIZE],
        )));
        let ring_buffer_clone = Arc::clone(&ring_buffer);
        let volume = Arc::new(Mutex::new(1.0));
        let volume_clone = Arc::clone(&volume);

        let error_callback = |err| eprintln!("an AudioError occurred on stream: {err}");
        let data_callback = move |buffer: &mut [ceres_core::Sample], _: &_| {
            if let Ok(mut ring) = ring_buffer_clone.lock() {
                let vol = *volume_clone.lock().unwrap();

                if ring.len() < buffer.len() {
                    eprintln!("ring buffer underrun");
                }

                buffer
                    .iter_mut()
                    .zip(ring.drain())
                    .for_each(|(b, s)| *b = s * vol);
            }
        };

        let stream = dev
            .build_output_stream(&config, data_callback, error_callback, None)
            .unwrap();

        stream.pause().unwrap();

        Self {
            stream,
            volume,
            ring_buffer: RingBuffer {
                buffer: ring_buffer,
            },
        }
    }

    pub fn get_ring_buffer(&self) -> RingBuffer {
        self.ring_buffer.clone()
    }

    pub fn pause(&self) {
        self.stream.pause().unwrap();
    }

    pub fn resume(&self) {
        self.stream.play().unwrap();
    }

    pub const fn volume(&self) -> &Arc<Mutex<f32>> {
        &self.volume
    }

    pub const fn sample_rate() -> i32 {
        SAMPLE_RATE
    }
}
