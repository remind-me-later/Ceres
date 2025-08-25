use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use ringbuf::{
    StaticRb,
    traits::{Consumer, Observer},
};
use rubato::Resampler;
use {std::sync::Arc, std::sync::Mutex};

// Buffer size is the number of samples per channel per callback
const BUFFER_SIZE: cpal::FrameCount = 512;
const RING_BUFFER_SIZE: usize = BUFFER_SIZE as usize * 8;
const SAMPLE_RATE: i32 = 48000;

// Originally both the emulator and host platform output samples at the same rate,
// as time passes one begins to shift away from the other, so we need to resample the emulator output
const ORIG_RATIO: f64 = 1.0;
const MAX_RESAMPLE_RATIO_RELATIVE: f64 = 5.0;

type ProcessSample = f32;

struct Buffers {
    input_buf: Vec<Vec<ProcessSample>>,
    left: StaticRb<ceres_core::Sample, RING_BUFFER_SIZE>,
    output_buf: Vec<Vec<ProcessSample>>,
    resampler: rubato::SincFixedOut<ProcessSample>,
    right: StaticRb<ceres_core::Sample, RING_BUFFER_SIZE>,
    volume: Arc<Mutex<f32>>,
}

impl Buffers {
    fn clear(&mut self) {
        self.left.clear();
        self.right.clear();
    }

    fn compute_resample_ratio(&self) -> f64 {
        #[expect(clippy::cast_precision_loss)]
        let occupied = self.num_samples() as f64;
        #[expect(clippy::cast_precision_loss)]
        let target = RING_BUFFER_SIZE as f64 / 2.0;
        let error = (occupied - target) / target;

        if error.abs() < 0.1 {
            return ORIG_RATIO;
        }

        // Adjust ratio based on buffer occupancy
        // If buffer is too full, speed up playback (increase ratio)
        // If buffer is too empty, slow down playback (decrease ratio)
        let adjustment = -error * 0.05;

        (ORIG_RATIO * (1.0 + adjustment))
            .clamp(ORIG_RATIO * 0.85, ORIG_RATIO * MAX_RESAMPLE_RATIO_RELATIVE)
    }

    fn new(volume: Arc<Mutex<f32>>) -> Result<Self, Error> {
        // FIXME: Cpal doesn't support pipewire on Linux, this seems to match the returned buffer size by accident
        // we have to way for cpal to have a nice way to get the supported buffer sizes or support pipewire
        #[cfg(target_os = "linux")]
        let chunk_size = BUFFER_SIZE as usize / 4;

        #[cfg(not(target_os = "linux"))]
        let chunk_size = BUFFER_SIZE as usize;

        let resampler = rubato::SincFixedOut::<ProcessSample>::new(
            ORIG_RATIO,
            MAX_RESAMPLE_RATIO_RELATIVE,
            rubato::SincInterpolationParameters {
                sinc_len: 256,
                f_cutoff: 0.95,
                oversampling_factor: 128,
                interpolation: rubato::SincInterpolationType::Cubic,
                window: rubato::WindowFunction::Blackman,
            },
            chunk_size,
            2,
        )
        .map_err(|_err| Error::BuildStream)?;

        let input_buf = resampler.input_buffer_allocate(true);
        let output_buf = resampler.output_buffer_allocate(true);

        Ok(Self {
            left: StaticRb::default(),
            right: StaticRb::default(),
            resampler,
            output_buf,
            input_buf,
            volume,
        })
    }

    fn num_samples(&self) -> usize {
        self.left.occupied_len()
    }

    fn push_samples(&mut self, l: ceres_core::Sample, r: ceres_core::Sample) {
        use ringbuf::traits::RingBuffer;

        self.left.push_overwrite(l);
        self.right.push_overwrite(r);
    }

    fn write_samples_interleaved(&mut self, buffer: &mut [ProcessSample]) {
        use ringbuf::traits::Consumer;

        let new_ratio = self.compute_resample_ratio();
        self.resampler
            .set_resample_ratio(new_ratio, true)
            .unwrap_or_else(|e| eprintln!("Failed to set resample ratio: {e}"));

        let needed = self.resampler.input_frames_next();
        let got = self.num_samples();

        if needed > got {
            // eprintln!("Buffer underrun, needed: {needed}, got: {got}");
            return;
        }

        let (input_buf_left, input_buf_right) = self.input_buf.split_at_mut(1);

        if let Ok(vol) = self.volume.lock() {
            for (l, r) in input_buf_left[0]
                .iter_mut()
                .zip(input_buf_right[0].iter_mut())
                .take(needed)
            {
                *l =
                    f32::from(self.left.try_pop().unwrap_or_default()) / f32::from(i16::MAX) * *vol;
                *r = f32::from(self.right.try_pop().unwrap_or_default()) / f32::from(i16::MAX)
                    * *vol;
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
                    eprintln!("Resampler error: {e}");
                }
            }
        }
    }
}

#[derive(Clone)]
pub struct AudioCallbackImpl {
    buffer: Arc<Mutex<Buffers>>,
}

impl AudioCallbackImpl {
    fn clear(&self) {
        self.buffer.lock().map(|mut b| b.clear()).ok();
    }

    const fn new(buffer: Arc<Mutex<Buffers>>) -> Self {
        Self { buffer }
    }
}

impl ceres_core::AudioCallback for AudioCallbackImpl {
    fn audio_sample(&self, l: ceres_core::Sample, r: ceres_core::Sample) {
        if let Ok(mut buffer) = self.buffer.lock() {
            buffer.push_samples(l, r);
        }
    }
}

pub struct AudioState {
    _host: cpal::Host,
    config: cpal::StreamConfig,
    device: cpal::Device,
}

impl AudioState {
    #[must_use]
    pub const fn config(&self) -> &cpal::StreamConfig {
        &self.config
    }

    #[must_use]
    pub const fn device(&self) -> &cpal::Device {
        &self.device
    }

    pub fn new() -> Result<Self, Error> {
        let host = cpal::default_host();
        let device = host.default_output_device().ok_or(Error::GetOutputDevice)?;

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
}

// Stream is not Send, so we can't use it directly in the renderer struct
pub struct Stream {
    cpal_strm: cpal::Stream,
    ring_buffer: AudioCallbackImpl,
    sample_rate: i32,
    volume: Arc<Mutex<f32>>,
    volume_before_mute: Option<f32>,
}

impl Stream {
    #[must_use]
    pub const fn is_muted(&self) -> bool {
        self.volume_before_mute.is_some()
    }

    pub fn mute(&mut self) {
        if let Ok(mut vol) = self.volume.lock() {
            self.volume_before_mute = Some(*vol);
            *vol = 0.0;
        }
    }

    pub fn new(state: &AudioState) -> Result<Self, Error> {
        const INITIAL_VOLUME: f32 = 1.0;
        let volume = Arc::new(Mutex::new(INITIAL_VOLUME));
        let buffer_volume = Arc::clone(&volume);

        let ring_buffer = Arc::new(Mutex::new(Buffers::new(buffer_volume)?));
        let ring_buffer_clone = Arc::clone(&ring_buffer);

        let error_callback = |err| eprintln!("an AudioError occurred on stream: {err}");
        let data_callback = move |buffer: &mut [ProcessSample], _: &_| {
            if let Ok(mut ring) = ring_buffer_clone.lock() {
                ring.write_samples_interleaved(buffer);
            }
        };

        let stream = state
            .device()
            .build_output_stream(state.config(), data_callback, error_callback, None)
            .map_err(|_err| Error::BuildStream)?;

        let res = Self {
            cpal_strm: stream,
            ring_buffer: AudioCallbackImpl::new(ring_buffer),
            volume,
            volume_before_mute: None,
            sample_rate: SAMPLE_RATE,
        };

        res.pause()?;

        Ok(res)
    }

    pub fn pause(&self) -> Result<(), Error> {
        self.cpal_strm.pause().map_err(|_err| Error::PauseStream)?;
        // Avoids audio stretching after unpausing
        self.ring_buffer.clear();
        Ok(())
    }

    pub fn resume(&self) -> Result<(), Error> {
        self.cpal_strm.play().map_err(|_err| Error::PlayStream)
    }

    #[must_use]
    pub fn ring_buffer(&self) -> AudioCallbackImpl {
        self.ring_buffer.clone()
    }

    #[must_use]
    pub const fn sample_rate(&self) -> i32 {
        self.sample_rate
    }

    pub fn set_volume(&self, volume: f32) {
        if let Ok(mut vol) = self.volume.lock() {
            *vol = volume;
        }
    }

    pub fn unmute(&mut self) {
        if let Some(vol) = self.volume_before_mute.take()
            && let Ok(mut v) = self.volume.lock()
        {
            *v = vol;
        }
    }

    #[must_use]
    pub fn volume(&self) -> f32 {
        self.volume.lock().map_or(0.0, |vol| *vol)
    }

    // pub fn set_sample_rate(&mut self, sample_rate: i32) {
    //     self.sample_rate = sample_rate;
    // }
}

#[derive(Debug)]
pub enum Error {
    BuildStream,
    GetOutputDevice,
    PauseStream,
    PlayStream,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::GetOutputDevice => write!(f, "couldn't get output device"),
            Self::BuildStream => write!(f, "couldn't build stream"),
            Self::PauseStream => write!(f, "couldn't pause stream"),
            Self::PlayStream => write!(f, "couldn't play stream"),
        }
    }
}

impl std::error::Error for Error {}
