use anyhow::Context;
use cpal::traits::StreamTrait;
use dasp_ring_buffer::Bounded;

use {alloc::sync::Arc, std::sync::Mutex};

// Buffer size is the number of samples per channel per callback
const BUFFER_SIZE: cpal::FrameCount = 1024;
const RING_BUFFER_SIZE: usize = BUFFER_SIZE as usize * 8;
const SAMPLE_RATE: i32 = 48000;

// RingBuffer is a wrapper around a bounded ring buffer
// that implements the AudioCallback trait
#[derive(Clone)]
pub struct RingBuffer {
    buffer: Arc<Mutex<Bounded<[f32; RING_BUFFER_SIZE]>>>,
}

impl ceres_core::AudioCallback for RingBuffer {
    fn audio_sample(&self, l: ceres_core::Sample, r: ceres_core::Sample) {
        if let Ok(mut buffer) = self.buffer.lock() {
            buffer.push(l as f32 / i16::MAX as f32);
            buffer.push(r as f32 / i16::MAX as f32);
        }
    }
}

// Stream is not Send, so we can't use it directly in the renderer struct
pub struct Renderer {
    stream: cpal::Stream,
    paused: bool,
    ring_buffer: RingBuffer,
}

impl Renderer {
    pub fn new() -> anyhow::Result<Self> {
        use cpal::traits::{DeviceTrait, HostTrait};

        let host = cpal::default_host();
        let dev = host
            .default_output_device()
            .context("cpal couldn't get default output device")?;

        let config = cpal::StreamConfig {
            channels: 2,
            sample_rate: cpal::SampleRate(SAMPLE_RATE as u32),
            buffer_size: cpal::BufferSize::Fixed(BUFFER_SIZE),
        };

        let ring_buffer = Arc::new(Mutex::new(Bounded::from([0.0; RING_BUFFER_SIZE])));
        let ring_buffer_clone = Arc::clone(&ring_buffer);

        let error_callback = |err| eprintln!("an AudioError occurred on stream: {err}");
        let data_callback = move |b: &mut [f32], _: &_| {
            if let Ok(mut ring_buffer) = ring_buffer_clone.lock() {
                // TODO: resampling
                if ring_buffer.len() < b.len() {
                    eprintln!("ring buffer underrun");
                }

                b.iter_mut()
                    .zip(ring_buffer.drain())
                    .for_each(|(b, s)| *b = s);
            }
        };

        let stream = dev.build_output_stream(&config, data_callback, error_callback, None)?;

        stream.play()?;

        Ok(Self {
            stream,
            paused: false,
            ring_buffer: RingBuffer {
                buffer: ring_buffer,
            },
        })
    }

    pub fn get_ring_buffer(&self) -> RingBuffer {
        self.ring_buffer.clone()
    }

    pub fn pause(&mut self) -> anyhow::Result<()> {
        self.stream.pause().context("couldn't pause stream")
    }

    pub fn resume(&mut self) -> anyhow::Result<()> {
        self.stream.play().context("couldn't resume stream")
    }

    pub fn toggle(&mut self) -> anyhow::Result<()> {
        if self.paused {
            self.resume()?;
            self.paused = false;
        } else {
            self.pause()?;
            self.paused = true;
        }

        Ok(())
    }

    pub const fn sample_rate() -> i32 {
        SAMPLE_RATE
    }
}
