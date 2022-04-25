extern crate alloc;

mod audio_callbacks;
mod channels;
mod control;
mod high_pass_filter;
mod sequencer;

use self::{
    channels::{ChannelRegister::*, Channels},
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

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ApuRegister {
    NR10,
    NR11,
    NR12,
    NR13,
    NR14,
    NR21,
    NR22,
    NR23,
    NR24,
    NR30,
    NR31,
    NR32,
    NR33,
    NR34,
    NR41,
    NR42,
    NR43,
    NR44,
    NR50,
    NR51,
    NR52,
}

pub enum ApuIO {
    ApuRegister(ApuRegister),
    WavePatternRam { address: u8 },
}

#[derive(Clone, Copy)]
enum Volume {
    Vol0,
    Vol1,
    Vol2,
    Vol3,
    Vol4,
    Vol5,
    Vol6,
    Vol7,
}

impl From<u8> for Volume {
    fn from(val: u8) -> Self {
        use Volume::{Vol0, Vol1, Vol2, Vol3, Vol4, Vol5, Vol6, Vol7};
        match val & 0x07 {
            0 => Vol0,
            1 => Vol1,
            2 => Vol2,
            3 => Vol3,
            4 => Vol4,
            5 => Vol5,
            6 => Vol6,
            _ => Vol7,
        }
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

    pub fn tick(&mut self, mut microseconds_elapsed_times_16: u8) {
        if !self.control.is_enabled() {
            return;
        }

        while microseconds_elapsed_times_16 > 0 {
            microseconds_elapsed_times_16 -= 1;

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

    pub fn read(&self, io: ApuIO) -> u8 {
        match io {
            ApuIO::ApuRegister(register) => match register {
                ApuRegister::NR10 => self.channels.read(NR10),
                ApuRegister::NR11 => self.channels.read(NR11),
                ApuRegister::NR12 => self.channels.read(NR12),
                ApuRegister::NR13 => self.channels.read(NR13),
                ApuRegister::NR14 => self.channels.read(NR14),
                ApuRegister::NR21 => self.channels.read(NR21),
                ApuRegister::NR22 => self.channels.read(NR22),
                ApuRegister::NR23 => self.channels.read(NR23),
                ApuRegister::NR24 => self.channels.read(NR24),
                ApuRegister::NR30 => self.channels.read(NR30),
                ApuRegister::NR31 => self.channels.read(NR31),
                ApuRegister::NR32 => self.channels.read(NR32),
                ApuRegister::NR33 => self.channels.read(NR33),
                ApuRegister::NR34 => self.channels.read(NR34),
                ApuRegister::NR41 => self.channels.read(NR41),
                ApuRegister::NR42 => self.channels.read(NR42),
                ApuRegister::NR43 => self.channels.read(NR43),
                ApuRegister::NR44 => self.channels.read(NR44),
                ApuRegister::NR50 => self.control.read_nr50(),
                ApuRegister::NR51 => self.control.read_nr51(),
                ApuRegister::NR52 => self.control.read_nr52(&self.channels),
            },
            ApuIO::WavePatternRam { address } => self.channels.read_wave_ram(address),
        }
    }

    pub fn write(&mut self, io: ApuIO, val: u8) {
        match io {
            ApuIO::ApuRegister(register) => {
                if register == ApuRegister::NR52 {
                    if self.control.write_nr52(val) == TriggerReset::Reset {
                        self.reset()
                    }
                    return;
                }

                if self.control.is_enabled() {
                    match register {
                        ApuRegister::NR10 => self.channels.write(NR10, val),
                        ApuRegister::NR11 => self.channels.write(NR11, val),
                        ApuRegister::NR12 => self.channels.write(NR12, val),
                        ApuRegister::NR13 => self.channels.write(NR13, val),
                        ApuRegister::NR14 => self.channels.write(NR14, val),
                        ApuRegister::NR21 => self.channels.write(NR21, val),
                        ApuRegister::NR22 => self.channels.write(NR22, val),
                        ApuRegister::NR23 => self.channels.write(NR23, val),
                        ApuRegister::NR24 => self.channels.write(NR24, val),
                        ApuRegister::NR30 => self.channels.write(NR30, val),
                        ApuRegister::NR31 => self.channels.write(NR31, val),
                        ApuRegister::NR32 => self.channels.write(NR32, val),
                        ApuRegister::NR33 => self.channels.write(NR33, val),
                        ApuRegister::NR34 => self.channels.write(NR34, val),
                        ApuRegister::NR41 => self.channels.write(NR41, val),
                        ApuRegister::NR42 => self.channels.write(NR42, val),
                        ApuRegister::NR43 => self.channels.write(NR43, val),
                        ApuRegister::NR44 => self.channels.write(NR44, val),
                        ApuRegister::NR50 => self.control.write_nr50(val),
                        ApuRegister::NR51 => self.control.write_nr51(val),
                        ApuRegister::NR52 => unreachable!(),
                    }
                }
            }
            ApuIO::WavePatternRam { address } => self.channels.write_wave_ram(address, val),
        }
    }
}
