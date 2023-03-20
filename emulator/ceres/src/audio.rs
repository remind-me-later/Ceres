use anyhow::Context;

use {alloc::sync::Arc, ceres_core::Gb, parking_lot::Mutex};

const BUFFER_SIZE: cpal::FrameCount = 512;
const SAMPLE_RATE: i32 = 48000;

pub struct Renderer {
    _stream: cpal::Stream,
}

impl Renderer {
    pub fn new(gb: Arc<Mutex<Gb>>) -> anyhow::Result<Self> {
        use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

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

        Ok(Self { _stream: stream })
    }

    pub const fn sample_rate() -> i32 {
        SAMPLE_RATE
    }
}
