use super::{
    super::generic_channel::{GenericChannel, TriggerEvent},
    common::{SqCommon, MAX_SQUARE_CHANNEL_LENGTH},
};

pub struct Square2 {
    common: SqCommon,
}

impl Square2 {
    pub fn new() -> Self {
        Self {
            common: SqCommon::new(),
        }
    }

    pub fn reset(&mut self) {
        self.common.reset();
    }

    pub fn read_nr21(&self) -> u8 {
        self.common.control_byte()
    }

    pub fn read_nr22(&self) -> u8 {
        self.common.read_envelope()
    }

    pub fn read_nr23(&self) -> u8 {
        self.common.low_byte()
    }

    pub fn read_nr24(&self) -> u8 {
        self.common.high_byte()
    }

    pub fn write_nr21(&mut self, val: u8) {
        self.common.set_control_byte(val)
    }

    pub fn write_nr22(&mut self, val: u8) {
        self.common.write_envelope(val)
    }

    pub fn write_nr23(&mut self, val: u8) {
        self.common.set_low_byte(val)
    }

    pub fn write_nr24(&mut self, val: u8) {
        if self.common.set_high_byte(val) == TriggerEvent::Trigger {
            self.trigger();
        }
    }

    pub fn step_sample(&mut self) {
        self.common.step_sample()
    }

    pub fn step_envelope(&mut self) {
        self.common.step_envelope();
    }

    pub fn trigger(&mut self) {
        self.common.trigger();
    }

    pub fn output_volume(&self) -> u8 {
        self.common.output_volume()
    }

    pub fn is_enabled(&self) -> bool {
        self.common.is_enabled()
    }

    pub fn step_length(&mut self) {
        self.common.step_length();
    }

    pub fn mut_generic_channel(&mut self) -> &mut GenericChannel<MAX_SQUARE_CHANNEL_LENGTH> {
        self.common.mut_generic_channel()
    }
}
