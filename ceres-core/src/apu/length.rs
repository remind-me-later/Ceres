//! The length timer that stops a channel (NRx1 and bit 6 of NRx4).

#[derive(Clone, Copy)]
pub struct Length {
    /// Ticks left before the channel stops (SameBoy's `pulse_length`).
    pub counter: u16,
    pub enabled: bool,
}

impl Length {
    pub const fn new() -> Self {
        Self {
            counter: 0,
            enabled: false,
        }
    }

    /// Ticks the timer. Returns `true` if it expired.
    pub const fn tick(&mut self) -> bool {
        if self.enabled && self.counter != 0 {
            self.counter -= 1;
            return self.counter == 0;
        }
        false
    }

    /// A trigger with an expired timer reloads it to `max` (64 or 256).
    pub const fn trigger(&mut self, max: u16) {
        if self.counter == 0 {
            self.counter = max;
            self.enabled = false;
        }
    }

    /// Writes NRx4 (`value`), after a trigger. `always_glitch` makes even a
    /// write that leaves the timer disabled glitch (the CGB-B and older do it
    /// for the squares and the wave). Returns `true` if the timer expired.
    pub const fn write(
        &mut self,
        value: u8,
        always_glitch: bool,
        div_divider: u8,
        max: u16,
    ) -> bool {
        let mut expired = false;
        // APU glitch: enabling the length while the DIV divider's LSB is 1
        // ticks the length once.
        if (value & 0x40 != 0 || always_glitch)
            && !self.enabled
            && div_divider & 1 != 0
            && self.counter != 0
        {
            self.counter -= 1;
            if self.counter == 0 {
                if value & 0x80 != 0 {
                    // A trigger reloads it, minus the glitched tick.
                    self.counter = max - 1;
                } else {
                    expired = true;
                }
            }
        }
        self.enabled = value & 0x40 != 0;
        expired
    }
}
