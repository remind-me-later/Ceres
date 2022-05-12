#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TriggerEvent {
    Trigger,
    None,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum LengthPeriodHalf {
    First,
    Second,
}

pub struct GenericChannel<const N: u16> {
    is_enabled: bool,
    is_dac_enabled: bool,
    length: u16,
    use_length: bool,
    length_period_half: LengthPeriodHalf,
}

impl<const N: u16> GenericChannel<N> {
    pub fn new() -> Self {
        Self {
            is_enabled: false,
            is_dac_enabled: true,
            length: 0,
            use_length: false,
            length_period_half: LengthPeriodHalf::First, // doesn't matter
        }
    }

    pub fn write_control(&mut self, val: u8) -> TriggerEvent {
        let trigger = val & (1 << 7) != 0;
        self.use_length = val & (1 << 6) != 0;

        if self.use_length && self.length_period_half == LengthPeriodHalf::First {
            self.step_length();
        }

        if trigger {
            if self.is_dac_enabled {
                self.is_enabled = true;
            }

            if self.length == 0 {
                self.length = N;
            }

            TriggerEvent::Trigger
        } else {
            TriggerEvent::None
        }
    }

    pub fn read(&self) -> u8 {
        const MASK: u8 = 0xbf;
        MASK | (u8::from(self.use_length) << 6)
    }

    pub fn write_sound_length(&mut self, val: u8) {
        self.length = N - (u16::from(val) & (N - 1));
    }

    pub fn reset(&mut self) {
        self.is_enabled = false;
        self.is_dac_enabled = true;
        self.length = 0;
        self.use_length = false;
    }

    pub fn set_length_period_half(&mut self, length_period_half: LengthPeriodHalf) {
        self.length_period_half = length_period_half;
    }

    pub fn step_length(&mut self) {
        if self.use_length && self.length > 0 {
            self.length -= 1;

            if self.length == 0 {
                self.is_enabled = false;
            }
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.is_enabled && self.is_dac_enabled
    }

    pub fn set_enabled(&mut self, val: bool) {
        self.is_enabled = val;
    }

    pub fn enable_dac(&mut self) {
        self.is_dac_enabled = true;
    }

    pub fn disable_dac(&mut self) {
        self.is_enabled = false;
        self.is_dac_enabled = false;
    }
}
