use anyhow::Context;
use core::sync::atomic::AtomicBool;
use core::time::Duration;
use cpal::traits::StreamTrait;
use dasp_ring_buffer::Bounded;
use std::sync::Condvar;
use {alloc::sync::Arc, std::sync::Mutex};

// Buffer size is the number of samples per channel per callback
const BUFFER_SIZE: cpal::FrameCount = 512;
const RING_BUFFER_SIZE: usize = BUFFER_SIZE as usize * 2;
const SAMPLE_RATE: i32 = 48000;

// RingBuffer is a wrapper around a bounded ring buffer
// that implements the AudioCallback trait
#[derive(Clone)]
pub struct RingBuffer {
    exiting: Arc<AtomicBool>,
    buffer: Arc<(
        Mutex<Bounded<[ceres_core::Sample; RING_BUFFER_SIZE]>>,
        Condvar,
    )>,
}

impl ceres_core::AudioCallback for RingBuffer {
    fn audio_sample(&self, l: ceres_core::Sample, r: ceres_core::Sample) {
        // All of this to avoid overrunning the buffer
        let (buffer, cvar) = &*self.buffer;
        if let Ok(mut buffer) = buffer.lock() {
            while buffer.is_full() {
                if self.exiting.load(core::sync::atomic::Ordering::Relaxed) {
                    return;
                }

                // Wait a little longer than a frame
                buffer = cvar
                    .wait_timeout(buffer, Duration::new(0, 20_000_000))
                    .unwrap()
                    .0;
            }

            buffer.push(l);
            buffer.push(r);
        }
    }
}

// Stream is not Send, so we can't use it directly in the renderer struct
pub struct Renderer {
    stream: cpal::Stream,
    ring_buffer: RingBuffer,
}

impl Renderer {
    pub fn new(exiting: Arc<AtomicBool>) -> anyhow::Result<Self> {
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

        let ring_buffer = Arc::new((
            Mutex::new(Bounded::from([Default::default(); RING_BUFFER_SIZE])),
            Condvar::new(),
        ));
        let ring_buffer_clone = Arc::clone(&ring_buffer);

        let error_callback = |err| eprintln!("an AudioError occurred on stream: {err}");
        let data_callback = move |buffer: &mut [ceres_core::Sample], _: &_| {
            let (rb, cvar) = &*ring_buffer_clone;
            if let Ok(mut ring) = rb.lock() {
                if ring.len() < buffer.len() {
                    eprintln!("ring buffer underrun");
                }

                buffer
                    .iter_mut()
                    .zip(ring.drain())
                    .for_each(|(b, s)| *b = s);

                cvar.notify_one();
            }
        };

        let stream = dev.build_output_stream(&config, data_callback, error_callback, None)?;

        stream.pause().context("couldn't pause stream")?;

        Ok(Self {
            stream,
            ring_buffer: RingBuffer {
                exiting,
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

    pub const fn sample_rate() -> i32 {
        SAMPLE_RATE
    }
}
