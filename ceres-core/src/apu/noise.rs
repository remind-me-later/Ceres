use {
    super::envelope::Envelope,
    crate::apu::{LengthTimer, PHalf},
};

pub(super) struct Noise {
    ltimer: LengthTimer<0x3F>,
    env: Envelope,

    on: bool,
    dac_on: bool,
    timer: i32,
    timer_period: u16,
    // linear feedback shift register
    lfsr: u16,
    wide_step: bool,
    out: u8,
    nr43: u8,
}

impl Default for Noise {
    fn default() -> Self {
        Self {
            on: false,
            dac_on: false,
            ltimer: LengthTimer::default(),
            env: Envelope::default(),
            timer: 1,
            timer_period: 0,
            lfsr: 0x7FFF,
            wide_step: false,
            out: 0,
            nr43: 0,
        }
    }
}

impl Noise {
    pub(super) const fn read_nr42(&self) -> u8 {
        self.env.read()
    }

    pub(super) const fn read_nr43(&self) -> u8 {
        self.nr43
    }

    pub(super) fn read_nr44(&self) -> u8 {
        0xBF | self.ltimer.read_on()
    }

    pub(super) fn write_nr41(&mut self, val: u8) {
        self.ltimer.write_len(val);
    }

    pub(super) fn write_nr42(&mut self, val: u8) {
        if val & 0xF8 == 0 {
            self.on = false;
            self.dac_on = false;
        } else {
            self.dac_on = true;
        }

        self.env.write(val);
    }

    pub(super) fn write_nr43(&mut self, val: u8) {
        self.nr43 = val;
        self.wide_step = val & 8 != 0;

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

        self.timer_period = divisor << clock_shift;
        self.timer = 1;
    }

    pub(super) fn write_nr44(&mut self, val: u8) {
        self.ltimer.write_on(val, &mut self.on);

        // trigger
        if val & 0x80 != 0 {
            if self.dac_on {
                self.on = true;
            }

            self.ltimer.trigger(&mut self.on);

            self.timer = i32::from(self.timer_period);
            self.lfsr = 0x7FFF;
            self.env.trigger();
        }
    }

    pub(super) const fn out(&self) -> u8 {
        self.out * self.env.vol()
    }

    pub(super) fn step_env(&mut self) {
        if !self.on {
            return;
        }

        self.env.step();
    }

    pub(super) fn step_sample(&mut self, cycles: i32) {
        if !self.true_on() {
            return;
        }

        self.timer -= cycles;

        if self.timer < 0 {
            self.timer += i32::from(self.timer_period);

            let xor_bit = (self.lfsr & 1) ^ ((self.lfsr & 2) >> 1);
            self.lfsr >>= 1;
            self.lfsr |= xor_bit << 14;
            if self.wide_step {
                self.lfsr |= xor_bit << 6;
            }

            self.out = u8::from(self.lfsr & 1 == 0);
        }
    }

    pub(super) const fn true_on(&self) -> bool {
        self.on && self.dac_on
    }

    pub(super) fn step_len(&mut self) {
        self.ltimer.step(&mut self.on);
    }

    pub(super) fn set_period_half(&mut self, p_half: PHalf) {
        self.ltimer.set_phalf(p_half);
    }

    pub(super) const fn on(&self) -> bool {
        self.on
    }
}
