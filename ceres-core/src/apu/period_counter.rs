use super::{sweep::SweepCalculationResult, SweepTrait};

pub(super) enum PeriodCalculationResult {
    DisableChannel,
    None,
}

pub(super) struct PeriodCounter<const PERIOD_MULTIPLIER: u16, Sweep: SweepTrait> {
    timer: i32,
    period: u16, // 11 bit
    sweep: Sweep,
}

impl<const PERIOD_MULTIPLIER: u16, Sweep: SweepTrait> Default
    for PeriodCounter<PERIOD_MULTIPLIER, Sweep>
{
    fn default() -> Self {
        let period = 0;

        Self {
            timer: Self::timer_from_period(period),
            period,
            sweep: Sweep::default(),
        }
    }
}

impl<const PERIOD_MUL: u16, S: SweepTrait> PeriodCounter<PERIOD_MUL, S> {
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

    pub(super) fn trigger(&mut self) -> PeriodCalculationResult {
        self.timer = Self::timer_from_period(self.period);
        match self.sweep.trigger(self.period) {
            SweepCalculationResult::DisableChannel => PeriodCalculationResult::DisableChannel,
            SweepCalculationResult::UpdatePeriod { period } => {
                self.period = period;
                PeriodCalculationResult::None
            }
            SweepCalculationResult::None => PeriodCalculationResult::None,
        }
    }

    pub(super) fn step(&mut self, t_cycles: i32) -> bool {
        self.timer -= t_cycles;

        if self.timer <= 0 {
            self.timer += Self::timer_from_period(self.period);
            return true;
        }

        false
    }

    pub(super) fn step_sweep(&mut self) -> PeriodCalculationResult {
        match self.sweep.step() {
            SweepCalculationResult::DisableChannel => PeriodCalculationResult::DisableChannel,
            SweepCalculationResult::UpdatePeriod { period } => {
                self.period = period;
                PeriodCalculationResult::None
            }
            SweepCalculationResult::None => PeriodCalculationResult::None,
        }
    }

    const fn timer_from_period(period: u16) -> i32 {
        const MAX_PERIOD: u16 = 0x800; // 2^11
        (PERIOD_MUL * (MAX_PERIOD - period)) as i32
    }
}
