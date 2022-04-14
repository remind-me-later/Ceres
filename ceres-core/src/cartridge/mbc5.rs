use super::{RAM_BANK_SIZE, ROM_BANK_SIZE};

pub struct Mbc5 {
    is_ram_enabled: bool,
    rom_bank_low: u8,
    rom_bank_high: u8,
    ram_bank: u8,
    total_rom_banks_mask: usize,
}

impl Mbc5 {
    pub fn new(total_rom_banks_mask: usize) -> Self {
        Self {
            is_ram_enabled: false,
            rom_bank_low: 0,
            rom_bank_high: 1,
            ram_bank: 0,
            total_rom_banks_mask,
        }
    }

    fn rom_offsets(&self) -> (usize, usize) {
        let lower_bits = self.rom_bank_low as usize;
        let upper_bits = (self.rom_bank_high as usize) << 8;
        let rom_bank = (upper_bits | lower_bits) & self.total_rom_banks_mask;
        // let rom_bank = if rom_bank == 0 { 1 } else { rom_bank };
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
            0x0000..=0x1fff => self.is_ram_enabled = value & 0xf == 0xa,
            0x2000..=0x2fff => {
                self.rom_bank_low = value;
                *rom_offsets = self.rom_offsets();
            }
            0x3000..=0x3fff => {
                self.rom_bank_high = value & 1;
                *rom_offsets = self.rom_offsets();
            }
            0x4000..=0x5fff => {
                self.ram_bank = value & 0xf;
                *ram_offset = RAM_BANK_SIZE * self.ram_bank as usize;
            }
            _ => (),
        }
    }

    pub fn is_ram_enabled(&self) -> bool {
        self.is_ram_enabled
    }
}
