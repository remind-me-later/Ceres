// 11 bit frequency data
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct FrequencyData<const MULTIPLIER: u16> {
    frequency_data: u16,
}

// clippy false positive
#[allow(clippy::use_self)]
impl<const MULTIPLIER: u16> From<FrequencyData<MULTIPLIER>> for u16 {
    fn from(frequency_data: FrequencyData<MULTIPLIER>) -> Self {
        frequency_data.frequency_data
    }
}

impl<const PERIOD_MULTIPLIER: u16> FrequencyData<PERIOD_MULTIPLIER> {
    pub fn new() -> Self {
        Self { frequency_data: 0 }
    }

    pub fn reset(&mut self) {
        self.frequency_data = 0;
    }

    pub fn set(&mut self, frequency: u16) {
        self.frequency_data = frequency;
    }

    pub fn get(self) -> u16 {
        self.frequency_data
    }

    pub fn low_byte(self) -> u8 {
        (self.frequency_data & 0xff) as u8
    }

    pub fn set_low_byte(&mut self, value: u8) {
        self.frequency_data = (self.frequency_data & 0x700) | u16::from(value);
    }

    pub fn high_byte(self) -> u8 {
        const MASK: u8 = 0xbf;
        MASK | ((self.frequency_data >> 8) & 7) as u8
    }

    pub fn set_high_byte(&mut self, value: u8) {
        self.frequency_data = (self.frequency_data & 0xff) | (u16::from(value & 0x7) << 8);
    }

    pub fn period(self) -> u16 {
        PERIOD_MULTIPLIER * (2048 - self.frequency_data)
    }
}

impl<const PERIOD_MULTIPLIER: u16> Default for FrequencyData<PERIOD_MULTIPLIER> {
    fn default() -> Self {
        Self::new()
    }
}
