pub(crate) struct Hram {
    hram: [u8; Self::SIZE as usize],
}

impl Default for Hram {
    fn default() -> Self {
        Self {
            hram: [0; Self::SIZE as usize],
        }
    }
}

impl Hram {
    pub(crate) const SIZE: u8 = 0x7F;

    #[must_use]
    pub(crate) const fn hram(&self) -> &[u8; Self::SIZE as usize] {
        &self.hram
    }

    pub(crate) const fn hram_mut(&mut self) -> &mut [u8; Self::SIZE as usize] {
        &mut self.hram
    }

    pub(crate) const fn read(&self, addr: u8) -> u8 {
        self.hram[(addr & Self::SIZE) as usize]
    }

    pub(crate) const fn write(&mut self, addr: u8, val: u8) {
        self.hram[(addr & Self::SIZE) as usize] = val;
    }
}
