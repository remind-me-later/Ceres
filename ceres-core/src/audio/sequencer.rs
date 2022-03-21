use super::{channels::LengthPeriodHalf, Channels};
use crate::T_CYCLES_PER_SECOND;

#[allow(clippy::cast_possible_truncation)]
const TIMER_RESET_VALUE: u16 = (T_CYCLES_PER_SECOND / 512) as u16;

pub struct Sequencer {
    timer: u16,
    counter: u8,
}

impl Sequencer {
    pub fn new() -> Self {
        Self {
            timer: TIMER_RESET_VALUE,
            counter: 0,
        }
    }

    pub fn tick(&mut self, channels: &mut Channels) {
        self.timer -= 1;
        if self.timer == 0 {
            self.timer = TIMER_RESET_VALUE;
            self.step(channels);
        }

        channels.step_sample();
    }

    fn step(&mut self, channels: &mut Channels) {
        if self.counter % 2 == 0 {
            channels.step_length();
            channels.set_length_period_half(LengthPeriodHalf::First);
        } else {
            channels.set_length_period_half(LengthPeriodHalf::Second);
        }

        match self.counter {
            2 | 6 => channels.step_sweep(),
            7 => channels.step_envelope(),
            _ => (),
        }

        self.counter = (self.counter + 1) % 8;
    }
}
