use super::{
    super::generic_channel::{GenericChannel, TriggerEvent},
    common::{SqCommon, MAX_SQUARE_CHANNEL_LENGTH},
    sweep::Sweep,
};

pub struct Square1 {
    sweep: Sweep,
    common: SqCommon,
}

impl Square1 {
    pub fn new() -> Self {
        Self {
            sweep: Sweep::new(),
            common: SqCommon::new(),
        }
    }

    pub fn reset(&mut self) {
        self.common.reset();
        self.sweep.reset();
    }

    pub fn read_nr10(&self) -> u8 {
        self.sweep.read()
    }

    pub fn read_nr11(&self) -> u8 {
        self.common.control_byte()
    }

    pub fn read_nr12(&self) -> u8 {
        self.common.read_envelope()
    }

    pub fn read_nr13(&self) -> u8 {
        self.common.low_byte()
    }

    pub fn read_nr14(&self) -> u8 {
        self.common.high_byte()
    }

    pub fn write_nr10(&mut self, val: u8) {
        self.sweep.write(val)
    }

    pub fn write_nr11(&mut self, val: u8) {
        self.common.set_control_byte(val)
    }

    pub fn write_nr12(&mut self, val: u8) {
        self.common.write_envelope(val)
    }

    pub fn write_nr13(&mut self, val: u8) {
        self.common.set_low_byte(val)
    }

    pub fn write_nr14(&mut self, val: u8) {
        if self.common.set_high_byte(val) == TriggerEvent::Trigger {
            self.trigger();
        }
    }

    pub fn step_sample(&mut self) {
        self.common.step_sample()
    }

    pub fn step_sweep(&mut self) {
        if self.is_enabled() {
            self.sweep.step(&mut self.common);
        }
    }

    pub fn step_envelope(&mut self) {
        self.common.step_envelope()
    }

    pub fn trigger(&mut self) {
        self.common.trigger();
        self.sweep.trigger(&mut self.common);
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
