const VRAM_BANK_SIZE: usize = 0x2000;

#[derive(Clone, Copy)]
pub enum VramBankRegister {
    Bank0,
    Bank1,
}

impl VramBankRegister {
    const fn multiplier(self) -> u16 {
        use VramBankRegister::*;
        match self {
            Bank0 => 0,
            Bank1 => 1,
        }
    }
}

impl From<bool> for VramBankRegister {
    fn from(val: bool) -> Self {
        use VramBankRegister::*;
        match val {
            true => Bank1,
            false => Bank0,
        }
    }
}

impl From<u8> for VramBankRegister {
    fn from(val: u8) -> Self {
        use VramBankRegister::*;
        match val & 1 {
            0 => Bank0,
            _ => Bank1,
        }
    }
}

impl From<VramBankRegister> for u8 {
    fn from(register: VramBankRegister) -> Self {
        use VramBankRegister::*;
        match register {
            Bank0 => 0xfe,
            Bank1 => 0xff,
        }
    }
}

pub struct VramBank {
    vram: [u8; VRAM_BANK_SIZE * 2],
    bank: VramBankRegister,
}

impl core::ops::Index<usize> for VramBank {
    type Output = u8;

    fn index(&self, idx: usize) -> &Self::Output {
        &self.vram[idx]
    }
}

impl core::ops::IndexMut<usize> for VramBank {
    fn index_mut(&mut self, idx: usize) -> &mut Self::Output {
        &mut self.vram[idx]
    }
}

impl VramBank {
    pub const fn new() -> Self {
        Self {
            vram: [0; VRAM_BANK_SIZE * 2],
            bank: VramBankRegister::Bank0,
        }
    }

    pub const fn bank(&self) -> u8 {
        const BANK_MASK: u8 = 0xfe;
        self.bank as u8 | BANK_MASK
    }

    pub fn set_bank(&mut self, val: u8) {
        self.bank = val.into()
    }

    pub const fn read(&self, address: u16) -> u8 {
        let address = address & 0x1fff;
        let idx = address + self.bank.multiplier() * VRAM_BANK_SIZE as u16;
        self.vram[(idx as usize)]
    }

    pub fn write(&mut self, address: u16, val: u8) {
        let address = address & 0x1fff;
        let idx = address + self.bank.multiplier() * VRAM_BANK_SIZE as u16;
        self.vram[(idx as usize)] = val
    }

    pub const fn get_bank(&self, address: u16, bank: VramBankRegister) -> u8 {
        let address = address & 0x1fff;
        let idx = address + bank.multiplier() * VRAM_BANK_SIZE as u16;
        self.vram[(idx as usize)]
    }
}
