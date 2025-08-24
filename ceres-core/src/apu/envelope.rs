#[derive(Clone, Copy, Default)]
enum EnvelopeDirection {
    #[default]
    Decrease,
    Increase,
}

impl From<u8> for EnvelopeDirection {
    fn from(val: u8) -> Self {
        if val & 8 == 0 {
            Self::Decrease
        } else {
            Self::Increase
        }
    }
}

impl From<EnvelopeDirection> for u8 {
    fn from(val: EnvelopeDirection) -> Self {
        match val {
            EnvelopeDirection::Decrease => 0,
            EnvelopeDirection::Increase => 8,
        }
    }
}

#[derive(Default)]
pub struct Envelope {
    direction: EnvelopeDirection,
    enabled: bool,
    period: u8, // 3 bits
    timer: u8,
    volume: u8,         // 4 bits
    volume_written: u8, // 4 bits
}

impl Envelope {
    pub fn read(&self) -> u8 {
        (self.volume_written << 4) | u8::from(self.direction) | self.period
    }

    pub const fn step(&mut self) {
        use EnvelopeDirection::{Decrease, Increase};

        if !self.enabled {
            return;
        }

        if matches!(self.direction, Increase) && self.volume == 0xF
            || matches!(self.direction, Decrease) && self.volume == 0
        {
            self.enabled = false;
            return;
        }

        self.timer += 1;

        if self.timer > self.period {
            self.timer = 0;

            match self.direction {
                Increase => self.volume += 1,
                Decrease => self.volume -= 1,
            }
        }
    }

    pub const fn trigger(&mut self) {
        self.timer = 0;
        self.volume = self.volume_written;
    }

    pub const fn volume(&self) -> u8 {
        self.volume
    }

    pub fn write(&mut self, val: u8) {
        self.period = val & 7;
        self.enabled = self.period != 0;

        self.timer = 0;
        self.direction = EnvelopeDirection::from(val);
        self.volume_written = val >> 4;
        self.volume = self.volume_written;
    }
}
