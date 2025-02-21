use std::vec;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use dasp_ring_buffer::Slice;
use rubato::Resampler;
use {std::sync::Arc, std::sync::Mutex};

// Buffer size is the number of samples per channel per callback
const BUFFER_SIZE: cpal::FrameCount = 512;
const RING_BUFFER_SIZE: usize = BUFFER_SIZE as usize * 16;
const SAMPLE_RATE: i32 = 48000;

#[derive(Debug)]
struct Buffers {
    // lr: Bounded<Box<[(ceres_core::Sample, ceres_core::Sample)]>>,
    left: Vec<ceres_core::Sample>,
    right: Vec<ceres_core::Sample>,
}

impl Buffers {
    fn new() -> Self {
        Self {
            left: vec![Default::default(); RING_BUFFER_SIZE],
            right: vec![Default::default(); RING_BUFFER_SIZE],
        }
    }

    fn remove_first(&mut self, n: usize) {
        self.left.drain(0..n);
        self.right.drain(0..n);
    }
}

// RingBuffer is a wrapper around a bounded ring buffer
// that implements the AudioCallback trait
#[derive(Clone, Debug)]
pub struct RingBuffer {
    buffer: Arc<Mutex<Buffers>>,
    volume: Arc<Mutex<f32>>,
}

impl RingBuffer {
    fn new(buffer: Arc<Mutex<Buffers>>, volume: Arc<Mutex<f32>>) -> Self {
        Self { buffer, volume }
    }
}

impl ceres_core::AudioCallback for RingBuffer {
    fn audio_sample(&self, l: ceres_core::Sample, r: ceres_core::Sample) {
        if let Ok(mut buffer) = self.buffer.lock() {
            if buffer.left.len() >= RING_BUFFER_SIZE {
                // println!("ring buffer full");
                return;
            }

            if let Ok(volume) = self.volume.lock() {
                let l = l * *volume;
                let r = r * *volume;

                buffer.left.push(l);
                buffer.right.push(r);
            }
        }
    }
}

pub struct State {
    _host: cpal::Host,
    device: cpal::Device,
    config: cpal::StreamConfig,
}

impl State {
    pub fn new() -> Result<Self, Error> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or(Error::CouldntGetOutputDevice)?;

        let config = cpal::StreamConfig {
            channels: 2,
            sample_rate: cpal::SampleRate(SAMPLE_RATE as u32),
            buffer_size: cpal::BufferSize::Fixed(BUFFER_SIZE),
        };

        Ok(Self {
            _host: host,
            device,
            config,
        })
    }

    #[must_use]
    pub fn device(&self) -> &cpal::Device {
        &self.device
    }

    #[must_use]
    pub fn config(&self) -> &cpal::StreamConfig {
        &self.config
    }
}

// Stream is not Send, so we can't use it directly in the renderer struct
pub struct Stream {
    stream: cpal::Stream,
    ring_buffer: RingBuffer,
    volume: Arc<Mutex<f32>>,
}

impl Stream {
    pub fn new(state: &State) -> Result<Self, Error> {
        let ring_buffer = Arc::new(Mutex::new(Buffers::new()));

        let ring_buffer_clone = Arc::clone(&ring_buffer);

        let mut resampler = rubato::FastFixedOut::<ceres_core::Sample>::new(
            1.044,
            2.0,
            rubato::PolynomialDegree::Cubic,
            BUFFER_SIZE as usize,
            2,
        )
        .map_err(|_err| Error::CouldntBuildStream)?;

        let mut output_buf: [[ceres_core::Sample; BUFFER_SIZE as usize]; 2] =
            [[Default::default(); BUFFER_SIZE as usize]; 2];

        let error_callback = |err| eprintln!("an AudioError occurred on stream: {err}");
        let data_callback = move |buffer: &mut [ceres_core::Sample], _: &_| {
            if let Ok(mut ring) = ring_buffer_clone.lock() {
                match resampler.process_into_buffer(
                    &[ring.left.slice(), ring.right.slice()],
                    &mut output_buf,
                    None,
                ) {
                    Ok((consumed_in, _written_out)) => {
                        ring.remove_first(consumed_in);
                        buffer
                            .chunks_exact_mut(2)
                            .zip(output_buf[0].iter().zip(output_buf[1].iter()))
                            .for_each(|(out, (&sample_l, &sample_r))| {
                                out[0] = sample_l;
                                out[1] = sample_r;
                            });
                    }
                    Err(e) => {
                        eprintln!("resampler error, possible underrun: {e}");
                    }
                }
            }
        };

        let stream = state
            .device()
            .build_output_stream(state.config(), data_callback, error_callback, None)
            .map_err(|_err| Error::CouldntBuildStream)?;

        let volume = Arc::new(Mutex::new(1.0));
        let buffer_volume = Arc::clone(&volume);

        let mut res = Self {
            stream,
            ring_buffer: RingBuffer::new(ring_buffer, buffer_volume),
            volume,
        };

        res.pause()?;

        Ok(res)
    }

    #[must_use]
    pub fn get_ring_buffer(&self) -> RingBuffer {
        self.ring_buffer.clone()
    }

    pub fn pause(&mut self) -> Result<(), Error> {
        self.stream
            .pause()
            .map_err(|_err| Error::CouldntPauseStream)
    }

    pub fn resume(&mut self) -> Result<(), Error> {
        self.stream.play().map_err(|_err| Error::CouldntPlayStream)
    }

    #[must_use]
    pub fn volume(&self) -> &Arc<Mutex<f32>> {
        &self.volume
    }

    #[must_use]
    pub const fn sample_rate() -> i32 {
        SAMPLE_RATE
    }
}

#[derive(Debug)]
pub enum Error {
    CouldntGetOutputDevice,
    CouldntBuildStream,
    CouldntPauseStream,
    CouldntPlayStream,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Error::CouldntGetOutputDevice => write!(f, "couldn't get output device"),
            Error::CouldntBuildStream => write!(f, "couldn't build stream"),
            Error::CouldntPauseStream => write!(f, "couldn't pause stream"),
            Error::CouldntPlayStream => write!(f, "couldn't play stream"),
        }
    }
}

impl std::error::Error for Error {}
