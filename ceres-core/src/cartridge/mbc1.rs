use super::{RAM_BANK_SIZE, ROM_BANK_SIZE};

pub struct Mbc1 {
    ramg: bool,
    bank1: u8,
    bank2: u8,
    mode: bool,
    is_multicart: bool,
}

impl Mbc1 {
    pub fn new() -> Self {
        Self {
            ramg: false,
            bank1: 1,
            bank2: 0,
            mode: false,
            is_multicart: false,
        }
    }

    fn rom_offsets(&self, multicart: bool) -> (usize, usize) {
        let upper_bits = if multicart {
            self.bank2 << 4
        } else {
            self.bank2 << 5
        };
        let lower_bits = if multicart {
            self.bank1 & 0xf
        } else {
            self.bank1
        };

        let lower_bank = if self.mode { upper_bits as usize } else { 0 };
        let upper_bank = (upper_bits | lower_bits) as usize;
        (ROM_BANK_SIZE * lower_bank, ROM_BANK_SIZE * upper_bank)
    }

    fn ram_offset(&self) -> usize {
        let bank = if self.mode { self.bank2 as usize } else { 0 };
        RAM_BANK_SIZE * bank
    }

    pub fn write_rom(
        &mut self,
        addr: u16,
        value: u8,
        rom_offsets: &mut (usize, usize),
        ram_offset: &mut usize,
    ) {
        match addr {
            0x0000..=0x1fff => self.ramg = (value & 0xf) == 0xa,
            0x2000..=0x3fff => {
                self.bank1 = if value & 0x1f == 0 { 1 } else { value & 0x1f };
                *rom_offsets = self.rom_offsets(self.is_multicart);
            }
            0x4000..=0x5fff => {
                self.bank2 = value & 3;
                *rom_offsets = self.rom_offsets(self.is_multicart);
                *ram_offset = self.ram_offset();
            }
            0x6000..=0x7fff => {
                self.mode = (value & 1) == 1;
                *rom_offsets = self.rom_offsets(self.is_multicart);
                *ram_offset = self.ram_offset();
            }
            _ => (),
        }
    }

    pub fn ramg(&self) -> bool {
        self.ramg
    }
}
