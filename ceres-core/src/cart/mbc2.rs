use super::ROM_BANK_SIZE;

pub struct Mbc2 {
    is_ram_enabled: bool,
    rom_bank: u8,
}

impl Mbc2 {
    pub fn new() -> Self {
        Self {
            is_ram_enabled: false,
            rom_bank: 1,
        }
    }

    pub fn write_rom(&mut self, addr: u16, val: u8, rom_offsets: &mut (usize, usize)) {
        if addr <= 0x3fff {
            if (addr >> 8) & 1 == 0 {
                self.is_ram_enabled = (val & 0xf) == 0xa;
            } else {
                let val = val & 0xf;
                self.rom_bank = if val == 0 { 1 } else { val };
                *rom_offsets = (0x0000, ROM_BANK_SIZE * self.rom_bank as usize);
            }
        }
    }

    pub fn is_ram_enabled(&self) -> bool {
        self.is_ram_enabled
    }
}