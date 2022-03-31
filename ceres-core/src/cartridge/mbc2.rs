use super::ROM_BANK_SIZE;

pub struct Mbc2 {
    ramg: bool,
    rom_bank: u8,
}

impl Mbc2 {
    pub const fn new() -> Self {
        Self {
            ramg: false,
            rom_bank: 1,
        }
    }

    pub fn write_rom(&mut self, addr: u16, value: u8, rom_offsets: &mut (usize, usize)) {
        if let 0x0000..=0x3fff = addr {
            if addr >> 8 == 0 {
                self.ramg = (value & 0xf) == 0xa;
            } else {
                let value = value & 0xf;
                self.rom_bank = if value == 0 { 1 } else { value };
                *rom_offsets = (0x0000, ROM_BANK_SIZE * self.rom_bank as usize);
            }
        }
    }

    pub fn ramg(&self) -> bool {
        self.ramg
    }
}
