use super::Wram;

#[derive(Default)]
pub struct Svbk {
    svbk: u8,
}

impl Svbk {
    #[must_use]
    pub const fn bank_offset(&self) -> u16 {
        // Banks 1 to 7: bank 0 selects bank 1.
        (if self.svbk == 0 { 1 } else { self.svbk } as u16) * Wram::BANK_SIZE
    }

    #[must_use]
    pub const fn read(&self) -> u8 {
        self.svbk | 0xF8
    }

    pub const fn write(&mut self, val: u8) {
        self.svbk = val & 7;
    }
}
