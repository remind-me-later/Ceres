pub const HRAM_SIZE: u8 = 0x7F;

#[derive(Debug)]
pub struct Hram {
    hram: [u8; HRAM_SIZE as usize],
}

impl Default for Hram {
    fn default() -> Self {
        Self {
            hram: [0; HRAM_SIZE as usize],
        }
    }
}

impl Hram {
    pub const fn read(&self, addr: u8) -> u8 {
        self.hram[(addr & 0x7F) as usize]
    }

    pub const fn write(&mut self, addr: u8, val: u8) {
        self.hram[(addr & 0x7F) as usize] = val;
    }

    #[must_use]
    pub const fn hram(&self) -> &[u8; HRAM_SIZE as usize] {
        &self.hram
    }

    pub const fn hram_mut(&mut self) -> &mut [u8; HRAM_SIZE as usize] {
        &mut self.hram
    }
}
