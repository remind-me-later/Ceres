use cpal::traits::{DeviceTrait as _, HostTrait as _, StreamTrait as _};
use ringbuf::{
    HeapRb,
    traits::{Consumer as _, Observer as _, Producer as _, Split as _},
};
use rubato::Resampler as _;
use {std::sync::Arc, std::sync::Mutex};

// Buffer size is the number of samples per channel per callback
const BUFFER_SIZE: u32 = 512;
const RING_BUFFER_SIZE: usize = BUFFER_SIZE as usize * 4;
const SAMPLE_RATE: i32 = 48000;

// Originally both the emulator and host platform output samples at the same rate,
// as time passes one begins to shift away from the other, so we need to resample the emulator output

const ORIG_RATIO: f64 = 1.0;

const MAX_RESAMPLE_RATIO_RELATIVE: f64 = 5.0;

type ProcessSample = f32;

type Rb = HeapRb<ceres_core::Sample>;
type RbProducer = <Rb as ringbuf::traits::Split>::Prod;
type RbConsumer = <Rb as ringbuf::traits::Split>::Cons;

struct AudioProcessor {
    buffer_input_left: Vec<ceres_core::Sample>,
    buffer_input_right: Vec<ceres_core::Sample>,
    input_buf: Vec<Vec<ProcessSample>>,
    left_consumer: Arc<Mutex<RbConsumer>>,
    output_buf: Vec<Vec<ProcessSample>>,
    resampler: rubato::SincFixedOut<ProcessSample>,
    right_consumer: Arc<Mutex<RbConsumer>>,
    volume: Arc<Mutex<f32>>,
}

impl AudioProcessor {
    fn compute_resample_ratio(occupied: usize) -> f64 {
        #[expect(clippy::cast_precision_loss)]
        let occupied = occupied as f64;

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

    fn new(
        volume: Arc<Mutex<f32>>,
        left_consumer: Arc<Mutex<RbConsumer>>,
        right_consumer: Arc<Mutex<RbConsumer>>,
    ) -> Result<Self, Error> {
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
            resampler,
            output_buf,
            input_buf,
            volume,
            left_consumer,
            right_consumer,
            buffer_input_left: Vec::with_capacity(chunk_size * 2),
            buffer_input_right: Vec::with_capacity(chunk_size * 2),
        })
    }

    fn write_samples_interleaved(&mut self, buffer: &mut [ProcessSample]) {
        // 1. Lock consumer to check status and set resample ratio
        let needed;
        let num_samples;

        if let Ok(left) = self.left_consumer.lock() {
            num_samples = left.occupied_len();

            std::mem::drop(left); // Release lock early

            let ratio = Self::compute_resample_ratio(num_samples);

            // This is fine because the emulator thread does NOT need this lock.
            self.resampler
                .set_resample_ratio(ratio, true)
                .unwrap_or_else(|e| eprintln!("Failed to set resample ratio: {e}"));

            needed = self.resampler.input_frames_next();
        } else {
            buffer.fill(0.0); // Silence on poisoned lock
            return;
        }

        // 2. Underrun Check
        if needed > num_samples {
            eprintln!("Underrun: needed {needed}, got {num_samples}");
            buffer.fill(0.0);
            return;
        }

        // 3. Pop Samples
        if let Ok(mut left) = self.left_consumer.lock()
            && let Ok(mut right) = self.right_consumer.lock()
        {
            self.buffer_input_left.clear();
            self.buffer_input_right.clear();

            self.buffer_input_left.extend(left.pop_iter().take(needed));
            self.buffer_input_right
                .extend(right.pop_iter().take(needed));
        } else {
            buffer.fill(0.0);
            return;
        }

        // 4. Prepare Resampler Input (No locks held)
        let (input_buf_left, input_buf_right) = self.input_buf.split_at_mut(1);

        // Get volume and immediately unlock
        let vol = if let Ok(vol) = self.volume.lock() {
            *vol
        } else {
            buffer.fill(0.0);
            return;
        };

        for ((l, &l1), (r, &r1)) in input_buf_left[0]
            .iter_mut()
            .zip(self.buffer_input_left.iter())
            .zip(
                input_buf_right[0]
                    .iter_mut()
                    .zip(self.buffer_input_right.iter()),
            )
            .take(needed)
        {
            *l = f32::from(l1) / f32::from(i16::MAX) * vol;
            *r = f32::from(r1) / f32::from(i16::MAX) * vol;
        }

        // 5. Resample (Heavy Work)
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
                buffer.fill(0.0); // Silence on error
            }
        }
    }
}

#[derive(Clone)]
pub struct AudioCallbackImpl {
    left: Arc<Mutex<RbProducer>>,
    right: Arc<Mutex<RbProducer>>,
}

impl AudioCallbackImpl {
    const fn new(left: Arc<Mutex<RbProducer>>, right: Arc<Mutex<RbProducer>>) -> Self {
        Self { left, right }
    }
}

impl ceres_core::AudioCallback for AudioCallbackImpl {
    fn audio_sample(&self, l: ceres_core::Sample, r: ceres_core::Sample) {
        // Emulator thread locks ONLY the Producer side.
        // This never blocks on the Audio Thread.
        if let Ok(mut left) = self.left.lock() {
            left.try_push(l).ok();
        }
        if let Ok(mut right) = self.right.lock() {
            right.try_push(r).ok();
        }
    }
}

#[expect(clippy::struct_field_names)]
pub struct Stream {
    // We hold references to consumers to clear them on pause
    left_consumer: Arc<Mutex<RbConsumer>>,
    right_consumer: Arc<Mutex<RbConsumer>>,
    ring_buffer: AudioCallbackImpl,
    sample_rate: i32,
    stream: cpal::Stream,
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

    pub fn new() -> Result<Self, Error> {
        const INITIAL_VOLUME: f32 = 1.0;

        let volume = Arc::new(Mutex::new(INITIAL_VOLUME));
        let buffer_volume = Arc::clone(&volume);

        // Initialize Ring Buffers
        let left_rb = Rb::new(RING_BUFFER_SIZE);
        let right_rb = Rb::new(RING_BUFFER_SIZE);

        let (left_prod, left_cons) = left_rb.split();
        let (right_prod, right_cons) = right_rb.split();

        let left_prod = Arc::new(Mutex::new(left_prod));
        let right_prod = Arc::new(Mutex::new(right_prod));
        let left_cons = Arc::new(Mutex::new(left_cons));
        let right_cons = Arc::new(Mutex::new(right_cons));

        let mut audio_processor = AudioProcessor::new(
            buffer_volume,
            Arc::clone(&left_cons),
            Arc::clone(&right_cons),
        )?;

        let host = cpal::default_host();
        let device = host.default_output_device().ok_or(Error::GetOutputDevice)?;

        let config = cpal::StreamConfig {
            channels: 2,
            sample_rate: SAMPLE_RATE as u32,
            buffer_size: cpal::BufferSize::Fixed(BUFFER_SIZE),
        };

        let error_callback = |err| eprintln!("an AudioError occurred on stream: {err}");
        let data_callback = move |buffer: &mut [ProcessSample], _: &_| {
            audio_processor.write_samples_interleaved(buffer);
        };

        let stream = device
            .build_output_stream(&config, data_callback, error_callback, None)
            .map_err(|_err| Error::BuildStream)?;

        let res = Self {
            stream,
            ring_buffer: AudioCallbackImpl::new(left_prod, right_prod),
            left_consumer: left_cons,
            right_consumer: right_cons,
            volume,
            volume_before_mute: None,
            sample_rate: SAMPLE_RATE,
        };

        res.pause()?;

        Ok(res)
    }

    pub fn pause(&self) -> Result<(), Error> {
        self.stream.pause().map_err(|_err| Error::PauseStream)?;

        // Clear buffers on pause
        if let Ok(mut l) = self.left_consumer.lock() {
            l.clear();
        }
        if let Ok(mut r) = self.right_consumer.lock() {
            r.clear();
        }

        Ok(())
    }

    pub fn resume(&self) -> Result<(), Error> {
        self.stream.play().map_err(|_err| Error::PlayStream)
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
