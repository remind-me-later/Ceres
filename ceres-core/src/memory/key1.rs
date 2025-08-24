#[derive(Default)]
pub struct Key1 {
    key1: u8,
}

impl Key1 {
    pub fn change_speed(&mut self) {
        debug_assert!(self.is_requested(), "KEY1 not requested");
        self.key1 = !self.key1;
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
