use super::{
    common::{Common, Freq},
    envelope::Envelope,
};

const SQ_MAX_LEN: u16 = 64;
const SQ_PERIOD_MUL: u16 = 4;
const DUTY_TABLE: [u8; 4] = [0b0000_0001, 0b1000_0001, 0b1000_0111, 0b0111_1110];

struct Square {
    pub com: Common<SQ_MAX_LEN>,
    pub freq: Freq<SQ_PERIOD_MUL>,
    duty: u8,
    duty_bit: u8,
    period: u16,
    out: u8,
    env: Envelope,
}

impl Square {
    fn new() -> Self {
        let freq = Freq::new();
        let period = freq.period();

        Self {
            freq,
            period,
            duty: DUTY_TABLE[0],
            duty_bit: 0,
            com: Common::new(),
            out: 0,
            env: Envelope::new(),
        }
    }

    fn control_byte(&self) -> u8 {
        0x3f | ((self.duty as u8) << 6)
    }

    fn set_control_byte(&mut self, val: u8) {
        self.duty = (val >> 6) & 3;
        self.com.write_len(val);
    }

    fn set_high_byte(&mut self, val: u8) -> bool {
        self.freq.set_hi(val);
        self.com.write_control(val)
    }

    fn trigger(&mut self) {
        self.period = self.freq.period();
        self.out = 0;
        self.duty_bit = 0;
        self.env.trigger();
    }

    fn reset(&mut self) {
        self.com.reset();
        self.freq.reset();
        self.duty = DUTY_TABLE[0];
        self.duty_bit = 0;
        self.env.reset();
    }

    pub fn duty_output(&mut self) -> u8 {
        let out = (DUTY_TABLE[self.duty as usize] & (1 << self.duty_bit)) >> self.duty_bit;
        self.duty_bit = (self.duty_bit + 1) & 7;
        out
    }

    fn step_sample(&mut self) {
        if !self.com.on() {
            return;
        }

        let (period, overflow) = self.period.overflowing_sub(1);

        if overflow {
            self.period = self.freq.period();

            if self.com.on() {
                self.out = self.duty_output();
            }
        } else {
            self.period = period;
        }
    }
}

pub struct Ch1 {
    sq: Square,
    // sweep
    sw_on: bool,
    sw_dec: bool,
    sw_period: u8,
    sw_shift: u8,
    sw_timer: u8,
    sw_shadow_freq: u16,
}

impl Ch1 {
    pub fn new() -> Self {
        Self {
            sw_period: 8,
            sw_dec: false,
            sw_shift: 0,
            sw_timer: 8,
            sw_shadow_freq: 0,
            sw_on: false,
            sq: Square::new(),
        }
    }

    pub fn reset(&mut self) {
        self.sq.reset();
        self.sw_period = 8;
        self.sw_timer = 8;
        self.sw_dec = false;
        self.sw_shift = 0;
        self.sw_on = false;
    }

    fn sweep_calculation(&mut self) {
        let tmp = self.sw_shadow_freq >> self.sw_shift;
        self.sw_shadow_freq = if self.sw_dec {
            self.sw_shadow_freq - tmp
        } else {
            self.sw_shadow_freq + tmp
        };

        if self.sw_shadow_freq > 0x7ff {
            self.sq.com.set_on(false);
        } else if self.sw_shift != 0 {
            self.sq.freq.val = self.sw_shadow_freq & 0x7ff;
        }
    }

    pub fn read_nr10(&self) -> u8 {
        0x80 | ((self.sw_period as u8 & 7) << 4) | ((!self.sw_dec as u8) << 3) | self.sw_shift
    }

    pub fn read_nr11(&self) -> u8 {
        self.sq.control_byte()
    }

    pub fn read_nr12(&self) -> u8 {
        self.sq.env.read()
    }

    pub fn read_nr14(&self) -> u8 {
        self.sq.com.read()
    }

    pub fn write_nr10(&mut self, val: u8) {
        self.sw_period = (val >> 4) & 7;
        if self.sw_period == 0 {
            self.sw_period = 8;
        }
        self.sw_dec = (val & 8) != 0;
        self.sw_shift = val & 7;
    }

    pub fn write_nr11(&mut self, val: u8) {
        self.sq.set_control_byte(val);
    }

    pub fn write_nr12(&mut self, val: u8) {
        self.sq.env.write(val, &mut self.sq.com);
    }

    pub fn write_nr13(&mut self, val: u8) {
        self.sq.freq.set_lo(val);
    }

    pub fn write_nr14(&mut self, val: u8) {
        if self.sq.set_high_byte(val) {
            self.trigger();
        }
    }

    pub fn step_sample(&mut self) {
        self.sq.step_sample();
    }

    pub fn step_sweep(&mut self) {
        if self.on() {
            self.sw_timer -= 1;
            if self.sw_timer == 0 {
                self.sw_timer = self.sw_period;
                if self.sw_on {
                    self.sweep_calculation();
                }
            }
        }
    }

    pub fn step_envelope(&mut self) {
        self.sq.env.step(&mut self.sq.com);
    }

    pub fn trigger(&mut self) {
        self.sq.trigger();
        self.sw_shadow_freq = self.sq.freq.val;
        self.sw_timer = self.sw_period;
        self.sw_on = self.sw_period != 8 && self.sw_shift != 0;

        if self.sw_shift != 0 {
            self.sweep_calculation();
        }
    }

    pub fn out(&self) -> u8 {
        self.sq.out * self.sq.env.vol
    }

    pub fn on(&self) -> bool {
        self.sq.com.on()
    }

    pub fn step_len(&mut self) {
        self.sq.com.step_len();
    }

    pub fn mut_com(&mut self) -> &mut Common<SQ_MAX_LEN> {
        &mut self.sq.com
    }
}

pub struct Ch2 {
    sq: Square,
}

impl Ch2 {
    pub fn new() -> Self {
        Self { sq: Square::new() }
    }

    pub fn reset(&mut self) {
        self.sq.reset();
    }

    pub fn read_nr21(&self) -> u8 {
        self.sq.control_byte()
    }

    pub fn read_nr22(&self) -> u8 {
        self.sq.env.read()
    }

    pub fn read_nr24(&self) -> u8 {
        self.sq.com.read()
    }

    pub fn write_nr21(&mut self, val: u8) {
        self.sq.set_control_byte(val);
    }

    pub fn write_nr22(&mut self, val: u8) {
        self.sq.env.write(val, &mut self.sq.com);
    }

    pub fn write_nr23(&mut self, val: u8) {
        self.sq.freq.set_lo(val);
    }

    pub fn write_nr24(&mut self, val: u8) {
        if self.sq.set_high_byte(val) {
            self.trigger();
        }
    }

    pub fn step_sample(&mut self) {
        self.sq.step_sample();
    }

    pub fn step_envelope(&mut self) {
        self.sq.env.step(&mut self.sq.com);
    }

    pub fn trigger(&mut self) {
        self.sq.trigger();
    }

    pub fn out(&self) -> u8 {
        self.sq.out * self.sq.env.vol
    }

    pub fn on(&self) -> bool {
        self.sq.com.on()
    }

    pub fn step_len(&mut self) {
        self.sq.com.step_len();
    }

    pub fn mut_com(&mut self) -> &mut Common<SQ_MAX_LEN> {
        &mut self.sq.com
    }
}
