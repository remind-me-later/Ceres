use super::{
    super::{generic_channel::GenericChannel, generic_channel::TriggerEvent},
    generic_square_channel::{GenericSquareChannel, MAX_SQUARE_CHANNEL_LENGTH},
};

#[derive(Clone, Copy)]
pub enum Register {
    NR21,
    NR22,
    NR23,
    NR24,
}

pub struct SquareChannel2 {
    generic_square_channel: GenericSquareChannel,
}

impl SquareChannel2 {
    pub fn new() -> Self {
        Self {
            generic_square_channel: GenericSquareChannel::new(),
        }
    }

    pub fn reset(&mut self) {
        self.generic_square_channel.reset();
    }

    pub fn read(&self, register: Register) -> u8 {
        match register {
            Register::NR21 => self.generic_square_channel.control_byte(),
            Register::NR22 => self.generic_square_channel.read_envelope(),
            Register::NR23 => self.generic_square_channel.low_byte(),
            Register::NR24 => self.generic_square_channel.high_byte(),
        }
    }

    pub fn write(&mut self, register: Register, val: u8) {
        match register {
            Register::NR21 => self.generic_square_channel.set_control_byte(val),
            Register::NR22 => self.generic_square_channel.wite_envelope(val),
            Register::NR23 => self.generic_square_channel.set_low_byte(val),
            Register::NR24 => {
                if self.generic_square_channel.set_high_byte(val) == TriggerEvent::Trigger {
                    self.trigger();
                }
            }
        }
    }

    pub fn step_sample(&mut self) {
        self.generic_square_channel.step_sample()
    }

    pub fn step_envelope(&mut self) {
        self.generic_square_channel.step_envelope();
    }

    pub fn trigger(&mut self) {
        self.generic_square_channel.trigger();
    }

    pub fn output_volume(&self) -> u8 {
        self.generic_square_channel.output_volume()
    }

    pub fn is_enabled(&self) -> bool {
        self.generic_square_channel.is_enabled()
    }

    pub fn step_length(&mut self) {
        self.generic_square_channel.step_length();
    }

    pub fn mut_generic_channel(&mut self) -> &mut GenericChannel<MAX_SQUARE_CHANNEL_LENGTH> {
        self.generic_square_channel.mut_generic_channel()
    }
}
