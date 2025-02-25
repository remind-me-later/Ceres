use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use ringbuf::{StaticRb, traits::Observer};
use rubato::Resampler;
use {std::sync::Arc, std::sync::Mutex};

// Buffer size is the number of samples per channel per callback
const BUFFER_SIZE: cpal::FrameCount = 512;
const RING_BUFFER_SIZE: usize = BUFFER_SIZE as usize * 4;
const SAMPLE_RATE: i32 = 48000;

// Originally both the emulator and host platform output samples at the same rate,
// as time passes one begins to shift away from the other, so we need to resample the emulator output
const ORIG_RATIO: f64 = 1.0;
const MAX_RESAMPLE_RATIO_RELATIVE: f64 = 2.0;

struct Buffers {
    left: StaticRb<ceres_core::Sample, RING_BUFFER_SIZE>,
    right: StaticRb<ceres_core::Sample, RING_BUFFER_SIZE>,
    resampler: rubato::FastFixedOut<ceres_core::Sample>,
    output_buf: [[ceres_core::Sample; BUFFER_SIZE as usize]; 2],
    input_buf: [[ceres_core::Sample; BUFFER_SIZE as usize * 2]; 2],
}

impl Buffers {
    fn new() -> Result<Self, Error> {
        Ok(Self {
            left: StaticRb::default(),
            right: StaticRb::default(),
            resampler: rubato::FastFixedOut::<ceres_core::Sample>::new(
                ORIG_RATIO,
                MAX_RESAMPLE_RATIO_RELATIVE,
                rubato::PolynomialDegree::Cubic,
                BUFFER_SIZE as usize,
                2,
            )
            .map_err(|_err| Error::CouldntBuildStream)?,
            output_buf: [[Default::default(); BUFFER_SIZE as usize]; 2],
            input_buf: [[Default::default(); BUFFER_SIZE as usize * 2]; 2],
        })
    }

    fn push_samples(&mut self, l: ceres_core::Sample, r: ceres_core::Sample) {
        use ringbuf::traits::RingBuffer;

        self.left.push_overwrite(l);
        self.right.push_overwrite(r);
    }

    fn num_samples(&self) -> usize {
        self.left.occupied_len()
    }

    fn write_samples_interleaved(&mut self, buffer: &mut [ceres_core::Sample]) {
        use ringbuf::traits::Consumer;

        let new_ratio = self.compute_resample_ratio();
        self.resampler
            .set_resample_ratio(new_ratio, true)
            .unwrap_or_else(|e| eprintln!("Failed to set resample ratio: {e}"));

        let needed = self.resampler.input_frames_next();
        let got = self.num_samples();

        if needed > got {
            println!("needed: {needed}, got: {got}");
            return;
        }

        let (input_buf_left, input_buf_right) = self.input_buf.split_at_mut(1);

        for (l, r) in input_buf_left[0]
            .iter_mut()
            .zip(input_buf_right[0].iter_mut())
            .take(needed)
        {
            *l = self.left.try_pop().unwrap_or_default();
            *r = self.right.try_pop().unwrap_or_default();
        }

        match self
            .resampler
            .process_into_buffer(&self.input_buf, &mut self.output_buf, None)
        {
            Ok(_) => {
                buffer
                    .chunks_exact_mut(2)
                    .zip(self.output_buf[0].iter().zip(self.output_buf[1].iter()))
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

    fn compute_resample_ratio(&self) -> f64 {
        let occupied = self.num_samples() as f64;
        let target = RING_BUFFER_SIZE as f64;
        let error = (occupied - target) / target;

        // Adjust ratio based on buffer occupancy
        // If buffer is too full, speed up playback (increase ratio)
        // If buffer is too empty, slow down playback (decrease ratio)
        let adjustment = -error * 0.1; // Small adjustment factor

        (ORIG_RATIO * (1.0 + adjustment))
            .clamp(ORIG_RATIO, ORIG_RATIO * MAX_RESAMPLE_RATIO_RELATIVE)
    }
}

// RingBuffer is a wrapper around a bounded ring buffer
// that implements the AudioCallback trait
#[derive(Clone)]
pub struct AudioCallbackImpl {
    buffer: Arc<Mutex<Buffers>>,
    volume: Arc<Mutex<f32>>,
}

impl AudioCallbackImpl {
    fn new(buffer: Arc<Mutex<Buffers>>, volume: Arc<Mutex<f32>>) -> Self {
        Self { buffer, volume }
    }
}

impl ceres_core::AudioCallback for AudioCallbackImpl {
    fn audio_sample(&self, l: ceres_core::Sample, r: ceres_core::Sample) {
        if let Ok(mut buffer) = self.buffer.lock() {
            if let Ok(volume) = self.volume.lock() {
                let l = l * *volume;
                let r = r * *volume;

                buffer.push_samples(l, r);
            }
        }
    }
}

pub struct AudioState {
    _host: cpal::Host,
    device: cpal::Device,
    config: cpal::StreamConfig,
}

impl AudioState {
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
    ring_buffer: AudioCallbackImpl,
    volume: Arc<Mutex<f32>>,
}

impl Stream {
    pub fn new(state: &AudioState) -> Result<Self, Error> {
        let ring_buffer = Arc::new(Mutex::new(Buffers::new()?));
        let ring_buffer_clone = Arc::clone(&ring_buffer);

        let error_callback = |err| eprintln!("an AudioError occurred on stream: {err}");
        let data_callback = move |buffer: &mut [ceres_core::Sample], _: &_| {
            if let Ok(mut ring) = ring_buffer_clone.lock() {
                ring.write_samples_interleaved(buffer);
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
            ring_buffer: AudioCallbackImpl::new(ring_buffer, buffer_volume),
            volume,
        };

        res.pause()?;

        Ok(res)
    }

    #[must_use]
    pub fn get_ring_buffer(&self) -> AudioCallbackImpl {
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
