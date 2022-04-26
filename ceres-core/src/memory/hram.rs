const HIGH_RAM_SIZE: usize = 0x80;

pub struct Hram {
    high_ram: [u8; HIGH_RAM_SIZE],
}

impl Hram {
    pub const fn new() -> Self {
        Self {
            high_ram: [0; HIGH_RAM_SIZE],
        }
    }

    pub const fn read(&self, address: u8) -> u8 {
        self.high_ram[(address & 0x7f) as usize]
    }

    pub fn write(&mut self, address: u8, val: u8) {
        self.high_ram[(address & 0x7f) as usize] = val;
    }
}
