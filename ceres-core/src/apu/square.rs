use {
    super::{
        SweepTrait, envelope::Envelope, length_timer::LengthTimerCalculationResult,
        period_counter::PeriodTriggerResult,
    },
    crate::apu::{LengthTimer, PeriodCounter, PeriodHalf, period_counter::PeriodStepResult},
};

#[derive(Default, Debug)]
pub struct Square<Sweep: SweepTrait> {
    length_timer: LengthTimer<0x3F>,
    period_counter: PeriodCounter<4, Sweep>,
    envelope: Envelope,

    enabled: bool,
    dac_enabled: bool,
    output: u8,

    duty: u8,
    duty_bit: u8,
}

impl<S: SweepTrait> Square<S> {
    pub fn read_nrx0(&self) -> u8 {
        self.period_counter.read_sweep()
    }

    pub const fn read_nrx1(&self) -> u8 {
        0x3F | (self.duty << 6)
    }

    pub const fn read_nrx2(&self) -> u8 {
        self.envelope.read()
    }

    pub const fn read_nrx4(&self) -> u8 {
        0xBF | self.length_timer.read_enabled()
    }

    pub fn write_nrx0(&mut self, val: u8) {
        self.period_counter.write_sweep(val);
    }

    pub const fn write_nrx1(&mut self, val: u8) {
        self.duty = (val >> 6) & 3;
        self.length_timer.write_len(val);
    }

    pub const fn write_nrx2(&mut self, val: u8) {
        if val & 0xF8 == 0 {
            self.enabled = false;
            self.dac_enabled = false;
        } else {
            self.dac_enabled = true;
        }

        self.envelope.write(val);
    }

    pub fn write_nrx3(&mut self, val: u8) {
        self.period_counter.write_low(val);
    }

    pub fn write_nrx4(&mut self, val: u8) {
        self.period_counter.write_high(val);
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

            self.envelope.trigger();

            if matches!(
                self.period_counter.trigger(),
                PeriodTriggerResult::DisableChannel
            ) {
                self.enabled = false;
            }

            if matches!(
                self.length_timer.trigger(),
                LengthTimerCalculationResult::DisableChannel
            ) {
                self.enabled = false;
            }
        }
    }

    pub fn step_sample(&mut self, cycles: i32) {
        // Shape of the duty waveform for a certain duty
        const DUTY_WAV: [u8; 4] = [
            0b0000_0001, // _______- : 12.5%
            0b1000_0001, // -______- : 25%
            0b1000_0111, // -____--- : 50%
            0b0111_1110, // _------_ : 75%
        ];

        if !self.is_enabled() {
            return;
        }

        if matches!(
            self.period_counter.step(cycles),
            PeriodStepResult::AdvanceFrequency
        ) {
            self.duty_bit = (self.duty_bit + 1) & 7;
            self.output = u8::from((DUTY_WAV[self.duty as usize] & (1 << self.duty_bit)) != 0);
        }
    }

    pub fn step_sweep(&mut self) {
        if self.enabled
            && matches!(
                self.period_counter.step_sweep(),
                PeriodTriggerResult::DisableChannel
            )
        {
            self.enabled = false;
        }
    }

    pub const fn step_envelope(&mut self) {
        if self.enabled {
            self.envelope.step();
        }
    }

    pub const fn output(&self) -> u8 {
        self.output * self.envelope.volume()
    }

    pub const fn is_truly_enabled(&self) -> bool {
        self.enabled && self.dac_enabled
    }

    pub const fn step_length_timer(&mut self) {
        if matches!(
            self.length_timer.step(),
            LengthTimerCalculationResult::DisableChannel
        ) {
            self.enabled = false;
        }
    }

    pub const fn set_period_half(&mut self, p_half: PeriodHalf) {
        self.length_timer.set_phalf(p_half);
    }

    pub const fn is_enabled(&self) -> bool {
        self.enabled
    }
}
