use super::{RAM_BANK_SIZE, ROM_BANK_SIZE};

pub struct Mbc5 {
    ramg: bool,
    romb0: u8,
    romb1: u8,
    ramb: u8,
}

impl Mbc5 {
    pub fn new() -> Self {
        Self {
            ramg: false,
            romb0: 1,
            romb1: 0,
            ramb: 0,
        }
    }

    fn rom_offsets(&self) -> (usize, usize) {
        let lower_bits = self.romb0 as usize;
        let upper_bits = (self.romb1 as usize) << 8;
        let rom_bank = upper_bits | lower_bits;
        (0x0000, ROM_BANK_SIZE * rom_bank)
    }

    pub fn write_rom(
        &mut self,
        addr: u16,
        value: u8,
        rom_offsets: &mut (usize, usize),
        ram_offset: &mut usize,
    ) {
        match addr {
            0x0000..=0x1fff => self.ramg = value == 0xa,
            0x2000..=0x2fff => {
                self.romb0 = value;
                *rom_offsets = self.rom_offsets();
            }
            0x3000..=0x3fff => {
                self.romb1 = value & 1;
                *rom_offsets = self.rom_offsets();
            }
            0x4000..=0x5fff => {
                self.ramb = value & 0b1111;
                *ram_offset = RAM_BANK_SIZE * self.ramb as usize;
            }

            _ => (),
        }
    }

    pub fn ramg(&self) -> bool {
        self.ramg
    }
}
