#[derive(Clone, Copy, Default)]
enum EnvelopeDirection {
    #[default]
    Dec = 0,
    Inc = 1,
}

impl EnvelopeDirection {
    const fn from_u8(val: u8) -> Self {
        if val & 8 == 0 {
            Self::Dec
        } else {
            Self::Inc
        }
    }

    const fn to_u8(self) -> u8 {
        (self as u8) << 3
    }
}

#[derive(Default)]
pub(super) struct Envelope {
    on: bool,
    dir: EnvelopeDirection,

    // between 0 and F
    vol: u8,
    vol_init: u8,

    period: u8,
    timer: u8,
}

impl Envelope {
    pub(super) const fn read(&self) -> u8 {
        self.vol_init << 4 | self.dir.to_u8() | self.period
    }

    pub(super) fn write(&mut self, val: u8) {
        self.period = val & 7;
        self.on = self.period != 0;

        self.timer = 0;
        self.dir = EnvelopeDirection::from_u8(val);
        self.vol_init = val >> 4;
        self.vol = self.vol_init;
    }

    pub(super) fn step(&mut self) {
        use EnvelopeDirection::{Dec, Inc};

        if !self.on {
            return;
        }

        if matches!(self.dir, Inc) && self.vol == 0xF || matches!(self.dir, Dec) && self.vol == 0 {
            self.on = false;
            return;
        }

        self.timer += 1;

        if self.timer > self.period {
            self.timer = 0;

            match self.dir {
                Inc => self.vol += 1,
                Dec => self.vol -= 1,
            }
        }
    }

    pub(super) fn trigger(&mut self) {
        self.timer = 0;
        self.vol = self.vol_init;
    }

    pub(super) const fn vol(&self) -> u8 {
        self.vol
    }
}
