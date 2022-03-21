use crate::audio::channels::envelope::Envelope;

use super::super::{
    frequency_data::FrequencyData,
    generic_channel::{GenericChannel, TriggerEvent},
};
use super::wave_duty::WaveDuty;

const SQUARE_CHANNEL_PERIOD_MULTIPLIER: u16 = 4;
pub const MAX_SQUARE_CHANNEL_LENGTH: u16 = 64;

pub struct GenericSquareChannel {
    frequency: FrequencyData<SQUARE_CHANNEL_PERIOD_MULTIPLIER>,
    duty_cycle: WaveDuty,
    duty_cycle_bit: u8,
    generic_channel: GenericChannel<MAX_SQUARE_CHANNEL_LENGTH>,
    current_frequency_period: u16,
    last_output: u8,
    envelope: Envelope,
}

impl GenericSquareChannel {
    pub fn new() -> Self {
        let frequency = FrequencyData::new();
        let current_frequency_period = frequency.period();

        Self {
            frequency,
            current_frequency_period,
            duty_cycle: WaveDuty::Half,
            duty_cycle_bit: 0,
            generic_channel: GenericChannel::new(),
            last_output: 0,
            envelope: Envelope::new(),
        }
    }

    pub fn control_byte(&self) -> u8 {
        const MASK: u8 = 0x3f;
        MASK | u8::from(self.duty_cycle)
    }

    pub fn set_control_byte(&mut self, value: u8) {
        self.duty_cycle = value.into();
        self.generic_channel.write_sound_length(value);
    }

    pub const fn low_byte(&self) -> u8 {
        0xff
    }

    pub fn set_low_byte(&mut self, value: u8) {
        self.frequency.set_low_byte(value);
    }

    pub fn high_byte(&self) -> u8 {
        self.generic_channel.read()
    }

    pub fn set_high_byte(&mut self, value: u8) -> TriggerEvent {
        self.frequency.set_high_byte(value);
        self.generic_channel.write_control(value)
    }

    pub fn trigger(&mut self) {
        self.current_frequency_period = self.frequency.period();
        self.last_output = 0;
        self.duty_cycle_bit = 0;
        self.envelope.trigger();
    }

    pub fn reset(&mut self) {
        self.generic_channel.reset();
        self.frequency.reset();
        self.duty_cycle = WaveDuty::HalfQuarter;
        self.duty_cycle_bit = 0;
        self.envelope.reset();
    }

    pub fn next_duty_cycle_value(&mut self) -> u8 {
        let output =
            (self.duty_cycle.duty_byte() & (1 << self.duty_cycle_bit)) >> self.duty_cycle_bit;
        self.duty_cycle_bit = (self.duty_cycle_bit + 1) % 8;
        output
    }

    pub const fn frequency(&self) -> &FrequencyData<SQUARE_CHANNEL_PERIOD_MULTIPLIER> {
        &self.frequency
    }

    pub fn set_frequency_data(&mut self, frequency: u16) {
        self.frequency.set(frequency);
    }

    pub fn set_enabled(&mut self, val: bool) {
        self.generic_channel.set_enabled(val)
    }

    pub fn mut_generic_channel(&mut self) -> &mut GenericChannel<MAX_SQUARE_CHANNEL_LENGTH> {
        &mut self.generic_channel
    }

    pub fn step_sample(&mut self) {
        if !self.generic_channel.is_enabled() {
            return;
        }

        let (new_frequency_period, overflow) = self.current_frequency_period.overflowing_sub(1);

        if overflow {
            self.current_frequency_period = self.frequency.period();

            if self.generic_channel.is_enabled() {
                self.last_output = self.next_duty_cycle_value();
            }
        } else {
            self.current_frequency_period = new_frequency_period;
        }
    }

    pub fn output_volume(&self) -> u8 {
        self.last_output * self.envelope.volume()
    }

    pub fn step_envelope(&mut self) {
        self.envelope.step(&mut self.generic_channel);
    }

    pub fn read_envelope(&self) -> u8 {
        self.envelope.read()
    }

    pub fn wite_envelope(&mut self, value: u8) {
        self.envelope.write(value, &mut self.generic_channel);
    }

    pub fn step_length(&mut self) {
        self.generic_channel.step_length();
    }

    pub const fn is_enabled(&self) -> bool {
        self.generic_channel.is_enabled()
    }
}
