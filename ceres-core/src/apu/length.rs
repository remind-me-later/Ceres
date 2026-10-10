//! The length timer that stops a channel (NRx1 and bit 6 of NRx4).

use super::{NRX4_LENGTH_B, NRX4_TRIGGER_B};

#[derive(Clone, Copy)]
pub(super) struct Length {
    /// Ticks left before the channel stops (SameBoy's `pulse_length`).
    pub counter: u16,
    pub enabled: bool,
    /// The longest length: 64, 256 for the wave channel.
    pub max: u16,
}

impl Length {
    pub(super) const fn new(max: u16) -> Self {
        Self {
            counter: 0,
            enabled: false,
            max,
        }
    }

    /// Writes the length register: the timer counts up from `length` to the
    /// maximum.
    pub(super) const fn load(&mut self, length: u16) {
        self.counter = self.max - length;
    }

    /// Ticks the timer. Returns `true` if it expired.
    pub(super) const fn tick(&mut self) -> bool {
        if self.enabled && self.counter != 0 {
            self.counter -= 1;
            return self.counter == 0;
        }
        false
    }

    /// A trigger with an expired timer reloads it.
    pub(super) const fn trigger(&mut self) {
        if self.counter == 0 {
            self.counter = self.max;
            self.enabled = false;
        }
    }

    /// Writes NRx4 (`value`), after a trigger. `always_glitch` makes even a
    /// write that leaves the timer disabled glitch (the CGB-B and older do it
    /// for the squares and the wave). Returns `true` if the timer expired.
    pub(super) const fn write(&mut self, value: u8, always_glitch: bool, div_divider: u8) -> bool {
        let mut expired = false;
        // APU glitch: enabling the length while the DIV divider's LSB is 1
        // ticks the length once.
        if (value & NRX4_LENGTH_B != 0 || always_glitch)
            && !self.enabled
            && div_divider & 1 != 0
            && self.counter != 0
        {
            self.counter -= 1;
            if self.counter == 0 {
                if value & NRX4_TRIGGER_B != 0 {
                    // A trigger reloads it, minus the glitched tick.
                    self.counter = self.max - 1;
                } else {
                    expired = true;
                }
            }
        }
        self.enabled = value & NRX4_LENGTH_B != 0;
        expired
    }
}
