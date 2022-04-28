use super::{
    envelope::Envelope,
    generic_channel::{GenericChannel, TriggerEvent},
};

const MAX_NOISE_CHANNEL_LENGTH: u16 = 64;

pub struct NoiseChannel {
    generic_channel: GenericChannel<MAX_NOISE_CHANNEL_LENGTH>,
    envelope: Envelope,
    current_timer_cycle: u16,
    internal_timer_period: u16,
    lfsr: u16,
    width_mode_on: bool,
    output_volume: u8,
    nr43: u8,
}

impl NoiseChannel {
    pub fn new() -> Self {
        Self {
            generic_channel: GenericChannel::new(),
            envelope: Envelope::new(),
            current_timer_cycle: 1,
            internal_timer_period: 0,
            lfsr: 0x7fff,
            width_mode_on: false,
            output_volume: 0,
            nr43: 0,
        }
    }

    pub fn reset(&mut self) {
        self.generic_channel.reset();
        self.envelope.reset();
        self.internal_timer_period = 0;
        self.current_timer_cycle = 1;
        self.lfsr = 0x7fff;
        self.width_mode_on = false;
        self.output_volume = 0;
        self.nr43 = 0;
    }

    pub fn read_nr42(&self) -> u8 {
        self.envelope.read()
    }

    pub fn read_nr43(&self) -> u8 {
        self.nr43
    }

    pub fn read_nr44(&self) -> u8 {
        self.generic_channel.read()
    }

    pub fn write_nr41(&mut self, val: u8) {
        self.generic_channel.write_sound_length(val)
    }

    pub fn write_nr42(&mut self, val: u8) {
        self.envelope.write(val, &mut self.generic_channel)
    }

    pub fn write_nr43(&mut self, val: u8) {
        self.nr43 = val;
        self.width_mode_on = val & 8 != 0;

        let clock_shift = val >> 4;
        let divisor: u16 = match val & 7 {
            0 => 8,
            1 => 16,
            2 => 32,
            3 => 48,
            4 => 64,
            5 => 80,
            6 => 96,
            _ => 112,
        };

        self.internal_timer_period = divisor << clock_shift;
        self.current_timer_cycle = 1;
    }

    pub fn write_nr44(&mut self, val: u8) {
        if self.generic_channel.write_control(val) == TriggerEvent::Trigger {
            self.trigger();
        }
    }

    pub fn trigger(&mut self) {
        self.current_timer_cycle = self.internal_timer_period;
        self.lfsr = 0x7fff;
        self.envelope.trigger();
    }

    pub fn step_envelope(&mut self) {
        self.envelope.step(&mut self.generic_channel);
    }

    pub fn step_sample(&mut self) {
        if !self.is_enabled() {
            return;
        }

        let (new_timer_cycle, overflow) = self.current_timer_cycle.overflowing_sub(1);

        if overflow {
            self.current_timer_cycle = self.internal_timer_period;

            let xor_bit = (self.lfsr & 1) ^ ((self.lfsr & 2) >> 1);
            self.lfsr >>= 1;
            self.lfsr |= xor_bit << 14;
            if self.width_mode_on {
                self.lfsr |= xor_bit << 6;
            }

            self.output_volume = if self.lfsr & 1 == 0 { 1 } else { 0 };
        } else {
            self.current_timer_cycle = new_timer_cycle;
        }
    }

    pub fn output_volume(&self) -> u8 {
        self.output_volume * self.envelope.volume()
    }

    pub fn is_enabled(&self) -> bool {
        self.generic_channel.is_enabled()
    }

    pub fn step_length(&mut self) {
        self.generic_channel.step_length();
    }

    pub fn mut_generic_channel(&mut self) -> &mut GenericChannel<MAX_NOISE_CHANNEL_LENGTH> {
        &mut self.generic_channel
    }
}
