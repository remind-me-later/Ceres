#[derive(Default)]
pub struct Key1 {
    key1: u8,
}

/// The timed phases of a CGB speed switch (SameBoy's `speed_switch_*`).
#[derive(Default)]
pub struct SpeedSwitch {
    /// CPU cycles until the new speed takes effect.
    pub countdown: i32,
    /// CPU cycles during which only the timers keep running.
    pub freeze: i32,
    /// CPU cycles the CPU stays halted for.
    pub halt_countdown: i32,
    /// The halt countdown expired: wake the CPU.
    pub unhalt: bool,
}

impl Key1 {
    pub const fn set_double_speed(&mut self, on: bool) {
        self.key1 = (self.key1 & 0x7F) | ((on as u8) << 7);
    }

    pub const fn toggle_double_speed(&mut self) {
        self.key1 ^= 0x80;
    }

    pub const fn clear_request(&mut self) {
        self.key1 &= 0x80;
    }

    #[must_use]
    pub const fn is_enabled(&self) -> bool {
        self.key1 & 0x80 != 0
    }

    #[must_use]
    pub const fn is_requested(&self) -> bool {
        self.key1 & 1 != 0
    }

    #[must_use]
    pub const fn read(&self) -> u8 {
        self.key1 | 0x7E
    }

    pub const fn write(&mut self, val: u8) {
        self.key1 = self.key1 & 0x80 | val & 1;
    }
}
