use {
    noise::Noise,
    square::{Ch1, Ch2},
    wave::Wave,
};

mod core;
mod envelope;
mod freq;
mod noise;
mod square;
mod wave;

pub struct Channels {
    pub ch1: Ch1,
    pub ch2: Ch2,
    pub ch3: Wave,
    pub ch4: Noise,
}

impl Channels {
    pub fn new() -> Self {
        Self {
            ch1: Ch1::new(),
            ch2: Ch2::new(),
            ch3: Wave::new(),
            ch4: Noise::new(),
        }
    }

    pub fn read_enable_u4(&self) -> u8 {
        (u8::from(self.ch4.on()) << 3)
            | (u8::from(self.ch3.on()) << 2)
            | (u8::from(self.ch2.on()) << 1)
            | u8::from(self.ch1.on())
    }

    pub fn set_period_half(&mut self, period_half: u8) {
        self.ch1.mut_core().set_period_half(period_half);
        self.ch2.mut_core().set_period_half(period_half);
        self.ch3.mut_core().set_period_half(period_half);
        self.ch4.mut_core().set_period_half(period_half);
    }

    pub fn step_length(&mut self) {
        self.ch1.step_length();
        self.ch2.step_length();
        self.ch3.step_length();
        self.ch4.step_length();
    }

    pub fn step_sample(&mut self) {
        self.ch1.step_sample();
        self.ch2.step_sample();
        self.ch3.step_sample();
        self.ch4.step_sample();
    }

    pub fn step_sweep(&mut self) {
        self.ch1.step_sweep();
    }

    pub fn step_envelope(&mut self) {
        self.ch1.step_envelope();
        self.ch2.step_envelope();
        self.ch4.step_envelope();
    }

    pub fn reset(&mut self) {
        self.ch1.reset();
        self.ch2.reset();
        self.ch3.reset();
        self.ch4.reset();
    }

    pub fn dac_output_iter(&self) -> DacOutputIterator {
        DacOutputIterator { idx: 0, chs: self }
    }
}

pub struct DacOutputIterator<'a> {
    idx: u8,
    chs: &'a Channels,
}

impl<'a> Iterator for DacOutputIterator<'a> {
    type Item = u8;

    fn next(&mut self) -> Option<Self::Item> {
        let val = match self.idx {
            0 => Some(self.chs.ch1.output_volume() * u8::from(self.chs.ch1.on())),
            1 => Some(self.chs.ch2.output_volume() * u8::from(self.chs.ch2.on())),
            2 => Some(self.chs.ch3.output_volume() * u8::from(self.chs.ch3.on())),
            3 => Some(self.chs.ch4.output_volume() * u8::from(self.chs.ch4.on())),
            _ => return None,
        };

        self.idx += 1;

        val
    }
}
