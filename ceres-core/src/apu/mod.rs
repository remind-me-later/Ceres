use {self::channel::Channels, crate::T_CYCLES_PER_SECOND};

mod channel;

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
    channels: Channels,
    cycles_to_render: u32,
    cycles_until_next_render: u32,
    callbacks: *mut dyn AudioCallbacks,
    sequencer: Sequencer,
    high_pass_filter: HighPassFilter,
    on: bool,
    nr51: u8,
    right_output_volume: u8,
    left_output_volume: u8,
    right_vin_on: bool,
    left_vin_on: bool,
}

impl Apu {
    pub fn new(callbacks: *mut dyn AudioCallbacks) -> Self {
        let cycles_to_render = unsafe { (*callbacks).cycles_to_render() };

        Self {
            channels: Channels::new(),
            cycles_to_render,
            cycles_until_next_render: cycles_to_render,
            callbacks,
            sequencer: Sequencer::new(),
            high_pass_filter: HighPassFilter::new(),
            on: false,
            nr51: 0,
            right_output_volume: 0,
            left_output_volume: 0,
            right_vin_on: false,
            left_vin_on: false,
        }
    }

    pub fn tick(&mut self, mut mus_elapsed: u8) {
        if !self.on {
            return;
        }

        while mus_elapsed > 0 {
            mus_elapsed -= 1;

            self.sequencer.tick(&mut self.channels);

            self.cycles_until_next_render -= 1;
            if self.cycles_until_next_render == 0 {
                self.cycles_until_next_render = self.cycles_to_render;
                self.mix_and_render();
            }
        }
    }

    fn mix_and_render(&mut self) {
        let (left, right) = self
            .channels
            .dac_output_iter()
            .zip(self.channel_on_iter())
            .fold((0, 0), |(left, right), (output, (left_on, right_on))| {
                (
                    left + output * u8::from(left_on),
                    right + output * u8::from(right_on),
                )
            });

        let (left_output_volume, right_output_volume) =
            (self.left_output_volume, self.right_output_volume);

        // transform to i16 sample
        let left_sample = (0xf - left as i16 * 2) * i16::from(left_output_volume);
        let right_sample = (0xf - right as i16 * 2) * i16::from(right_output_volume);

        // filter
        let left_sample = self.high_pass_filter.filter(left_sample, self.on);
        let right_sample = self.high_pass_filter.filter(right_sample, self.on);

        let frame = Frame::new(left_sample, right_sample);
        unsafe { (*self.callbacks).push_frame(frame) };
    }

    fn reset(&mut self) {
        self.cycles_until_next_render = self.cycles_to_render;
        self.channels.reset();
        self.left_output_volume = 0;
        self.left_vin_on = false;
        self.right_output_volume = 0;
        self.right_vin_on = false;
        self.nr51 = 0;
    }

    pub fn read_nr10(&self) -> u8 {
        self.channels.ch1.read_nr10()
    }

    pub fn read_nr11(&self) -> u8 {
        self.channels.ch1.read_nr11()
    }

    pub fn read_nr12(&self) -> u8 {
        self.channels.ch1.read_nr12()
    }

    pub fn read_nr14(&self) -> u8 {
        self.channels.ch1.read_nr14()
    }

    pub fn read_nr21(&self) -> u8 {
        self.channels.ch2.read_nr21()
    }

    pub fn read_nr22(&self) -> u8 {
        self.channels.ch2.read_nr22()
    }

    pub fn read_nr24(&self) -> u8 {
        self.channels.ch2.read_nr24()
    }

    pub fn read_nr30(&self) -> u8 {
        self.channels.ch3.read_nr30()
    }

    pub fn read_nr32(&self) -> u8 {
        self.channels.ch3.read_nr32()
    }

    pub fn read_nr34(&self) -> u8 {
        self.channels.ch3.read_nr34()
    }

    pub fn read_nr42(&self) -> u8 {
        self.channels.ch4.read_nr42()
    }

    pub fn read_nr43(&self) -> u8 {
        self.channels.ch4.read_nr43()
    }

    pub fn read_nr44(&self) -> u8 {
        self.channels.ch4.read_nr44()
    }

    pub fn read_nr50(&self) -> u8 {
        self.right_output_volume
            | (u8::from(self.right_vin_on) << 3)
            | (self.left_output_volume << 4)
            | (u8::from(self.left_vin_on) << 7)
    }

    pub fn read_nr51(&self) -> u8 {
        self.nr51
    }

    pub fn read_nr52(&self) -> u8 {
        0x70 | (u8::from(self.on) << 7) | self.channels.read_enable_u4()
    }

    pub fn read_wave(&self, addr: u8) -> u8 {
        self.channels.ch3.read_wave_ram(addr)
    }

    pub fn write_wave(&mut self, addr: u8, val: u8) {
        self.channels.ch3.write_wave_ram(addr, val);
    }

    pub fn write_nr10(&mut self, val: u8) {
        if self.on {
            self.channels.ch1.write_nr10(val);
        }
    }

    pub fn write_nr11(&mut self, val: u8) {
        if self.on {
            self.channels.ch1.write_nr11(val);
        }
    }

    pub fn write_nr12(&mut self, val: u8) {
        if self.on {
            self.channels.ch1.write_nr12(val);
        }
    }

    pub fn write_nr13(&mut self, val: u8) {
        if self.on {
            self.channels.ch1.write_nr13(val);
        }
    }

    pub fn write_nr14(&mut self, val: u8) {
        if self.on {
            self.channels.ch1.write_nr14(val);
        }
    }

    pub fn write_nr21(&mut self, val: u8) {
        if self.on {
            self.channels.ch2.write_nr21(val);
        }
    }

    pub fn write_nr22(&mut self, val: u8) {
        if self.on {
            self.channels.ch2.write_nr22(val);
        }
    }

    pub fn write_nr23(&mut self, val: u8) {
        if self.on {
            self.channels.ch2.write_nr23(val);
        }
    }

    pub fn write_nr24(&mut self, val: u8) {
        if self.on {
            self.channels.ch2.write_nr24(val);
        }
    }

    pub fn write_nr30(&mut self, val: u8) {
        if self.on {
            self.channels.ch3.write_nr30(val);
        }
    }

    pub fn write_nr31(&mut self, val: u8) {
        if self.on {
            self.channels.ch3.write_nr31(val);
        }
    }

    pub fn write_nr32(&mut self, val: u8) {
        if self.on {
            self.channels.ch3.write_nr32(val);
        }
    }

    pub fn write_nr33(&mut self, val: u8) {
        if self.on {
            self.channels.ch3.write_nr33(val);
        }
    }

    pub fn write_nr34(&mut self, val: u8) {
        if self.on {
            self.channels.ch3.write_nr34(val);
        }
    }

    pub fn write_nr41(&mut self, val: u8) {
        if self.on {
            self.channels.ch4.write_nr41(val);
        }
    }

    pub fn write_nr42(&mut self, val: u8) {
        if self.on {
            self.channels.ch4.write_nr42(val);
        }
    }

    pub fn write_nr43(&mut self, val: u8) {
        if self.on {
            self.channels.ch4.write_nr43(val);
        }
    }

    pub fn write_nr44(&mut self, val: u8) {
        if self.on {
            self.channels.ch4.write_nr44(val);
        }
    }

    pub fn write_nr50(&mut self, val: u8) {
        if self.on {
            self.right_output_volume = val & 7;
            self.right_vin_on = val & (1 << 3) != 0;
            self.left_output_volume = (val >> 4) & 7;
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

    pub fn channel_on_iter(&self) -> ChannelOnIter {
        ChannelOnIter {
            channel_index: 0,
            nr51: self.nr51,
        }
    }
}

pub struct ChannelOnIter {
    channel_index: u8,
    nr51: u8,
}

impl Iterator for ChannelOnIter {
    type Item = (bool, bool);

    fn next(&mut self) -> Option<Self::Item> {
        if self.channel_index < 4 {
            let right_bit = 1 << self.channel_index;
            let left_bit = 1 << (self.channel_index + 4);
            let (left_on, right_on) = (self.nr51 & left_bit != 0, self.nr51 & right_bit != 0);

            self.channel_index += 1;
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

#[allow(clippy::cast_possible_truncation)]
const TIMER_RESET_VALUE: u16 = (T_CYCLES_PER_SECOND / 512) as u16;

pub struct Sequencer {
    timer: u16,
    counter: u8,
}

impl Sequencer {
    pub fn new() -> Self {
        Self {
            timer: TIMER_RESET_VALUE,
            counter: 0,
        }
    }

    pub fn tick(&mut self, channels: &mut Channels) {
        self.timer -= 1;
        if self.timer == 0 {
            self.timer = TIMER_RESET_VALUE;
            self.step(channels);
        }

        channels.step_sample();
    }

    fn step(&mut self, channels: &mut Channels) {
        if self.counter & 1 == 0 {
            channels.step_length();
            channels.set_period_half(0);
        } else {
            channels.set_period_half(1);
        }

        match self.counter {
            2 | 6 => channels.step_sweep(),
            7 => channels.step_envelope(),
            _ => (),
        }

        self.counter = (self.counter + 1) % 8;
    }
}
