pub use self::{
    noise::Noise,
    square::{Ch1, Ch2},
    wave::Wave,
};
use crate::{Gb, T_CYCLES_PER_SECOND};

mod ccore;
mod envelope;
mod freq;
mod noise;
mod square;
mod wave;

pub const TIMER_RESET_VALUE: u16 = (T_CYCLES_PER_SECOND / 512) as u16;

pub trait AudioCallbacks {
    fn sample_rate(&self) -> u32;
    fn push_frame(&mut self, frame: Frame);

    #[allow(clippy::cast_precision_loss)]
    fn cycles_to_render(&self) -> u32 {
        T_CYCLES_PER_SECOND / self.sample_rate()
    }
}

pub type Sample = f32;

pub struct Frame {
    left: Sample,
    right: Sample,
}

impl Frame {
    fn new(left: Sample, right: Sample) -> Self {
        Self { left, right }
    }

    #[must_use]
    pub fn left(&self) -> Sample {
        self.left
    }

    #[must_use]
    pub fn right(&self) -> Sample {
        self.right
    }
}

impl Gb {
    pub fn tick_apu(&mut self) {
        let mut mus_elapsed = self.t_elapsed();

        if !self.apu_on {
            return;
        }

        while mus_elapsed > 0 {
            mus_elapsed -= 1;

            self.timer -= 1;
            if self.timer == 0 {
                self.timer = TIMER_RESET_VALUE;
                self.step();
            }

            self.ch1.step_sample();
            self.ch2.step_sample();
            self.ch3.step_sample();
            self.ch4.step_sample();

            self.apu_cycles_until_render -= 1;
            if self.apu_cycles_until_render == 0 {
                self.apu_cycles_until_render = self.apu_render_period;
                self.mix_and_render();
            }
        }
    }

    fn step(&mut self) {
        if self.apu_counter & 1 == 0 {
            self.ch1.step_length();
            self.ch2.step_length();
            self.ch3.step_length();
            self.ch4.step_length();

            self.set_period_half(0);
        } else {
            self.set_period_half(1);
        }

        match self.apu_counter {
            2 | 6 => self.ch1.step_sweep(),
            7 => {
                self.ch1.step_envelope();
                self.ch2.step_envelope();
                self.ch4.step_envelope();
            }
            _ => (),
        }

        self.apu_counter = (self.apu_counter + 1) & 7;
    }

    fn set_period_half(&mut self, period_half: u8) {
        self.ch1.mut_core().set_period_half(period_half);
        self.ch2.mut_core().set_period_half(period_half);
        self.ch3.mut_core().set_period_half(period_half);
        self.ch4.mut_core().set_period_half(period_half);
    }

    fn mix_and_render(&mut self) {
        let (left, right) = self
            .output_iter()
            .zip(self.channel_on_iter())
            .fold((0, 0), |(l, r), (out, (l_on, r_on))| {
                (l + out * l_on as u8, r + out * r_on as u8)
            });

        let (l_out, r_out) = (self.left_volume, self.right_volume);

        // transform to i16 sample
        let left_sample = (0xf - left as i16 * 2) * i16::from(l_out);
        let right_sample = (0xf - right as i16 * 2) * i16::from(r_out);

        // filter
        let left_sample = self.high_pass_filter.filter(left_sample, self.apu_on);
        let right_sample = self.high_pass_filter.filter(right_sample, self.apu_on);

        let frame = Frame::new(left_sample, right_sample);
        unsafe { (*self.audio_callbacks).push_frame(frame) };
    }

    fn reset(&mut self) {
        self.apu_cycles_until_render = self.apu_render_period;
        self.ch1.reset();
        self.ch2.reset();
        self.ch3.reset();
        self.ch4.reset();
        self.left_volume = 0;
        self.left_vin_on = false;
        self.right_volume = 0;
        self.right_vin_on = false;
        self.nr51 = 0;
    }

    #[must_use]
    pub fn read_nr50(&self) -> u8 {
        self.right_volume
            | (u8::from(self.right_vin_on) << 3)
            | (self.left_volume << 4)
            | (u8::from(self.left_vin_on) << 7)
    }

    #[must_use]
    pub fn read_nr52(&self) -> u8 {
        u8::from(self.apu_on) << 7
            | 0x70
            | u8::from(self.ch4.on()) << 3
            | u8::from(self.ch3.on()) << 2
            | u8::from(self.ch2.on()) << 1
            | u8::from(self.ch1.on())
    }

    pub fn write_wave(&mut self, addr: u8, val: u8) {
        self.ch3.write_wave_ram(addr, val);
    }

    pub fn write_nr50(&mut self, val: u8) {
        if self.apu_on {
            self.right_volume = val & 7;
            self.right_vin_on = val & (1 << 3) != 0;
            self.left_volume = (val >> 4) & 7;
            self.left_vin_on = val & (1 << 7) != 0;
        }
    }

    pub fn write_nr51(&mut self, val: u8) {
        if self.apu_on {
            self.nr51 = val;
        }
    }

    pub fn write_nr52(&mut self, val: u8) {
        self.apu_on = val & (1 << 7) != 0;
        if !self.apu_on {
            self.reset();
        }
    }

    fn channel_on_iter(&self) -> ChannelOnIter {
        ChannelOnIter {
            i: 0,
            nr51: self.nr51,
        }
    }

    fn output_iter(&self) -> OutputIter {
        OutputIter { i: 0, gb: self }
    }
}

struct OutputIter<'a> {
    i: u8,
    gb: &'a Gb,
}

impl<'a> Iterator for OutputIter<'a> {
    type Item = u8;

    fn next(&mut self) -> Option<Self::Item> {
        let val = match self.i {
            0 => Some(self.gb.ch1.out() * u8::from(self.gb.ch1.on())),
            1 => Some(self.gb.ch2.out() * u8::from(self.gb.ch2.on())),
            2 => Some(self.gb.ch3.out() * u8::from(self.gb.ch3.on())),
            3 => Some(self.gb.ch4.out() * u8::from(self.gb.ch4.on())),
            _ => return None,
        };

        self.i += 1;

        val
    }
}

pub struct ChannelOnIter {
    i: u8,
    nr51: u8,
}

impl Iterator for ChannelOnIter {
    type Item = (bool, bool);

    fn next(&mut self) -> Option<Self::Item> {
        if self.i < 4 {
            let right_bit = 1 << self.i;
            let left_bit = 1 << (self.i + 4);
            let (left_on, right_on) = (self.nr51 & left_bit != 0, self.nr51 & right_bit != 0);

            self.i += 1;
            Some((left_on, right_on))
        } else {
            None
        }
    }
}

pub struct HighPassFilter {
    capacitor: f32,
    charge_factor: f32,
}

impl HighPassFilter {
    pub fn new() -> Self {
        let charge_factor = 0.998_943;
        // FIXME not powf in no_std :(
        // let charge_factor = 0.999958_f32.powf(T_CYCLES_PER_SECOND as f32 / sample_rate as f32);

        Self {
            capacitor: 0.0,
            charge_factor,
        }
    }

    pub fn filter(&mut self, input: i16, dac_on: bool) -> f32 {
        // TODO: amplification
        let input = (input * 32) as f32 / 32768.0;
        if dac_on {
            let output = input - self.capacitor;
            self.capacitor = input - output * self.charge_factor;
            output
        } else {
            0.0
        }
    }
}
