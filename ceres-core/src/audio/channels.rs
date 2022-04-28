mod envelope;
mod frequency_data;
mod generic_channel;
mod noise_channel;
mod square;
mod wave;

use noise_channel::NoiseChannel;
use square::square1::Square1;
use square::square2::Square2;
use wave::WaveChannel;

pub use generic_channel::LengthPeriodHalf;

pub struct Channels {
    pub square1: Square1,
    pub square2: Square2,
    pub wave: WaveChannel,
    pub noise: NoiseChannel,
}

impl Channels {
    pub fn new() -> Self {
        Self {
            square1: Square1::new(),
            square2: Square2::new(),
            wave: WaveChannel::new(),
            noise: NoiseChannel::new(),
        }
    }

    pub fn read_enable_u4(&self) -> u8 {
        (u8::from(self.noise.is_enabled()) << 3)
            | (u8::from(self.wave.is_enabled()) << 2)
            | (u8::from(self.square2.is_enabled()) << 1)
            | u8::from(self.square1.is_enabled())
    }

    pub fn set_length_period_half(&mut self, length_period_half: LengthPeriodHalf) {
        self.square1
            .mut_generic_channel()
            .set_length_period_half(length_period_half);
        self.square2
            .mut_generic_channel()
            .set_length_period_half(length_period_half);
        self.wave
            .mut_generic_channel()
            .set_length_period_half(length_period_half);
        self.noise
            .mut_generic_channel()
            .set_length_period_half(length_period_half);
    }

    pub fn step_length(&mut self) {
        self.square1.step_length();
        self.square2.step_length();
        self.wave.step_length();
        self.noise.step_length();
    }

    pub fn step_sample(&mut self) {
        self.square1.step_sample();
        self.square2.step_sample();
        self.wave.step_sample();
        self.noise.step_sample();
    }

    pub fn step_sweep(&mut self) {
        self.square1.step_sweep();
    }

    pub fn step_envelope(&mut self) {
        self.square1.step_envelope();
        self.square2.step_envelope();
        self.noise.step_envelope();
    }

    pub fn reset(&mut self) {
        self.square1.reset();
        self.square2.reset();
        self.wave.reset();
        self.noise.reset();
    }

    pub fn dac_output_iter(&self) -> DacOutputIterator {
        DacOutputIterator {
            channel_index: 0,
            channels: self,
        }
    }
}

pub struct DacOutputIterator<'a> {
    channel_index: u8,
    channels: &'a Channels,
}

impl<'a> Iterator for DacOutputIterator<'a> {
    type Item = u8;

    fn next(&mut self) -> Option<Self::Item> {
        let value = match self.channel_index {
            0 => Some(
                self.channels.square1.output_volume()
                    * u8::from(self.channels.square1.is_enabled()),
            ),
            1 => Some(
                self.channels.square2.output_volume()
                    * u8::from(self.channels.square2.is_enabled()),
            ),
            2 => Some(
                self.channels.wave.output_volume()
                    * u8::from(self.channels.wave.is_enabled()),
            ),
            3 => Some(
                self.channels.noise.output_volume()
                    * u8::from(self.channels.noise.is_enabled()),
            ),
            _ => return None,
        };

        self.channel_index += 1;

        value
    }
}
