use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
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

impl RingBuffer {
    pub fn new() -> Self {
        Self {
            buffer: Arc::new(Mutex::new(Bounded::from(
                [Default::default(); RING_BUFFER_SIZE],
            ))),
        }
    }

    pub fn clone_buffer(&self) -> Arc<Mutex<Bounded<[ceres_core::Sample; RING_BUFFER_SIZE]>>> {
        Arc::clone(&self.buffer)
    }
}

impl ceres_core::AudioCallback for RingBuffer {
    fn audio_sample(&self, l: ceres_core::Sample, r: ceres_core::Sample) {
        if let Ok(mut buffer) = self.buffer.lock() {
            buffer.push(l);
            buffer.push(r);
        }
    }
}

pub struct State {
    _host: cpal::Host,
    device: cpal::Device,
    config: cpal::StreamConfig,
}

impl State {
    pub fn new() -> Self {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .expect("cpal couldn't get default output device");

        let config = cpal::StreamConfig {
            channels: 2,
            sample_rate: cpal::SampleRate(SAMPLE_RATE as u32),
            buffer_size: cpal::BufferSize::Fixed(BUFFER_SIZE),
        };

        Self {
            _host: host,
            device,
            config,
        }
    }

    pub fn device(&self) -> &cpal::Device {
        &self.device
    }

    pub fn config(&self) -> &cpal::StreamConfig {
        &self.config
    }
}

// Stream is not Send, so we can't use it directly in the renderer struct
pub struct Stream {
    stream: cpal::Stream,
    ring_buffer: RingBuffer,
}

impl Stream {
    pub fn new(state: &State) -> Self {
        let ring_buffer = RingBuffer::new();
        let ring_buffer_clone = ring_buffer.clone_buffer();

        let error_callback = |err| eprintln!("an AudioError occurred on stream: {err}");
        let data_callback = move |buffer: &mut [ceres_core::Sample], _: &_| {
            if let Ok(mut ring) = ring_buffer_clone.lock() {
                if ring.len() < buffer.len() {
                    eprintln!("ring buffer underrun");
                }

                buffer
                    .iter_mut()
                    .zip(ring.drain())
                    .for_each(|(b, s)| *b = s);
            }
        };

        let stream = state
            .device()
            .build_output_stream(state.config(), data_callback, error_callback, None)
            .expect("cpal couldn't build output stream");

        stream.pause().expect("couldn't pause stream");

        Self {
            stream,
            ring_buffer,
        }
    }

    pub fn get_ring_buffer(&self) -> RingBuffer {
        self.ring_buffer.clone()
    }

    pub fn pause(&mut self) {
        self.stream.pause().expect("couldn't pause stream")
    }

    pub fn resume(&mut self) {
        self.stream.play().expect("couldn't resume stream")
    }

    pub const fn sample_rate() -> i32 {
        SAMPLE_RATE
    }
}
