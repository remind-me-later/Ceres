mod audio_callbacks;
mod channels;
mod control;
mod high_pass_filter;
mod sequencer;

use self::{
    channels::Channels,
    control::{Control, TriggerReset},
    high_pass_filter::HighPassFilter,
    sequencer::Sequencer,
};
use alloc::rc::Rc;
pub use audio_callbacks::AudioCallbacks;
use core::cell::RefCell;

pub type Sample = f32;

pub struct Frame {
    left: Sample,
    right: Sample,
}

impl Frame {
    fn new(left: Sample, right: Sample) -> Self {
        Self { left, right }
    }

    pub fn left(&self) -> Sample {
        self.left
    }

    pub fn right(&self) -> Sample {
        self.right
    }
}

pub struct Apu {
    channels: Channels,
    cycles_to_render: f32,
    cycles_until_next_render: f32,
    callbacks: Rc<RefCell<dyn AudioCallbacks>>,
    sequencer: Sequencer,
    control: Control,
    high_pass_filter: HighPassFilter,
}

impl Apu {
    pub fn new(callbacks: Rc<RefCell<dyn AudioCallbacks>>) -> Self {
        let cycles_to_render = callbacks.borrow().cycles_to_render();
        let sample_rate = callbacks.borrow().sample_rate();

        Self {
            channels: Channels::new(),
            cycles_to_render,
            cycles_until_next_render: 0.0,
            control: Control::new(),
            callbacks,
            sequencer: Sequencer::new(),
            high_pass_filter: HighPassFilter::new(sample_rate),
        }
    }

    pub fn tick(&mut self, mut mus_elapsed: u8) {
        if !self.control.is_enabled() {
            return;
        }

        while mus_elapsed > 0 {
            mus_elapsed -= 1;

            self.sequencer.tick(&mut self.channels);

            self.cycles_until_next_render -= 1.0;
            if self.cycles_until_next_render <= 0.0 {
                self.cycles_until_next_render += self.cycles_to_render;
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
        self.callbacks.borrow_mut().push_frame(frame);
    }

    fn reset(&mut self) {
        self.cycles_until_next_render = 0.0;
        self.channels.reset();
        self.control.reset();
    }

    pub fn read_nr10(&self) -> u8 {
        self.channels.square1.read_nr10()
    }

    pub fn read_nr11(&self) -> u8 {
        self.channels.square1.read_nr11()
    }

    pub fn read_nr12(&self) -> u8 {
        self.channels.square1.read_nr12()
    }

    pub fn read_nr13(&self) -> u8 {
        self.channels.square1.read_nr13()
    }

    pub fn read_nr14(&self) -> u8 {
        self.channels.square1.read_nr14()
    }

    pub fn read_nr21(&self) -> u8 {
        self.channels.square2.read_nr21()
    }

    pub fn read_nr22(&self) -> u8 {
        self.channels.square2.read_nr22()
    }

    pub fn read_nr23(&self) -> u8 {
        self.channels.square2.read_nr23()
    }

    pub fn read_nr24(&self) -> u8 {
        self.channels.square2.read_nr24()
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
        self.channels.wave.write_wave_ram(addr, val)
    }

    pub fn write_nr10(&mut self, val: u8) {
        if self.control.is_enabled() {
            self.channels.square1.write_nr10(val)
        }
    }

    pub fn write_nr11(&mut self, val: u8) {
        if self.control.is_enabled() {
            self.channels.square1.write_nr11(val)
        }
    }

    pub fn write_nr12(&mut self, val: u8) {
        if self.control.is_enabled() {
            self.channels.square1.write_nr12(val)
        }
    }

    pub fn write_nr13(&mut self, val: u8) {
        if self.control.is_enabled() {
            self.channels.square1.write_nr13(val)
        }
    }

    pub fn write_nr14(&mut self, val: u8) {
        if self.control.is_enabled() {
            self.channels.square1.write_nr14(val)
        }
    }

    pub fn write_nr21(&mut self, val: u8) {
        if self.control.is_enabled() {
            self.channels.square2.write_nr21(val)
        }
    }

    pub fn write_nr22(&mut self, val: u8) {
        if self.control.is_enabled() {
            self.channels.square2.write_nr22(val)
        }
    }

    pub fn write_nr23(&mut self, val: u8) {
        if self.control.is_enabled() {
            self.channels.square2.write_nr23(val)
        }
    }

    pub fn write_nr24(&mut self, val: u8) {
        if self.control.is_enabled() {
            self.channels.square2.write_nr24(val)
        }
    }

    pub fn write_nr30(&mut self, val: u8) {
        if self.control.is_enabled() {
            self.channels.wave.write_nr30(val)
        }
    }

    pub fn write_nr31(&mut self, val: u8) {
        if self.control.is_enabled() {
            self.channels.wave.write_nr31(val)
        }
    }

    pub fn write_nr32(&mut self, val: u8) {
        if self.control.is_enabled() {
            self.channels.wave.write_nr32(val)
        }
    }

    pub fn write_nr33(&mut self, val: u8) {
        if self.control.is_enabled() {
            self.channels.wave.write_nr33(val)
        }
    }

    pub fn write_nr34(&mut self, val: u8) {
        if self.control.is_enabled() {
            self.channels.wave.write_nr34(val)
        }
    }

    pub fn write_nr41(&mut self, val: u8) {
        if self.control.is_enabled() {
            self.channels.noise.write_nr41(val)
        }
    }

    pub fn write_nr42(&mut self, val: u8) {
        if self.control.is_enabled() {
            self.channels.noise.write_nr42(val)
        }
    }

    pub fn write_nr43(&mut self, val: u8) {
        if self.control.is_enabled() {
            self.channels.noise.write_nr43(val)
        }
    }

    pub fn write_nr44(&mut self, val: u8) {
        if self.control.is_enabled() {
            self.channels.noise.write_nr44(val)
        }
    }

    pub fn write_nr50(&mut self, val: u8) {
        if self.control.is_enabled() {
            self.control.write_nr50(val)
        }
    }

    pub fn write_nr51(&mut self, val: u8) {
        if self.control.is_enabled() {
            self.control.write_nr51(val)
        }
    }

    pub fn write_nr52(&mut self, val: u8) {
        if self.control.write_nr52(val) == TriggerReset::Reset {
            self.reset()
        }
    }
}
