use {
    ceres_core::Sample,
    cpal::{
        traits::{DeviceTrait, HostTrait, StreamTrait},
        SampleRate,
    },
    dasp_ring_buffer::Bounded,
    parking_lot::Mutex,
    std::sync::Arc,
};

const BUFFER_SIZE: cpal::FrameCount = 512;
const RING_BUFFER_SIZE: usize = BUFFER_SIZE as usize * 4;
const SAMPLE_RATE: u32 = 48000;

pub struct Renderer {
    ring_buffer: Arc<Mutex<Bounded<[Sample; RING_BUFFER_SIZE]>>>,
    stream: cpal::Stream,
}

impl Renderer {
    pub fn init() -> Self {
        let host = cpal::default_host();
        let device = host.default_output_device().unwrap();

        let desired_config = cpal::StreamConfig {
            channels: 2,
            sample_rate: SampleRate(SAMPLE_RATE),
            buffer_size: cpal::BufferSize::Fixed(BUFFER_SIZE),
        };

        let rbuf = Arc::new(Mutex::new(Bounded::from(
            [Default::default(); RING_BUFFER_SIZE],
        )));

        let data_callback = {
            let rbuf = Arc::clone(&rbuf);
            move |output: &mut [Sample], _: &_| {
                let mut buf = rbuf.lock();
                output
                    .iter_mut()
                    .zip(buf.drain())
                    .for_each(|(out_sample, gb_sample)| *out_sample = gb_sample);
            }
        };

        let error_callback = |e| panic!("an AudioError occurred on stream: {}", e);

        let stream = device
            .build_output_stream(&desired_config, data_callback, error_callback)
            .unwrap();

        stream.pause().unwrap();

        Self {
            ring_buffer: rbuf,
            stream,
        }
    }

    pub fn play(&mut self) {
        self.stream.play().unwrap();
    }

    pub fn pause(&mut self) {
        self.stream.pause().unwrap();
    }

    pub fn sample_rate() -> u32 {
        SAMPLE_RATE
    }

    pub fn push_frame(&mut self, l: Sample, r: Sample) {
        let mut buf = self.ring_buffer.lock();
        buf.push(l);
        buf.push(r);
    }
}
