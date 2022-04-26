// TODO: alloc dynamically?
const WRAM_SIZE: usize = 0x2000;
const WRAM_SIZE_CGB: usize = WRAM_SIZE * 4;

pub struct Wram {
    wram: [u8; WRAM_SIZE_CGB],
    cgb_ram_bank: u8,
}

impl Wram {
    pub const fn new() -> Self {
        Self {
            wram: [0; WRAM_SIZE_CGB],
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

    pub const fn read_ram(&self, address: u16) -> u8 {
        self.wram[(address & 0xfff) as usize]
    }

    pub fn write_ram(&mut self, address: u16, val: u8) {
        self.wram[(address & 0xfff) as usize] = val;
    }

    pub const fn read_bank_ram(&self, address: u16) -> u8 {
        self.wram[((address & 0xfff) | (self.cgb_ram_bank as u16 * 0x1000)) as usize]
    }

    pub fn write_bank_ram(&mut self, address: u16, val: u8) {
        self.wram[((address & 0xfff) | (self.cgb_ram_bank as u16 * 0x1000)) as usize] = val;
    }
}
