use super::{ccore::Ccore, envelope::Envelope};

const MAX_NOISE_CHANNEL_LENGTH: u16 = 64;

pub struct Noise {
    core: Ccore<MAX_NOISE_CHANNEL_LENGTH>,
    env: Envelope,
    current_timer_cycle: u16,
    internal_timer_period: u16,
    lfsr: u16,
    width_mode_on: bool,
    out: u8,
    nr43: u8,
}

impl Noise {
    pub fn new() -> Self {
        Self {
            core: Ccore::new(),
            env: Envelope::new(),
            current_timer_cycle: 1,
            internal_timer_period: 0,
            lfsr: 0x7fff,
            width_mode_on: false,
            out: 0,
            nr43: 0,
        }
    }

    pub fn reset(&mut self) {
        self.core.reset();
        self.env.reset();
        self.internal_timer_period = 0;
        self.current_timer_cycle = 1;
        self.lfsr = 0x7fff;
        self.width_mode_on = false;
        self.out = 0;
        self.nr43 = 0;
    }

    pub fn read_nr42(&self) -> u8 {
        self.env.read()
    }

    pub fn read_nr43(&self) -> u8 {
        self.nr43
    }

    pub fn read_nr44(&self) -> u8 {
        self.core.read()
    }

    pub fn write_nr41(&mut self, val: u8) {
        self.core.write_len(val);
    }

    pub fn write_nr42(&mut self, val: u8) {
        self.env.write(val, &mut self.core);
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
        if self.core.write_control(val) {
            self.trigger();
        }
    }

    pub fn trigger(&mut self) {
        self.current_timer_cycle = self.internal_timer_period;
        self.lfsr = 0x7fff;
        self.env.trigger();
    }

    pub fn step_envelope(&mut self) {
        self.env.step(&mut self.core);
    }

    pub fn step_sample(&mut self) {
        if !self.on() {
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

            self.out = if self.lfsr & 1 == 0 { 1 } else { 0 };
        } else {
            self.current_timer_cycle = new_timer_cycle;
        }
    }

    pub fn out(&self) -> u8 {
        self.out * self.env.volume()
    }

    pub fn on(&self) -> bool {
        self.core.on()
    }

    pub fn step_length(&mut self) {
        self.core.step_len();
    }

    pub fn mut_core(&mut self) -> &mut Ccore<MAX_NOISE_CHANNEL_LENGTH> {
        &mut self.core
    }
}
