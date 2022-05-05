const WRAM_SIZE: usize = 0x2000;
const WRAM_SIZE_CGB: usize = WRAM_SIZE * 4;

pub struct Wram {
    wram: [u8; WRAM_SIZE_CGB],
    svbk: u8,
    bank: u8, // between 1 and 7
}

impl Wram {
    pub fn new() -> Self {
        Self {
            wram: [0; WRAM_SIZE_CGB],
            svbk: 0,
            bank: 1,
        }
    }

    pub fn read_svbk(&self) -> u8 {
        self.svbk | 0xf8
    }

    pub fn write_svbk(&mut self, val: u8) {
        let tmp = val & 7;
        self.svbk = tmp;
        self.bank = if tmp == 0 { 1 } else { tmp };
    }

    pub fn read_ram(&self, addr: u16) -> u8 {
        self.wram[(addr & 0xfff) as usize]
    }

    pub fn write_ram(&mut self, addr: u16, val: u8) {
        self.wram[(addr & 0xfff) as usize] = val;
    }

    pub fn read_bank_ram(&self, addr: u16) -> u8 {
        self.wram[(addr & 0xfff | (self.bank as u16 * 0x1000)) as usize]
    }

    pub fn write_bank_ram(&mut self, addr: u16, val: u8) {
        self.wram[(addr & 0xfff | (self.bank as u16 * 0x1000)) as usize] = val;
    }
}
