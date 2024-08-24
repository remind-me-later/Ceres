use {
    super::{envelope::Envelope, length_timer::LengthTimerCalculationResult},
    crate::apu::{LengthTimer, PeriodHalf},
};

pub(super) struct Noise {
    length_timer: LengthTimer<0x3F>,
    envelope: Envelope,

    enabled: bool,
    dac_enabled: bool,
    timer: i32,
    timer_period: u16,
    // linear feedback shift register
    lfsr: u16,
    wide_step: bool,
    output: u8,
    nr43: u8,
}

impl Default for Noise {
    fn default() -> Self {
        Self {
            enabled: false,
            dac_enabled: false,
            length_timer: LengthTimer::default(),
            envelope: Envelope::default(),
            timer: 1,
            timer_period: 0,
            lfsr: 0x7FFF,
            wide_step: false,
            output: 0,
            nr43: 0,
        }
    }
}

impl Noise {
    pub(super) const fn read_nr42(&self) -> u8 {
        self.envelope.read()
    }

    pub(super) const fn read_nr43(&self) -> u8 {
        self.nr43
    }

    pub(super) fn read_nr44(&self) -> u8 {
        0xBF | self.length_timer.read_enabled()
    }

    pub(super) fn write_nr41(&mut self, val: u8) {
        self.length_timer.write_len(val);
    }

    pub(super) fn write_nr42(&mut self, val: u8) {
        if val & 0xF8 == 0 {
            self.enabled = false;
            self.dac_enabled = false;
        } else {
            self.dac_enabled = true;
        }

        self.envelope.write(val);
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
        if matches!(
            self.length_timer.write_enabled(val),
            LengthTimerCalculationResult::DisableChannel
        ) {
            self.enabled = false;
        }

        // trigger
        if val & 0x80 != 0 {
            if self.dac_enabled {
                self.enabled = true;
            }

            if matches!(
                self.length_timer.trigger(),
                LengthTimerCalculationResult::DisableChannel
            ) {
                self.enabled = false;
            }

            self.timer = i32::from(self.timer_period);
            self.lfsr = 0x7FFF;
            self.envelope.trigger();
        }
    }

    pub(super) const fn output(&self) -> u8 {
        self.output * self.envelope.volume()
    }

    pub(super) fn step_envelope(&mut self) {
        if !self.enabled {
            return;
        }

        self.envelope.step();
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

            self.output = u8::from(self.lfsr & 1 == 0);
        }
    }

    pub(super) const fn true_on(&self) -> bool {
        self.enabled && self.dac_enabled
    }

    pub(super) fn step_length_timer(&mut self) {
        if matches!(
            self.length_timer.step(),
            LengthTimerCalculationResult::DisableChannel
        ) {
            self.enabled = false;
        }
    }

    pub(super) fn set_period_half(&mut self, p_half: PeriodHalf) {
        self.length_timer.set_phalf(p_half);
    }

    pub(super) const fn enabled(&self) -> bool {
        self.enabled
    }
}
