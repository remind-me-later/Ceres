// utilities common to all channels
pub struct Ccore<const N: u16> {
    on: bool,
    dac_on: bool,
    use_len: bool,
    len: u16,
    period_half: u8, // 0 or 1
}

impl<const N: u16> Ccore<N> {
    pub fn new() -> Self {
        Self {
            on: false,
            dac_on: true,
            len: 0,
            use_len: false,
            period_half: 0, // doesn't matter
        }
    }

    // returns true if write should trigger an apu event
    #[must_use]
    pub fn write_control(&mut self, val: u8) -> bool {
        let trigger = val & (1 << 7) != 0;
        self.use_len = val & (1 << 6) != 0;

        if self.use_len && self.period_half == 0 {
            self.step_len();
        }

        if trigger {
            if self.dac_on {
                self.on = true;
            }

            if self.len == 0 {
                self.len = N;
            }

            true
        } else {
            false
        }
    }

    pub fn read(&self) -> u8 {
        0xbf | (u8::from(self.use_len) << 6)
    }

    pub fn write_len(&mut self, val: u8) {
        self.len = N - (u16::from(val) & (N - 1));
    }

    pub fn reset(&mut self) {
        self.on = false;
        self.dac_on = true;
        self.len = 0;
        self.use_len = false;
    }

    pub fn set_period_half(&mut self, period_half: u8) {
        self.period_half = period_half & 1;
    }

    pub fn step_len(&mut self) {
        if self.use_len && self.len > 0 {
            self.len -= 1;

            if self.len == 0 {
                self.on = false;
            }
        }
    }

    pub fn on(&self) -> bool {
        self.on && self.dac_on
    }

    pub fn set_on(&mut self, on: bool) {
        self.on = on;
    }

    pub fn enable_dac(&mut self) {
        self.dac_on = true;
    }

    pub fn disable_dac(&mut self) {
        self.on = false;
        self.dac_on = false;
    }
}
