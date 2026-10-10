use super::Wram;

#[derive(Default)]
pub(crate) struct Svbk {
    svbk: u8,
}

impl Svbk {
    #[must_use]
    pub(crate) const fn bank_offset(&self) -> u16 {
        // Banks 1 to 7: bank 0 selects bank 1.
        (if self.svbk == 0 { 1 } else { self.svbk } as u16) * Wram::BANK_SIZE
    }

    #[must_use]
    pub(crate) const fn read(&self) -> u8 {
        // The unused bits read 1.
        self.svbk | 0xF8
    }

    pub(crate) const fn write(&mut self, val: u8) {
        self.svbk = val & 7;
    }
}
