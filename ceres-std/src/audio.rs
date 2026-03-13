use cpal::traits::{DeviceTrait as _, HostTrait as _, StreamTrait as _};
use ringbuf::{
    HeapRb,
    traits::{Consumer as _, Observer as _, Producer as _, Split as _},
};
use rubato::Resampler as _;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};

// Buffer size is the number of samples per channel per callback
const BUFFER_SIZE: u32 = 512;
const RING_BUFFER_SIZE: usize = BUFFER_SIZE as usize * 4;
const SAMPLE_RATE: i32 = 48000;

// Originally both the emulator and host platform output samples at the same rate,
// as time passes one begins to shift away from the other, so we need to resample the emulator output

const ORIG_RATIO: f64 = 1.0;

const MAX_RESAMPLE_RATIO_RELATIVE: f64 = 5.0;

type ProcessSample = f32;

type Rb = HeapRb<(ceres_core::Sample, ceres_core::Sample)>;
type RbProducer = <Rb as ringbuf::traits::Split>::Prod;
type RbConsumer = <Rb as ringbuf::traits::Split>::Cons;

struct AudioProcessor {
    input_interleaved: Vec<ProcessSample>,
    consumer: Arc<Mutex<RbConsumer>>,
    resampler: rubato::Async<ProcessSample>,
    volume: Arc<AtomicU32>,
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
        // If buffer is too full, speed up playback (decrease ratio)
        // If buffer is too empty, slow down playback (increase ratio)
        let adjustment = -error * 0.05;

        (ORIG_RATIO * (1.0 + adjustment))
            .clamp(ORIG_RATIO * 0.85, ORIG_RATIO * MAX_RESAMPLE_RATIO_RELATIVE)
    }

    fn new(volume: Arc<AtomicU32>, consumer: Arc<Mutex<RbConsumer>>) -> Result<Self, Error> {
        let chunk_size = BUFFER_SIZE as usize;

        let resampler = rubato::Async::<ProcessSample>::new_sinc(
            ORIG_RATIO,
            MAX_RESAMPLE_RATIO_RELATIVE,
            &rubato::SincInterpolationParameters {
                sinc_len: 256,
                f_cutoff: 0.95,
                oversampling_factor: 128,
                interpolation: rubato::SincInterpolationType::Cubic,
                window: rubato::WindowFunction::Blackman,
            },
            chunk_size,
            2,
            rubato::FixedAsync::Output,
        )
        .map_err(|_err| Error::BuildStream)?;

        let input_interleaved = vec![0.0; resampler.input_frames_max() * 2];

        Ok(Self {
            resampler,
            input_interleaved,
            volume,
            consumer,
        })
    }

    fn write_samples_interleaved(&mut self, buffer: &mut [ProcessSample]) {
        // 1. Lock consumer and prepare input
        let mut cons = if let Ok(cons) = self.consumer.lock() {
            cons
        } else {
            buffer.fill(0.0); // Silence on poisoned lock
            return;
        };

        let num_samples = cons.occupied_len();
        let ratio = Self::compute_resample_ratio(num_samples);

        self.resampler
            .set_resample_ratio(ratio, true)
            .unwrap_or_else(|e| eprintln!("Failed to set resample ratio: {e}"));

        let needed = self.resampler.input_frames_next();

        // 2. Underrun Check
        if needed > num_samples {
            eprintln!("Underrun: needed {needed}, got {num_samples}");
            buffer.fill(0.0);
            return;
        }

        // 3. Pop Samples and convert to interleaved f32
        let vol = f32::from_bits(self.volume.load(Ordering::Relaxed));

        self.input_interleaved
            .chunks_exact_mut(2)
            .zip(cons.pop_iter())
            .take(needed)
            .for_each(|(interleaved, (l, r))| {
                interleaved[0] = f32::from(l) / f32::from(i16::MAX) * vol;
                interleaved[1] = f32::from(r) / f32::from(i16::MAX) * vol;
            });

        std::mem::drop(cons); // Unlock early before heavy resampling

        // 4. Resample directly into the output buffer
        let input_adapter =
            audioadapter_buffers::direct::InterleavedSlice::new(&self.input_interleaved, 2, needed)
                .unwrap();

        let mut output_adapter = audioadapter_buffers::direct::InterleavedSlice::new_mut(
            buffer,
            2,
            self.resampler.output_frames_max(),
        )
        .unwrap();

        if let Err(e) =
            self.resampler
                .process_into_buffer(&input_adapter, &mut output_adapter, None)
        {
            eprintln!("Resampler error: {e}");
            buffer.fill(0.0); // Silence on error
        }
    }
}

#[derive(Clone)]
pub struct AudioCallbackImpl {
    rb: Arc<Mutex<RbProducer>>,
}

impl AudioCallbackImpl {
    const fn new(rb: Arc<Mutex<RbProducer>>) -> Self {
        Self { rb }
    }
}

impl ceres_core::AudioCallback for AudioCallbackImpl {
    fn audio_sample(&self, l: ceres_core::Sample, r: ceres_core::Sample) {
        // Emulator thread locks ONLY the Producer side.
        // This never blocks on the Audio Thread.
        if let Ok(mut rb) = self.rb.lock() {
            rb.try_push((l, r)).ok();
        }
    }
}

#[expect(clippy::struct_field_names)]
pub struct Stream {
    consumer: Arc<Mutex<RbConsumer>>,
    ring_buffer: AudioCallbackImpl,
    sample_rate: i32,
    stream: cpal::Stream,
    volume: Arc<AtomicU32>,
    volume_before_mute: Option<f32>,
}

impl Stream {
    #[must_use]
    pub const fn is_muted(&self) -> bool {
        self.volume_before_mute.is_some()
    }

    pub fn mute(&mut self) {
        let vol = f32::from_bits(self.volume.load(Ordering::Relaxed));
        self.volume_before_mute = Some(vol);
        self.volume.store(0.0f32.to_bits(), Ordering::Relaxed);
    }

    pub fn new() -> Result<Self, Error> {
        const INITIAL_VOLUME: f32 = 1.0;

        let volume = Arc::new(AtomicU32::new(INITIAL_VOLUME.to_bits()));
        let processor_volume = Arc::clone(&volume);

        // Initialize Interleaved Ring Buffer
        let rb = Rb::new(RING_BUFFER_SIZE);
        let (prod, cons) = rb.split();

        let prod = Arc::new(Mutex::new(prod));
        let cons = Arc::new(Mutex::new(cons));

        let mut audio_processor = AudioProcessor::new(processor_volume, Arc::clone(&cons))?;

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
            ring_buffer: AudioCallbackImpl::new(prod),
            consumer: cons,
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
        if let Ok(mut cons) = self.consumer.lock() {
            cons.clear();
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
        self.volume.store(volume.to_bits(), Ordering::Relaxed);
    }

    pub fn unmute(&mut self) {
        if let Some(vol) = self.volume_before_mute.take() {
            self.volume.store(vol.to_bits(), Ordering::Relaxed);
        }
    }

    #[must_use]
    pub fn volume(&self) -> f32 {
        f32::from_bits(self.volume.load(Ordering::Relaxed))
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
