use super::{
    super::generic_channel::{GenericChannel, TriggerEvent},
    generic_square_channel::{GenericSquareChannel, MAX_SQUARE_CHANNEL_LENGTH},
    sweep::Sweep,
};

#[derive(Clone, Copy)]
pub enum Register {
    NR10,
    NR11,
    NR12,
    NR13,
    NR14,
}

pub struct SquareChannel1 {
    sweep: Sweep,
    generic_square_channel: GenericSquareChannel,
}

impl SquareChannel1 {
    pub fn new() -> Self {
        Self {
            sweep: Sweep::new(),
            generic_square_channel: GenericSquareChannel::new(),
        }
    }

    pub fn reset(&mut self) {
        self.generic_square_channel.reset();
        self.sweep.reset();
    }

    pub fn read(&self, register: Register) -> u8 {
        match register {
            Register::NR10 => self.sweep.read(),
            Register::NR11 => self.generic_square_channel.control_byte(),
            Register::NR12 => self.generic_square_channel.read_envelope(),
            Register::NR13 => self.generic_square_channel.low_byte(),
            Register::NR14 => self.generic_square_channel.high_byte(),
        }
    }

    pub fn write(&mut self, register: Register, val: u8) {
        match register {
            Register::NR10 => self.sweep.write(val),
            Register::NR11 => self.generic_square_channel.set_control_byte(val),
            Register::NR12 => self.generic_square_channel.wite_envelope(val),
            Register::NR13 => self.generic_square_channel.set_low_byte(val),
            Register::NR14 => {
                if self.generic_square_channel.set_high_byte(val) == TriggerEvent::Trigger {
                    self.trigger();
                }
            }
        }
    }

    pub fn step_sample(&mut self) {
        self.generic_square_channel.step_sample()
    }

    pub fn step_sweep(&mut self) {
        if self.is_enabled() {
            self.sweep.step(&mut self.generic_square_channel);
        }
    }

    pub fn step_envelope(&mut self) {
        self.generic_square_channel.step_envelope()
    }

    pub fn trigger(&mut self) {
        self.generic_square_channel.trigger();
        self.sweep.trigger(&mut self.generic_square_channel);
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
