// KEY1 bits
/// The CPU runs in double speed.
const KEY1_SPEED_B: u8 = 0x80;
/// A speed switch is armed: the next STOP performs it.
const KEY1_ARMED_B: u8 = 0x01;

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
        self.key1 = (self.key1 & !KEY1_SPEED_B) | if on { KEY1_SPEED_B } else { 0 };
    }

    pub const fn toggle_double_speed(&mut self) {
        self.key1 ^= KEY1_SPEED_B;
    }

    pub const fn clear_request(&mut self) {
        self.key1 &= KEY1_SPEED_B;
    }

    #[must_use]
    pub const fn is_enabled(&self) -> bool {
        self.key1 & KEY1_SPEED_B != 0
    }

    #[must_use]
    pub const fn is_requested(&self) -> bool {
        self.key1 & KEY1_ARMED_B != 0
    }

    #[must_use]
    pub const fn read(&self) -> u8 {
        // The other bits read 1.
        self.key1 | !(KEY1_SPEED_B | KEY1_ARMED_B)
    }

    pub const fn write(&mut self, val: u8) {
        self.key1 = self.key1 & KEY1_SPEED_B | val & KEY1_ARMED_B;
    }
}
