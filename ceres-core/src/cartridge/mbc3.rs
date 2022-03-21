use super::{RAM_BANK_SIZE, ROM_BANK_SIZE};

pub struct Mbc3 {
    rom_bank: u8,
    map_en: bool,
    map_select: u8,
    mbc30: bool,
}

impl Mbc3 {
    pub fn new(mbc30: bool) -> Self {
        Self {
            rom_bank: 1,
            map_en: false,
            map_select: 0,
            mbc30,
        }
    }

    pub fn write_rom(
        &mut self,
        addr: u16,
        value: u8,
        rom_offsets: &mut (usize, usize),
        ram_offset: &mut usize,
    ) {
        match addr {
            0x0000..=0x1fff => self.map_en = (value & 0x0f) == 0x0a,
            0x2000..=0x3fff => {
                self.rom_bank = if value == 0 { 1 } else { value };
                *rom_offsets = (0x0000, ROM_BANK_SIZE * self.rom_bank as usize);
            }
            0x4000..=0x5fff => {
                self.map_select = value & 0b1111;
                if self.mbc30 {
                    *ram_offset = RAM_BANK_SIZE * (self.map_select & 0b111) as usize;
                } else {
                    *ram_offset = RAM_BANK_SIZE * (self.map_select & 0b011) as usize;
                }
            }

            _ => (),
        }
    }

    pub fn map_select(&self) -> u8 {
        self.map_select
    }

    pub fn map_en(&self) -> bool {
        self.map_en
    }

    pub fn mbc30(&self) -> bool {
        self.mbc30
    }
}
