#[derive(Default, Debug, Clone, Copy)]
pub struct Svbk {
    svbk: u8,
}

impl Svbk {
    #[must_use]
    pub const fn bank_offset(self) -> u16 {
        // Return value between 0x1000 and 0x7000
        (if self.svbk == 0 { 1 } else { self.svbk } as u16) * 0x1000
    }

    #[must_use]
    pub const fn read(self) -> u8 {
        self.svbk | 0xF8
    }

    pub const fn write(&mut self, val: u8) {
        self.svbk = val & 7;
    }
}
