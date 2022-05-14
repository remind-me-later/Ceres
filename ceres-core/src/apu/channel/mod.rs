pub use generic_channel::LengthPeriodHalf;
use {
    noise_channel::NoiseChannel,
    square::{Chan1, Chan2},
    wave::Wave,
};

mod envelope;
mod frequency_data;
mod generic_channel;
mod noise_channel;
mod square;
mod wave;

pub struct Channels {
    pub sq1: Chan1,
    pub sq2: Chan2,
    pub wave: Wave,
    pub noise: NoiseChannel,
}

impl Channels {
    pub fn new() -> Self {
        Self {
            sq1: Chan1::new(),
            sq2: Chan2::new(),
            wave: Wave::new(),
            noise: NoiseChannel::new(),
        }
    }

    pub fn read_enable_u4(&self) -> u8 {
        (u8::from(self.noise.is_enabled()) << 3)
            | (u8::from(self.wave.is_enabled()) << 2)
            | (u8::from(self.sq2.is_enabled()) << 1)
            | u8::from(self.sq1.is_enabled())
    }

    pub fn set_length_period_half(&mut self, length_period_half: LengthPeriodHalf) {
        self.sq1
            .mut_generic_channel()
            .set_length_period_half(length_period_half);
        self.sq2
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
        self.sq1.step_length();
        self.sq2.step_length();
        self.wave.step_length();
        self.noise.step_length();
    }

    pub fn step_sample(&mut self) {
        self.sq1.step_sample();
        self.sq2.step_sample();
        self.wave.step_sample();
        self.noise.step_sample();
    }

    pub fn step_sweep(&mut self) {
        self.sq1.step_sweep();
    }

    pub fn step_envelope(&mut self) {
        self.sq1.step_envelope();
        self.sq2.step_envelope();
        self.noise.step_envelope();
    }

    pub fn reset(&mut self) {
        self.sq1.reset();
        self.sq2.reset();
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
            0 => Some(self.channels.sq1.output_volume() * u8::from(self.channels.sq1.is_enabled())),
            1 => Some(self.channels.sq2.output_volume() * u8::from(self.channels.sq2.is_enabled())),
            2 => {
                Some(self.channels.wave.output_volume() * u8::from(self.channels.wave.is_enabled()))
            }
            3 => Some(
                self.channels.noise.output_volume() * u8::from(self.channels.noise.is_enabled()),
            ),
            _ => return None,
        };

        self.channel_index += 1;

        value
    }
}
