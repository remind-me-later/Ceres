use super::{sweep::SweepCalculationResult, SweepTrait};

pub(super) enum WaveLengthCalculationResult {
    DisableChannel,
    None,
}

pub(super) struct WaveLength<const PERIOD_MULTIPLIER: u16, Sweep: SweepTrait> {
    timer: i32,
    period: u16, // 11 bit
    sweep: Sweep,
}

impl<const PERIOD_MULTIPLIER: u16, S: SweepTrait> Default for WaveLength<PERIOD_MULTIPLIER, S> {
    fn default() -> Self {
        Self {
            timer: Self::calc_period(0),
            period: 0,
            sweep: S::default(),
        }
    }
}

impl<const PERIOD_MUL: u16, S: SweepTrait> WaveLength<PERIOD_MUL, S> {
    pub(super) fn read_sweep(&self) -> u8 {
        self.sweep.read()
    }

    pub(super) fn write_sweep(&mut self, val: u8) {
        self.sweep.write(val);
    }

    pub(super) fn write_low(&mut self, val: u8) {
        self.period = (self.period & 0x700) | u16::from(val);
    }

    pub(super) fn write_high(&mut self, val: u8) {
        self.period = (u16::from(val) & 7) << 8 | (self.period & 0xFF);
    }

    pub(super) fn trigger(&mut self) -> WaveLengthCalculationResult {
        self.timer = Self::calc_period(self.period);
        match self.sweep.trigger(self.period) {
            SweepCalculationResult::DisableChannel => WaveLengthCalculationResult::DisableChannel,
            SweepCalculationResult::UpdatePeriod { period } => {
                self.period = period;
                WaveLengthCalculationResult::None
            }
            SweepCalculationResult::None => WaveLengthCalculationResult::None,
        }
    }

    pub(super) fn step(&mut self, t_cycles: i32) -> bool {
        self.timer -= t_cycles;

        if self.timer < 0 {
            self.timer += Self::calc_period(self.period);
            return true;
        }

        false
    }

    pub(super) fn step_sweep(&mut self) -> WaveLengthCalculationResult {
        match self.sweep.step() {
            SweepCalculationResult::DisableChannel => WaveLengthCalculationResult::DisableChannel,
            SweepCalculationResult::UpdatePeriod { period } => {
                self.period = period;
                WaveLengthCalculationResult::None
            }
            SweepCalculationResult::None => WaveLengthCalculationResult::None,
        }
    }

    const fn calc_period(freq: u16) -> i32 {
        const MAX_FREQ: u16 = 0x800; // 2^11
        (PERIOD_MUL * (MAX_FREQ - freq)) as i32
    }
}
