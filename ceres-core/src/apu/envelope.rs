#[derive(Clone, Copy, Default, Debug)]
enum EnvelopeDirection {
    #[default]
    Decrease = 0,
    Increase = 1,
}

impl EnvelopeDirection {
    const fn from_u8(val: u8) -> Self {
        if val & 8 == 0 {
            Self::Decrease
        } else {
            Self::Increase
        }
    }

    const fn to_u8(self) -> u8 {
        (self as u8) << 3
    }
}

#[derive(Default, Debug)]
pub struct Envelope {
    direction: EnvelopeDirection,
    enabled: bool,
    period: u8, // 3 bits
    timer: u8,
    volume: u8,         // 4 bits
    volume_written: u8, // 4 bits
}

impl Envelope {
    pub const fn read(&self) -> u8 {
        (self.volume_written << 4) | self.direction.to_u8() | self.period
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

    pub const fn write(&mut self, val: u8) {
        self.period = val & 7;
        self.enabled = self.period != 0;

        self.timer = 0;
        self.direction = EnvelopeDirection::from_u8(val);
        self.volume_written = val >> 4;
        self.volume = self.volume_written;
    }
}
