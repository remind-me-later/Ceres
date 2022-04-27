use crate::Model;
use alloc::{boxed::Box, vec};

// TODO: alloc dynamically?
const WRAM_SIZE: usize = 0x2000;
const WRAM_SIZE_CGB: usize = WRAM_SIZE * 4;

pub struct Wram {
    wram: Box<[u8]>,
    cgb_ram_bank: u8,
}

impl Wram {
    pub fn new(model: Model) -> Self {
        let wram = match model {
            Model::Dmg | Model::Mgb => vec![0; WRAM_SIZE].into_boxed_slice(),
            Model::Cgb => vec![0; WRAM_SIZE_CGB].into_boxed_slice(),
        };

        Self {
            wram,
            cgb_ram_bank: 1,
        }
    }

    pub fn read_bank(&self) -> u8 {
        const BANK_MASK: u8 = 0xf8;
        self.cgb_ram_bank | BANK_MASK
    }

    pub fn write_bank(&mut self, val: u8) {
        self.cgb_ram_bank = val & 0x7;

        if self.cgb_ram_bank == 0 {
            self.cgb_ram_bank = 1;
        }
    }

    pub fn read_ram(&self, address: u16) -> u8 {
        self.wram[(address & 0xfff) as usize]
    }

    pub fn write_ram(&mut self, address: u16, val: u8) {
        self.wram[(address & 0xfff) as usize] = val;
    }

    pub fn read_bank_ram(&self, address: u16) -> u8 {
        self.wram[((address & 0xfff) | (self.cgb_ram_bank as u16 * 0x1000)) as usize]
    }

    pub fn write_bank_ram(&mut self, address: u16, val: u8) {
        self.wram[((address & 0xfff) | (self.cgb_ram_bank as u16 * 0x1000)) as usize] = val;
    }
}
