use {
    cpal::{
        traits::{DeviceTrait, HostTrait, StreamTrait},
        SampleFormat,
    },
    dasp_ring_buffer::Bounded,
    parking_lot::Mutex,
    std::sync::Arc,
};

const BUFFER_SIZE: cpal::FrameCount = 512;
const RING_BUFFER_SIZE: usize = BUFFER_SIZE as usize * 4;

pub struct AudioRenderer {
    ring_buffer: Arc<Mutex<Bounded<Box<[ceres_core::Sample]>>>>,
    sample_rate: u32,
    _stream: cpal::Stream,
}

impl AudioRenderer {
    pub fn new() -> Self {
        let host = cpal::default_host();
        let device = host.default_output_device().unwrap();

        let default_config = device.default_output_config().unwrap();
        let supported_config = device
            .supported_output_configs()
            .unwrap()
            .find(|s| s.channels() == 2 && s.sample_format() == SampleFormat::F32)
            .unwrap()
            .with_sample_rate(default_config.sample_rate());

        // println!("{:?}", supported_config);

        let desired_config = cpal::StreamConfig {
            channels: 2,
            sample_rate: supported_config.sample_rate(),
            buffer_size: cpal::BufferSize::Fixed(BUFFER_SIZE),
        };

        let ring_buffer = Arc::new(Mutex::new(Bounded::from(
            vec![0.0; RING_BUFFER_SIZE].into_boxed_slice(),
        )));
        let error_callback = |err| panic!("an AudioError occurred on stream: {}", err);
        let ring_buffer_arc = Arc::clone(&ring_buffer);
        let data_callback = move |output: &mut [f32], _: &_| {
            let mut buf = ring_buffer_arc.lock();
            output
                .iter_mut()
                .zip(buf.drain())
                .for_each(|(out_sample, gb_sample)| *out_sample = gb_sample)
        };

        let stream = device
            .build_output_stream(&desired_config, data_callback, error_callback)
            .unwrap();

        stream.play().expect("AudioError playing sound");

        let sample_rate = desired_config.sample_rate.0;

        Self {
            _stream: stream,
            sample_rate,
            ring_buffer,
        }
    }

    // pub fn play(&mut self) {
    //     self.stream.play().unwrap();
    // }

    // pub fn pause(&mut self) {
    //     self.stream.pause().unwrap();
    // }
}

impl ceres_core::AudioCallbacks for AudioRenderer {
    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn push_frame(&mut self, l: ceres_core::Sample, r: ceres_core::Sample) {
        let mut buf = self.ring_buffer.lock();
        buf.push(l);
        buf.push(r);
    }
}
