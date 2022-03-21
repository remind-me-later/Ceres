mod envelope;
mod frequency_data;
mod generic_channel;
mod noise_channel;
mod square_channels;
mod wave_channel;

use self::{
    noise_channel::Register::{NR41, NR42, NR43, NR44},
    square_channels::{
        square_channel_1::Register::{NR10, NR11, NR12, NR13, NR14},
        square_channel_2::Register::{NR21, NR22, NR23, NR24},
    },
    wave_channel::Register::{NR30, NR31, NR32, NR33, NR34},
};
use noise_channel::NoiseChannel;
use square_channels::square_channel_1::SquareChannel1;
use square_channels::square_channel_2::SquareChannel2;
use wave_channel::WaveChannel;

pub use generic_channel::LengthPeriodHalf;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ChannelRegister {
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
}

pub struct Channels {
    square_channel_1: SquareChannel1,
    square_channel_2: SquareChannel2,
    wave_channel: WaveChannel,
    noise_channel: NoiseChannel,
}

impl Channels {
    pub fn new() -> Self {
        Self {
            square_channel_1: SquareChannel1::new(),
            square_channel_2: SquareChannel2::new(),
            wave_channel: WaveChannel::new(),
            noise_channel: NoiseChannel::new(),
        }
    }

    pub fn read(&self, register: ChannelRegister) -> u8 {
        match register {
            ChannelRegister::NR10 => self.square_channel_1.read(NR10),
            ChannelRegister::NR11 => self.square_channel_1.read(NR11),
            ChannelRegister::NR12 => self.square_channel_1.read(NR12),
            ChannelRegister::NR13 => self.square_channel_1.read(NR13),
            ChannelRegister::NR14 => self.square_channel_1.read(NR14),
            ChannelRegister::NR21 => self.square_channel_2.read(NR21),
            ChannelRegister::NR22 => self.square_channel_2.read(NR22),
            ChannelRegister::NR23 => self.square_channel_2.read(NR23),
            ChannelRegister::NR24 => self.square_channel_2.read(NR24),
            ChannelRegister::NR30 => self.wave_channel.read(NR30),
            ChannelRegister::NR31 => self.wave_channel.read(NR31),
            ChannelRegister::NR32 => self.wave_channel.read(NR32),
            ChannelRegister::NR33 => self.wave_channel.read(NR33),
            ChannelRegister::NR34 => self.wave_channel.read(NR34),
            ChannelRegister::NR41 => self.noise_channel.read(NR41),
            ChannelRegister::NR42 => self.noise_channel.read(NR42),
            ChannelRegister::NR43 => self.noise_channel.read(NR43),
            ChannelRegister::NR44 => self.noise_channel.read(NR44),
        }
    }

    pub fn write(&mut self, register: ChannelRegister, val: u8) {
        match register {
            ChannelRegister::NR10 => self.square_channel_1.write(NR10, val),
            ChannelRegister::NR11 => self.square_channel_1.write(NR11, val),
            ChannelRegister::NR12 => self.square_channel_1.write(NR12, val),
            ChannelRegister::NR13 => self.square_channel_1.write(NR13, val),
            ChannelRegister::NR14 => self.square_channel_1.write(NR14, val),
            ChannelRegister::NR21 => self.square_channel_2.write(NR21, val),
            ChannelRegister::NR22 => self.square_channel_2.write(NR22, val),
            ChannelRegister::NR23 => self.square_channel_2.write(NR23, val),
            ChannelRegister::NR24 => self.square_channel_2.write(NR24, val),
            ChannelRegister::NR30 => self.wave_channel.write(NR30, val),
            ChannelRegister::NR31 => self.wave_channel.write(NR31, val),
            ChannelRegister::NR32 => self.wave_channel.write(NR32, val),
            ChannelRegister::NR33 => self.wave_channel.write(NR33, val),
            ChannelRegister::NR34 => self.wave_channel.write(NR34, val),
            ChannelRegister::NR41 => self.noise_channel.write(NR41, val),
            ChannelRegister::NR42 => self.noise_channel.write(NR42, val),
            ChannelRegister::NR43 => self.noise_channel.write(NR43, val),
            ChannelRegister::NR44 => self.noise_channel.write(NR44, val),
        }
    }

    pub fn read_wave_ram(&self, address: u8) -> u8 {
        self.wave_channel.read_wave_ram(address)
    }

    pub fn write_wave_ram(&mut self, address: u8, value: u8) {
        self.wave_channel.write_wave_ram(address, value);
    }

    pub fn read_enable_u4(&self) -> u8 {
        (u8::from(self.noise_channel.is_enabled()) << 3)
            | (u8::from(self.wave_channel.is_enabled()) << 2)
            | (u8::from(self.square_channel_2.is_enabled()) << 1)
            | u8::from(self.square_channel_1.is_enabled())
    }

    pub fn set_length_period_half(&mut self, length_period_half: LengthPeriodHalf) {
        self.square_channel_1
            .mut_generic_channel()
            .set_length_period_half(length_period_half);
        self.square_channel_2
            .mut_generic_channel()
            .set_length_period_half(length_period_half);
        self.wave_channel
            .mut_generic_channel()
            .set_length_period_half(length_period_half);
        self.noise_channel
            .mut_generic_channel()
            .set_length_period_half(length_period_half);
    }

    pub fn step_length(&mut self) {
        self.square_channel_1.step_length();
        self.square_channel_2.step_length();
        self.wave_channel.step_length();
        self.noise_channel.step_length();
    }

    pub fn step_sample(&mut self) {
        self.square_channel_1.step_sample();
        self.square_channel_2.step_sample();
        self.wave_channel.step_sample();
        self.noise_channel.step_sample();
    }

    pub fn step_sweep(&mut self) {
        self.square_channel_1.step_sweep();
    }

    pub fn step_envelope(&mut self) {
        self.square_channel_1.step_envelope();
        self.square_channel_2.step_envelope();
        self.noise_channel.step_envelope();
    }

    pub fn reset(&mut self) {
        self.square_channel_1.reset();
        self.square_channel_2.reset();
        self.wave_channel.reset();
        self.noise_channel.reset();
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
                self.channels.square_channel_1.output_volume()
                    * u8::from(self.channels.square_channel_1.is_enabled()),
            ),
            1 => Some(
                self.channels.square_channel_2.output_volume()
                    * u8::from(self.channels.square_channel_2.is_enabled()),
            ),
            2 => Some(
                self.channels.wave_channel.output_volume()
                    * u8::from(self.channels.wave_channel.is_enabled()),
            ),
            3 => Some(
                self.channels.noise_channel.output_volume()
                    * u8::from(self.channels.noise_channel.is_enabled()),
            ),
            _ => return None,
        };

        self.channel_index += 1;

        value
    }
}
