pub const WORK_RAM_CGB_SIZE: usize = 0x8000;

pub struct WorkRam {
    wram: [u8; WORK_RAM_CGB_SIZE],
    bank_number: u8,
}

impl WorkRam {
    pub const fn new() -> Self {
        Self {
            wram: [0; WORK_RAM_CGB_SIZE],
            bank_number: 1,
        }
    }

    pub const fn read_bank(&self) -> u8 {
        const BANK_MASK: u8 = 0xf8;
        self.bank_number | BANK_MASK
    }

    pub fn write_bank(&mut self, val: u8) {
        self.bank_number = val & 0x7;

        if self.bank_number == 0 {
            self.bank_number = 1
        }
    }

    pub const fn read_low(&self, address: u16) -> u8 {
        self.wram[(address & 0xfff) as usize]
    }

    pub fn write_low(&mut self, address: u16, val: u8) {
        self.wram[(address & 0xfff) as usize] = val;
    }

    pub const fn read_high(&self, address: u16) -> u8 {
        let address = address & 0xfff;
        let idx = address | self.bank_number as u16 * 0x1000;
        self.wram[idx as usize]
    }

    pub fn write_high(&mut self, address: u16, val: u8) {
        let address = address & 0xfff;
        let idx = address | self.bank_number as u16 * 0x1000;
        self.wram[idx as usize] = val;
    }
}
