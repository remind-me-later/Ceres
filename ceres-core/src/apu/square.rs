use super::{ccore::Ccore, envelope::Envelope, freq::Freq};

const SQUARE_MAX_LENGTH: u16 = 64;
const SQUARE_PERIOD_MULTIPLIER: u16 = 4;
const DUTY_TABLE: [u8; 4] = [0b0000_0001, 0b1000_0001, 0b1000_0111, 0b0111_1110];

struct Sweep {
    on: bool,
    dec: bool,
    period: u8,
    shift: u8,
    timer: u8,
    shadow_freq: u16,
}

impl Sweep {
    fn new() -> Self {
        Self {
            period: 8,
            dec: false,
            shift: 0,
            timer: 8,
            shadow_freq: 0,
            on: false,
        }
    }

    fn trigger(&mut self, common: &mut Common) {
        self.shadow_freq = common.freq.val;
        self.timer = self.period;
        self.on = self.period != 8 && self.shift != 0;

        if self.shift != 0 {
            self.sweep_calculation(common);
        }
    }

    fn reset(&mut self) {
        self.period = 8;
        self.timer = 8;
        self.dec = false;
        self.shift = 0;
        self.on = false;
    }

    fn sweep_calculation(&mut self, common: &mut Common) {
        let tmp = self.shadow_freq >> self.shift;
        self.shadow_freq = if self.dec {
            self.shadow_freq - tmp
        } else {
            self.shadow_freq + tmp
        };

        if self.shadow_freq > 0x7ff {
            common.set_on(false);
        } else if self.shift != 0 {
            common.freq.val = self.shadow_freq & 0x7ff;
        }
    }

    fn step(&mut self, common: &mut Common) {
        self.timer -= 1;
        if self.timer == 0 {
            self.timer = self.period;
            if self.on {
                self.sweep_calculation(common);
            }
        }
    }

    fn read(&self) -> u8 {
        0x80 | ((self.period as u8 & 7) << 4) | ((!self.dec as u8) << 3) | self.shift
    }

    fn write(&mut self, val: u8) {
        self.period = (val >> 4) & 7;
        if self.period == 0 {
            self.period = 8;
        }
        self.dec = (val & 8) != 0;
        self.shift = val & 7;
    }
}

struct Common {
    pub core: Ccore<SQUARE_MAX_LENGTH>,
    pub freq: Freq<SQUARE_PERIOD_MULTIPLIER>,
    duty: u8,
    duty_bit: u8,
    period: u16,
    out: u8,
    env: Envelope,
}

impl Common {
    fn new() -> Self {
        let freq = Freq::new();
        let period = freq.period();

        Self {
            freq,
            period,
            duty: DUTY_TABLE[0],
            duty_bit: 0,
            core: Ccore::new(),
            out: 0,
            env: Envelope::new(),
        }
    }

    fn control_byte(&self) -> u8 {
        0x3f | ((self.duty as u8) << 6)
    }

    fn set_control_byte(&mut self, val: u8) {
        self.duty = (val >> 6) & 3;
        self.core.write_len(val);
    }

    fn set_low_byte(&mut self, val: u8) {
        self.freq.set_lo(val);
    }

    fn high_byte(&self) -> u8 {
        self.core.read()
    }

    fn set_high_byte(&mut self, val: u8) -> bool {
        self.freq.set_hi(val);
        self.core.write_control(val)
    }

    fn trigger(&mut self) {
        self.period = self.freq.period();
        self.out = 0;
        self.duty_bit = 0;
        self.env.trigger();
    }

    fn reset(&mut self) {
        self.core.reset();
        self.freq.reset();
        self.duty = DUTY_TABLE[0];
        self.duty_bit = 0;
        self.env.reset();
    }

    pub fn duty_output(&mut self) -> u8 {
        let output = (DUTY_TABLE[self.duty as usize] & (1 << self.duty_bit)) >> self.duty_bit;
        self.duty_bit = (self.duty_bit + 1) & 7;
        output
    }

    fn set_on(&mut self, on: bool) {
        self.core.set_on(on);
    }

    fn step_sample(&mut self) {
        if !self.core.on() {
            return;
        }

        let (period, overflow) = self.period.overflowing_sub(1);

        if overflow {
            self.period = self.freq.period();

            if self.core.on() {
                self.out = self.duty_output();
            }
        } else {
            self.period = period;
        }
    }

    fn out(&self) -> u8 {
        self.out * self.env.volume()
    }

    fn step_envelope(&mut self) {
        self.env.step(&mut self.core);
    }

    fn read_envelope(&self) -> u8 {
        self.env.read()
    }

    fn write_envelope(&mut self, val: u8) {
        self.env.write(val, &mut self.core);
    }

    fn step_length(&mut self) {
        self.core.step_len();
    }

    fn on(&self) -> bool {
        self.core.on()
    }
}

pub struct Ch1 {
    sweep: Sweep,
    common: Common,
}

impl Ch1 {
    pub fn new() -> Self {
        Self {
            sweep: Sweep::new(),
            common: Common::new(),
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

    pub fn read_nr14(&self) -> u8 {
        self.common.high_byte()
    }

    pub fn write_nr10(&mut self, val: u8) {
        self.sweep.write(val);
    }

    pub fn write_nr11(&mut self, val: u8) {
        self.common.set_control_byte(val);
    }

    pub fn write_nr12(&mut self, val: u8) {
        self.common.write_envelope(val);
    }

    pub fn write_nr13(&mut self, val: u8) {
        self.common.set_low_byte(val);
    }

    pub fn write_nr14(&mut self, val: u8) {
        if self.common.set_high_byte(val) {
            self.trigger();
        }
    }

    pub fn step_sample(&mut self) {
        self.common.step_sample();
    }

    pub fn step_sweep(&mut self) {
        if self.on() {
            self.sweep.step(&mut self.common);
        }
    }

    pub fn step_envelope(&mut self) {
        self.common.step_envelope();
    }

    pub fn trigger(&mut self) {
        self.common.trigger();
        self.sweep.trigger(&mut self.common);
    }

    pub fn out(&self) -> u8 {
        self.common.out()
    }

    pub fn on(&self) -> bool {
        self.common.on()
    }

    pub fn step_len(&mut self) {
        self.common.step_length();
    }

    pub fn mut_core(&mut self) -> &mut Ccore<SQUARE_MAX_LENGTH> {
        &mut self.common.core
    }
}

pub struct Ch2 {
    common: Common,
}

impl Ch2 {
    pub fn new() -> Self {
        Self {
            common: Common::new(),
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

    pub fn read_nr24(&self) -> u8 {
        self.common.high_byte()
    }

    pub fn write_nr21(&mut self, val: u8) {
        self.common.set_control_byte(val);
    }

    pub fn write_nr22(&mut self, val: u8) {
        self.common.write_envelope(val);
    }

    pub fn write_nr23(&mut self, val: u8) {
        self.common.set_low_byte(val);
    }

    pub fn write_nr24(&mut self, val: u8) {
        if self.common.set_high_byte(val) {
            self.trigger();
        }
    }

    pub fn step_sample(&mut self) {
        self.common.step_sample();
    }

    pub fn step_envelope(&mut self) {
        self.common.step_envelope();
    }

    pub fn trigger(&mut self) {
        self.common.trigger();
    }

    pub fn out(&self) -> u8 {
        self.common.out()
    }

    pub fn on(&self) -> bool {
        self.common.on()
    }

    pub fn step_len(&mut self) {
        self.common.step_length();
    }

    pub fn mut_core(&mut self) -> &mut Ccore<SQUARE_MAX_LENGTH> {
        &mut self.common.core
    }
}
