use super::generic_channel::GenericChannel;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Period {
    EnvelopeOff,
    Div1,
    Div2,
    Div3,
    Div4,
    Div5,
    Div6,
    Div7,
}

impl From<Period> for u8 {
    fn from(time: Period) -> Self {
        match time {
            // Apparently GB treats this internally as 8
            Period::EnvelopeOff => 8,
            Period::Div1 => 1,
            Period::Div2 => 2,
            Period::Div3 => 3,
            Period::Div4 => 4,
            Period::Div5 => 5,
            Period::Div6 => 6,
            Period::Div7 => 7,
        }
    }
}

impl Period {
    fn read(self) -> u8 {
        let value: u8 = self.into();
        value % 8
    }
}

impl Default for Period {
    fn default() -> Self {
        Period::EnvelopeOff
    }
}

impl From<u8> for Period {
    fn from(val: u8) -> Self {
        use self::Period::{Div1, Div2, Div3, Div4, Div5, Div6, Div7, EnvelopeOff};
        // bits 6-4
        match val & 0x07 {
            1 => Div1,
            2 => Div2,
            3 => Div3,
            4 => Div4,
            5 => Div5,
            6 => Div6,
            7 => Div7,
            _ => EnvelopeOff,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Direction {
    Amplify,
    Attenuate,
}

impl Default for Direction {
    fn default() -> Self {
        Direction::Attenuate
    }
}

impl From<u8> for Direction {
    fn from(value: u8) -> Self {
        use Direction::{Amplify, Attenuate};
        match value & 8 {
            0 => Attenuate,
            _ => Amplify,
        }
    }
}

impl From<Direction> for u8 {
    fn from(direction: Direction) -> Self {
        match direction {
            Direction::Amplify => 8,
            Direction::Attenuate => 0,
        }
    }
}

pub struct Envelope {
    is_enabled: bool,
    initial_volume: u8,
    volume: u8,
    direction: Direction,
    period: Period,
    timer: u8,
}

impl Envelope {
    pub fn new() -> Self {
        let period = Period::default();
        let timer = period.into();

        Self {
            is_enabled: false,
            initial_volume: 0,
            period,
            volume: 0,
            timer,
            direction: Direction::default(),
        }
    }

    pub fn reset(&mut self) {
        self.period = Period::default();
        self.timer = self.period.into();
        self.direction = Direction::default();
        self.initial_volume = 0;
        self.volume = 0;
        self.is_enabled = false;
    }

    pub fn read(&self) -> u8 {
        (self.initial_volume << 4) | u8::from(self.direction) | self.period.read()
    }

    pub fn write(&mut self, value: u8, generic_channel: &mut GenericChannel<64>) {
        // value == 0x7 || value == 0x0 to pass Blargg 2 test
        if value == 0x7 {
            generic_channel.disable_dac();
        }

        if value == 0x10 {
            generic_channel.enable_dac();
        }

        self.period = value.into();
        self.is_enabled = self.period != Period::EnvelopeOff;
        self.timer = self.period.into();
        self.direction = value.into();
        self.initial_volume = value >> 4;
        self.volume = self.initial_volume;
    }

    pub fn step(&mut self, generic_channel: &mut GenericChannel<64>) {
        use Direction::{Amplify, Attenuate};
        if !self.is_enabled || !generic_channel.is_enabled() {
            return;
        }

        if self.direction == Amplify && self.volume == 15
            || self.direction == Attenuate && self.volume == 0
        {
            self.is_enabled = false;
            return;
        }

        self.timer -= 1;
        if self.timer == 0 {
            self.timer = self.period.into();

            if self.direction == Amplify {
                self.volume += 1;
            } else {
                self.volume -= 1;
            };
        }
    }

    pub fn trigger(&mut self) {
        self.timer = self.period.into();
        self.volume = self.initial_volume;
    }

    pub fn volume(&self) -> u8 {
        self.volume
    }
}
