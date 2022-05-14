pub use audio_callbacks::AudioCallbacks;
use {
    self::{
        channel::{Channels, LengthPeriodHalf},
        control::{Control, TriggerReset},
    },
    crate::T_CYCLES_PER_SECOND,
};

mod audio_callbacks;
mod channel;
mod control;

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
    control: Control,
    high_pass_filter: HighPassFilter,
}

impl Apu {
    pub fn new(callbacks: *mut dyn AudioCallbacks) -> Self {
        let cycles_to_render = unsafe { (*callbacks).cycles_to_render() };

        Self {
            channels: Channels::new(),
            cycles_to_render,
            cycles_until_next_render: cycles_to_render,
            control: Control::new(),
            callbacks,
            sequencer: Sequencer::new(),
            high_pass_filter: HighPassFilter::new(),
        }
    }

    pub fn tick(&mut self, mut mus_elapsed: u8) {
        if !self.control.is_enabled() {
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
            .zip(self.control.channel_enabled_terminal_iter())
            .fold(
                (0, 0),
                |(left, right), (output, (is_left_enabled, is_right_enabled))| {
                    (
                        left + output * u8::from(is_left_enabled),
                        right + output * u8::from(is_right_enabled),
                    )
                },
            );

        let (left_output_volume, right_output_volume) = self.control.output_volumes();

        // transform to i16 sample
        let left_sample = (0xf - left as i16 * 2) * i16::from(left_output_volume);
        let right_sample = (0xf - right as i16 * 2) * i16::from(right_output_volume);

        // filter
        let left_sample = self
            .high_pass_filter
            .filter(left_sample, self.control.is_enabled());
        let right_sample = self
            .high_pass_filter
            .filter(right_sample, self.control.is_enabled());

        let frame = Frame::new(left_sample, right_sample);
        unsafe { (*self.callbacks).push_frame(frame) };
    }

    fn reset(&mut self) {
        self.cycles_until_next_render = self.cycles_to_render;
        self.channels.reset();
        self.control.reset();
    }

    pub fn read_nr10(&self) -> u8 {
        self.channels.sq1.read_nr10()
    }

    pub fn read_nr11(&self) -> u8 {
        self.channels.sq1.read_nr11()
    }

    pub fn read_nr12(&self) -> u8 {
        self.channels.sq1.read_nr12()
    }

    pub fn read_nr14(&self) -> u8 {
        self.channels.sq1.read_nr14()
    }

    pub fn read_nr21(&self) -> u8 {
        self.channels.sq2.read_nr21()
    }

    pub fn read_nr22(&self) -> u8 {
        self.channels.sq2.read_nr22()
    }

    pub fn read_nr24(&self) -> u8 {
        self.channels.sq2.read_nr24()
    }

    pub fn read_nr30(&self) -> u8 {
        self.channels.wave.read_nr30()
    }

    pub fn read_nr32(&self) -> u8 {
        self.channels.wave.read_nr32()
    }

    pub fn read_nr34(&self) -> u8 {
        self.channels.wave.read_nr34()
    }

    pub fn read_nr42(&self) -> u8 {
        self.channels.noise.read_nr42()
    }

    pub fn read_nr43(&self) -> u8 {
        self.channels.noise.read_nr43()
    }

    pub fn read_nr44(&self) -> u8 {
        self.channels.noise.read_nr44()
    }

    pub fn read_nr50(&self) -> u8 {
        self.control.read_nr50()
    }

    pub fn read_nr51(&self) -> u8 {
        self.control.read_nr51()
    }

    pub fn read_nr52(&self) -> u8 {
        self.control.read_nr52(&self.channels)
    }

    pub fn read_wave(&self, addr: u8) -> u8 {
        self.channels.wave.read_wave_ram(addr)
    }

    pub fn write_wave(&mut self, addr: u8, val: u8) {
        self.channels.wave.write_wave_ram(addr, val);
    }

    pub fn write_nr10(&mut self, val: u8) {
        if self.control.is_enabled() {
            self.channels.sq1.write_nr10(val);
        }
    }

    pub fn write_nr11(&mut self, val: u8) {
        if self.control.is_enabled() {
            self.channels.sq1.write_nr11(val);
        }
    }

    pub fn write_nr12(&mut self, val: u8) {
        if self.control.is_enabled() {
            self.channels.sq1.write_nr12(val);
        }
    }

    pub fn write_nr13(&mut self, val: u8) {
        if self.control.is_enabled() {
            self.channels.sq1.write_nr13(val);
        }
    }

    pub fn write_nr14(&mut self, val: u8) {
        if self.control.is_enabled() {
            self.channels.sq1.write_nr14(val);
        }
    }

    pub fn write_nr21(&mut self, val: u8) {
        if self.control.is_enabled() {
            self.channels.sq2.write_nr21(val);
        }
    }

    pub fn write_nr22(&mut self, val: u8) {
        if self.control.is_enabled() {
            self.channels.sq2.write_nr22(val);
        }
    }

    pub fn write_nr23(&mut self, val: u8) {
        if self.control.is_enabled() {
            self.channels.sq2.write_nr23(val);
        }
    }

    pub fn write_nr24(&mut self, val: u8) {
        if self.control.is_enabled() {
            self.channels.sq2.write_nr24(val);
        }
    }

    pub fn write_nr30(&mut self, val: u8) {
        if self.control.is_enabled() {
            self.channels.wave.write_nr30(val);
        }
    }

    pub fn write_nr31(&mut self, val: u8) {
        if self.control.is_enabled() {
            self.channels.wave.write_nr31(val);
        }
    }

    pub fn write_nr32(&mut self, val: u8) {
        if self.control.is_enabled() {
            self.channels.wave.write_nr32(val);
        }
    }

    pub fn write_nr33(&mut self, val: u8) {
        if self.control.is_enabled() {
            self.channels.wave.write_nr33(val);
        }
    }

    pub fn write_nr34(&mut self, val: u8) {
        if self.control.is_enabled() {
            self.channels.wave.write_nr34(val);
        }
    }

    pub fn write_nr41(&mut self, val: u8) {
        if self.control.is_enabled() {
            self.channels.noise.write_nr41(val);
        }
    }

    pub fn write_nr42(&mut self, val: u8) {
        if self.control.is_enabled() {
            self.channels.noise.write_nr42(val);
        }
    }

    pub fn write_nr43(&mut self, val: u8) {
        if self.control.is_enabled() {
            self.channels.noise.write_nr43(val);
        }
    }

    pub fn write_nr44(&mut self, val: u8) {
        if self.control.is_enabled() {
            self.channels.noise.write_nr44(val);
        }
    }

    pub fn write_nr50(&mut self, val: u8) {
        if self.control.is_enabled() {
            self.control.write_nr50(val);
        }
    }

    pub fn write_nr51(&mut self, val: u8) {
        if self.control.is_enabled() {
            self.control.write_nr51(val);
        }
    }

    pub fn write_nr52(&mut self, val: u8) {
        if self.control.write_nr52(val) == TriggerReset::Reset {
            self.reset();
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

    pub fn filter(&mut self, input: i16, dac_enabled: bool) -> f32 {
        // TODO: amplification
        let input = (input * 32) as f32 / 32768.0;
        if dac_enabled {
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
        if self.counter % 2 == 0 {
            channels.step_length();
            channels.set_length_period_half(LengthPeriodHalf::First);
        } else {
            channels.set_length_period_half(LengthPeriodHalf::Second);
        }

        match self.counter {
            2 | 6 => channels.step_sweep(),
            7 => channels.step_envelope(),
            _ => (),
        }

        self.counter = (self.counter + 1) % 8;
    }
}