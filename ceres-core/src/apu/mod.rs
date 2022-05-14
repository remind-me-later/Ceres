use {
    self::{
        noise::Noise,
        square::{Ch1, Ch2},
        wave::Wave,
    },
    crate::T_CYCLES_PER_SECOND,
};

mod ccore;
mod envelope;
mod freq;
mod noise;
mod square;
mod wave;

const TIMER_RESET_VALUE: u16 = (T_CYCLES_PER_SECOND / 512) as u16;

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

pub struct Apu {
    ch1: Ch1,
    ch2: Ch2,
    ch3: Wave,
    ch4: Noise,

    // sequencer
    timer: u16,
    counter: u8,

    cycles_to_render: u32,
    cycles_until_next_render: u32,
    callbacks: *mut dyn AudioCallbacks,
    high_pass_filter: HighPassFilter,

    on: bool,
    nr51: u8,
    right_volume: u8,
    left_volume: u8,
    right_vin_on: bool,
    left_vin_on: bool,
}

impl Apu {
    pub fn new(callbacks: *mut dyn AudioCallbacks) -> Self {
        let cycles_to_render = unsafe { (*callbacks).cycles_to_render() };

        Self {
            ch1: Ch1::new(),
            ch2: Ch2::new(),
            ch3: Wave::new(),
            ch4: Noise::new(),
            cycles_to_render,
            cycles_until_next_render: cycles_to_render,
            callbacks,
            high_pass_filter: HighPassFilter::new(),
            on: false,
            nr51: 0,
            right_volume: 0,
            left_volume: 0,
            right_vin_on: false,
            left_vin_on: false,
            timer: TIMER_RESET_VALUE,
            counter: 0,
        }
    }

    pub fn tick(&mut self, mut mus_elapsed: u8) {
        if !self.on {
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

            self.cycles_until_next_render -= 1;
            if self.cycles_until_next_render == 0 {
                self.cycles_until_next_render = self.cycles_to_render;
                self.mix_and_render();
            }
        }
    }

    fn step(&mut self) {
        if self.counter & 1 == 0 {
            self.ch1.step_length();
            self.ch2.step_length();
            self.ch3.step_length();
            self.ch4.step_length();

            self.set_period_half(0);
        } else {
            self.set_period_half(1);
        }

        match self.counter {
            2 | 6 => self.ch1.step_sweep(),
            7 => {
                self.ch1.step_envelope();
                self.ch2.step_envelope();
                self.ch4.step_envelope();
            }
            _ => (),
        }

        self.counter = (self.counter + 1) & 7;
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
        let left_sample = self.high_pass_filter.filter(left_sample, self.on);
        let right_sample = self.high_pass_filter.filter(right_sample, self.on);

        let frame = Frame::new(left_sample, right_sample);
        unsafe { (*self.callbacks).push_frame(frame) };
    }

    fn reset(&mut self) {
        self.cycles_until_next_render = self.cycles_to_render;
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

    pub fn read_nr10(&self) -> u8 {
        self.ch1.read_nr10()
    }

    pub fn read_nr11(&self) -> u8 {
        self.ch1.read_nr11()
    }

    pub fn read_nr12(&self) -> u8 {
        self.ch1.read_nr12()
    }

    pub fn read_nr14(&self) -> u8 {
        self.ch1.read_nr14()
    }

    pub fn read_nr21(&self) -> u8 {
        self.ch2.read_nr21()
    }

    pub fn read_nr22(&self) -> u8 {
        self.ch2.read_nr22()
    }

    pub fn read_nr24(&self) -> u8 {
        self.ch2.read_nr24()
    }

    pub fn read_nr30(&self) -> u8 {
        self.ch3.read_nr30()
    }

    pub fn read_nr32(&self) -> u8 {
        self.ch3.read_nr32()
    }

    pub fn read_nr34(&self) -> u8 {
        self.ch3.read_nr34()
    }

    pub fn read_nr42(&self) -> u8 {
        self.ch4.read_nr42()
    }

    pub fn read_nr43(&self) -> u8 {
        self.ch4.read_nr43()
    }

    pub fn read_nr44(&self) -> u8 {
        self.ch4.read_nr44()
    }

    pub fn read_nr50(&self) -> u8 {
        self.right_volume
            | (u8::from(self.right_vin_on) << 3)
            | (self.left_volume << 4)
            | (u8::from(self.left_vin_on) << 7)
    }

    pub fn read_nr51(&self) -> u8 {
        self.nr51
    }

    pub fn read_nr52(&self) -> u8 {
        u8::from(self.on) << 7
            | 0x70
            | u8::from(self.ch4.on()) << 3
            | u8::from(self.ch3.on()) << 2
            | u8::from(self.ch2.on()) << 1
            | u8::from(self.ch1.on())
    }

    pub fn read_wave(&self, addr: u8) -> u8 {
        self.ch3.read_wave_ram(addr)
    }

    pub fn write_wave(&mut self, addr: u8, val: u8) {
        self.ch3.write_wave_ram(addr, val);
    }

    pub fn write_nr10(&mut self, val: u8) {
        if self.on {
            self.ch1.write_nr10(val);
        }
    }

    pub fn write_nr11(&mut self, val: u8) {
        if self.on {
            self.ch1.write_nr11(val);
        }
    }

    pub fn write_nr12(&mut self, val: u8) {
        if self.on {
            self.ch1.write_nr12(val);
        }
    }

    pub fn write_nr13(&mut self, val: u8) {
        if self.on {
            self.ch1.write_nr13(val);
        }
    }

    pub fn write_nr14(&mut self, val: u8) {
        if self.on {
            self.ch1.write_nr14(val);
        }
    }

    pub fn write_nr21(&mut self, val: u8) {
        if self.on {
            self.ch2.write_nr21(val);
        }
    }

    pub fn write_nr22(&mut self, val: u8) {
        if self.on {
            self.ch2.write_nr22(val);
        }
    }

    pub fn write_nr23(&mut self, val: u8) {
        if self.on {
            self.ch2.write_nr23(val);
        }
    }

    pub fn write_nr24(&mut self, val: u8) {
        if self.on {
            self.ch2.write_nr24(val);
        }
    }

    pub fn write_nr30(&mut self, val: u8) {
        if self.on {
            self.ch3.write_nr30(val);
        }
    }

    pub fn write_nr31(&mut self, val: u8) {
        if self.on {
            self.ch3.write_nr31(val);
        }
    }

    pub fn write_nr32(&mut self, val: u8) {
        if self.on {
            self.ch3.write_nr32(val);
        }
    }

    pub fn write_nr33(&mut self, val: u8) {
        if self.on {
            self.ch3.write_nr33(val);
        }
    }

    pub fn write_nr34(&mut self, val: u8) {
        if self.on {
            self.ch3.write_nr34(val);
        }
    }

    pub fn write_nr41(&mut self, val: u8) {
        if self.on {
            self.ch4.write_nr41(val);
        }
    }

    pub fn write_nr42(&mut self, val: u8) {
        if self.on {
            self.ch4.write_nr42(val);
        }
    }

    pub fn write_nr43(&mut self, val: u8) {
        if self.on {
            self.ch4.write_nr43(val);
        }
    }

    pub fn write_nr44(&mut self, val: u8) {
        if self.on {
            self.ch4.write_nr44(val);
        }
    }

    pub fn write_nr50(&mut self, val: u8) {
        if self.on {
            self.right_volume = val & 7;
            self.right_vin_on = val & (1 << 3) != 0;
            self.left_volume = (val >> 4) & 7;
            self.left_vin_on = val & (1 << 7) != 0;
        }
    }

    pub fn write_nr51(&mut self, val: u8) {
        if self.on {
            self.nr51 = val;
        }
    }

    pub fn write_nr52(&mut self, val: u8) {
        self.on = val & (1 << 7) != 0;
        if !self.on {
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
        OutputIter { i: 0, apu: self }
    }
}

struct OutputIter<'a> {
    i: u8,
    apu: &'a Apu,
}

impl<'a> Iterator for OutputIter<'a> {
    type Item = u8;

    fn next(&mut self) -> Option<Self::Item> {
        let val = match self.i {
            0 => Some(self.apu.ch1.out() * u8::from(self.apu.ch1.on())),
            1 => Some(self.apu.ch2.out() * u8::from(self.apu.ch2.on())),
            2 => Some(self.apu.ch3.out() * u8::from(self.apu.ch3.on())),
            3 => Some(self.apu.ch4.out() * u8::from(self.apu.ch4.on())),
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
