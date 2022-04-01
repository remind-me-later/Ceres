pub const WORK_RAM_CGB_SIZE: usize = 0x8000;

pub struct WorkRam {
    wram: [u8; WORK_RAM_CGB_SIZE],
    bank_register: u8,
    current_bank: u8,
}

impl WorkRam {
    pub const fn new() -> Self {
        Self {
            wram: [0; WORK_RAM_CGB_SIZE],
            bank_register: 1,
            current_bank: 1,
        }
    }

    pub const fn read_bank(&self) -> u8 {
        const BANK_MASK: u8 = 0xf8;
        self.bank_register | BANK_MASK
    }

    pub fn write_bank(&mut self, val: u8) {
        self.bank_register = val & 0x7;

        self.current_bank = if self.bank_register == 0 {
            1
        } else {
            self.bank_register
        };
    }

    pub const fn read_low(&self, address: u16) -> u8 {
        self.wram[(address & 0xfff) as usize]
    }

    pub fn write_low(&mut self, address: u16, val: u8) {
        self.wram[(address & 0xfff) as usize] = val;
    }

    // TODO: is this correct?
    pub const fn read_high(&self, address: u16) -> u8 {
        let address = address & 0xfff;
        let idx = address + self.current_bank as u16 * 0x1000;
        self.wram[idx as usize]
    }

    pub fn write_high(&mut self, address: u16, val: u8) {
        let address = address & 0xfff;
        let idx = address + u16::from(self.current_bank) * 0x1000;
        self.wram[idx as usize] = val;
    }
}
