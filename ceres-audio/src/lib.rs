use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use dasp_ring_buffer::Bounded;
use {std::sync::Arc, std::sync::Mutex};

// Buffer size is the number of samples per channel per callback
const BUFFER_SIZE: cpal::FrameCount = 512;
const RING_BUFFER_SIZE: usize = BUFFER_SIZE as usize * 16;
const SAMPLE_RATE: i32 = 48000;

// RingBuffer is a wrapper around a bounded ring buffer
// that implements the AudioCallback trait
#[derive(Clone, Debug)]
pub struct RingBuffer {
    buffer: Arc<Mutex<Bounded<Box<[ceres_core::Sample]>>>>,
}

impl RingBuffer {
    pub fn new(buffer: Arc<Mutex<Bounded<Box<[ceres_core::Sample]>>>>) -> Self {
        // FIll with silence
        if let Ok(mut buffer) = buffer.lock() {
            for _ in 0..buffer.max_len() {
                buffer.push(Default::default());
            }
        }

        Self { buffer }
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
        let ring_buffer = Arc::new(Mutex::new(Bounded::from(
            vec![Default::default(); RING_BUFFER_SIZE].into_boxed_slice(),
        )));
        let ring_buffer_clone = Arc::clone(&ring_buffer);

        let error_callback = |err| eprintln!("an AudioError occurred on stream: {err}");
        let data_callback = move |buffer: &mut [ceres_core::Sample], _: &_| {
            if let Ok(mut ring) = ring_buffer_clone.lock() {
                if ring.len() < buffer.len() {
                    eprintln!("ring buffer underrun");
                    while !ring.is_full() {
                        ring.push(Default::default());
                    }
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
            .map_err(|_err| Error::CouldntBuildStream)?;

        let mut res = Self {
            stream,
            ring_buffer: RingBuffer::new(ring_buffer),
            volume: Arc::new(Mutex::new(1.0)),
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
