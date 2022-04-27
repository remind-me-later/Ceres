const HIGH_RAM_SIZE: usize = 0x80;

pub struct Hram {
    hram: [u8; HIGH_RAM_SIZE],
}

impl Hram {
    pub fn new() -> Self {
        Self {
            hram: [0; HIGH_RAM_SIZE],
        }
    }

    pub fn read(&self, addr: u8) -> u8 {
        self.hram[(addr & 0x7f) as usize]
    }

    pub fn write(&mut self, addr: u8, val: u8) {
        self.hram[(addr & 0x7f) as usize] = val;
    }
}
