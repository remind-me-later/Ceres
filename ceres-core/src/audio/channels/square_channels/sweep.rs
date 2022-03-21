use super::generic_square_channel::GenericSquareChannel;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Period {
    SweepOff,
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
            Period::SweepOff => 8,
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

impl Default for Period {
    fn default() -> Self {
        Period::SweepOff
    }
}

impl From<u8> for Period {
    fn from(val: u8) -> Self {
        use self::Period::{Div1, Div2, Div3, Div4, Div5, Div6, Div7, SweepOff};
        // bits 6-4
        match (val >> 4) & 0x07 {
            1 => Div1,
            2 => Div2,
            3 => Div3,
            4 => Div4,
            5 => Div5,
            6 => Div6,
            7 => Div7,
            _ => SweepOff,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Direction {
    High,
    Low,
}

impl Default for Direction {
    fn default() -> Self {
        Direction::High
    }
}

impl From<u8> for Direction {
    fn from(value: u8) -> Self {
        use Direction::*;
        match value & (1 << 3) {
            0 => High,
            _ => Low,
        }
    }
}

impl From<Direction> for u8 {
    fn from(dir: Direction) -> Self {
        use Direction::*;
        match dir {
            High => 0,
            Low => 1,
        }
    }
}

pub struct Sweep {
    period: Period,
    direction: Direction,
    shift_number: u8,
    timer: u8,
    shadow_frequency: u16,
    is_enabled: bool,
}

impl Sweep {
    pub fn new() -> Self {
        Self {
            period: Period::default(),
            direction: Direction::default(),
            shift_number: 0,
            timer: 0,
            shadow_frequency: 0,
            is_enabled: false,
        }
    }

    pub fn trigger(&mut self, generic_square_channel: &mut GenericSquareChannel) {
        self.shadow_frequency = generic_square_channel.frequency().get();
        self.timer = self.period.into();
        self.is_enabled = self.period != Period::SweepOff && self.shift_number != 0;

        if self.shift_number != 0 {
            self.sweep_calculation(generic_square_channel);
        }
    }

    pub fn reset(&mut self) {
        self.period = Period::default();
        self.direction = Direction::default();
        self.shift_number = 0;
        self.is_enabled = false;
    }

    fn sweep_calculation(&mut self, generic_square_channel: &mut GenericSquareChannel) {
        let tmp = self.shadow_frequency >> self.shift_number;
        self.shadow_frequency = if self.direction == Direction::High {
            self.shadow_frequency + tmp
        } else {
            self.shadow_frequency - tmp
        };

        if self.shadow_frequency > 0x7ff {
            generic_square_channel.set_enabled(false);
        } else if self.shift_number != 0 {
            generic_square_channel.set_frequency_data(self.shadow_frequency);
        }
    }

    pub fn step(&mut self, generic_square_channel: &mut GenericSquareChannel) {
        self.timer -= 1;
        if self.timer == 0 {
            self.timer = self.period.into();
            if self.is_enabled {
                self.sweep_calculation(generic_square_channel);
            }
        }
    }

    pub fn read(&self) -> u8 {
        const MASK: u8 = 0x80;
        MASK | ((u8::from(self.period)) << 4)
            | (u8::from(self.direction) << 3)
            | (self.shift_number)
    }

    pub fn write(&mut self, value: u8) {
        self.period = value.into();
        self.direction = value.into();
        self.shift_number = value & 7;
    }
}
