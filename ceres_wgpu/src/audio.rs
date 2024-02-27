use anyhow::Context;
use cpal::traits::StreamTrait;

use {alloc::sync::Arc, ceres_core::Gb, parking_lot::Mutex};

// Buffer size is the number of samples per channel per callback
const BUFFER_SIZE: cpal::FrameCount = 512;
const SAMPLE_RATE: i32 = 48000;

// Stream is not Send, so we can't use it directly in the renderer struct
pub struct Renderer {
    stream: cpal::Stream,
    paused: bool,
}

impl Renderer {
    pub fn new(gb: Arc<Mutex<Gb>>) -> anyhow::Result<Self> {
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

        let error_callback = |err| eprintln!("an AudioError occurred on stream: {err}");
        let data_callback = move |b: &mut [ceres_core::Sample], _: &_| {
            let mut gb = gb.lock();

            b.chunks_exact_mut(2).for_each(|w| {
                let (l, r) = gb.run_samples();
                w[0] = l;
                w[1] = r;
            });
        };

        let stream = dev.build_output_stream(&config, data_callback, error_callback, None)?;

        stream.play()?;

        Ok(Self {
            stream,
            paused: false,
        })
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
