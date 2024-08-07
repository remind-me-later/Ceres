use super::SweepTrait;

pub(super) struct WaveLength<const PERIOD_MUL: u16, S: SweepTrait> {
    period: i32,
    freq: u16, // 11 bit
    sweep: S,
}

impl<const PERIOD_MUL: u16, S: SweepTrait> Default for WaveLength<PERIOD_MUL, S> {
    fn default() -> Self {
        Self {
            period: Self::calc_period(0),
            freq: 0,
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
        self.freq = (self.freq & 0x700) | u16::from(val);
    }

    pub(super) fn write_high(&mut self, val: u8) {
        self.freq = (u16::from(val) & 7) << 8 | (self.freq & 0xFF);
    }

    pub(super) fn trigger(&mut self, on: &mut bool) {
        self.period = Self::calc_period(self.freq);
        self.sweep.trigger(&mut self.freq, on);
    }

    pub(super) fn step(&mut self, t_cycles: i32) -> bool {
        self.period -= t_cycles;

        if self.period < 0 {
            self.period += Self::calc_period(self.freq);
            return true;
        }

        false
    }

    pub(super) fn step_sweep(&mut self, on: &mut bool) {
        self.sweep.step(&mut self.freq, on);
    }

    const fn calc_period(freq: u16) -> i32 {
        const MAX_FREQ: u16 = 0x800; // 2^11
        (PERIOD_MUL * (MAX_FREQ - freq)) as i32
    }
}
