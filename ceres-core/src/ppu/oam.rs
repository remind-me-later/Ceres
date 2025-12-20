use crate::ppu::Ppu;

pub struct Oam {
    bytes: [u8; Self::SIZE as usize],
}

impl Default for Oam {
    fn default() -> Self {
        Self {
            bytes: [0; Self::SIZE as usize],
        }
    }
}

impl Oam {
    pub const SIZE: u8 = 0xA0;

    #[must_use]
    pub const fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    #[must_use]
    pub const fn bytes_mut(&mut self) -> &mut [u8] {
        &mut self.bytes
    }

    pub const fn read(&self, addr: u16) -> u8 {
        self.bytes[(addr & 0xFF) as usize]
    }

    pub const fn write(&mut self, addr: u16, val: u8) {
        self.bytes[(addr & 0xFF) as usize] = val;
    }
}

impl Ppu {
    #[must_use]
    pub const fn oam(&self) -> &Oam {
        &self.oam
    }

    #[must_use]
    pub const fn oam_mut(&mut self) -> &mut Oam {
        &mut self.oam
    }

    #[must_use]
    pub const fn read_oam(&self, addr: u16) -> u8 {
        if self.oam_read_blocked {
            return 0xFF;
        }

        if self.ext_dma_active && self.ext_dma_dst < 0xA0 {
            return self.oam.read((self.ext_dma_dst as u16 & !1) | (addr & 1));
        }

        self.oam.read(addr)
    }

    pub fn write_oam(&mut self, addr: u16, val: u8) {
        if !self.oam_write_blocked {
            self.oam.write(addr, val);
        }
    }

    pub const fn write_oam_by_dma(&mut self, addr: u16, val: u8) {
        self.oam.write(addr, val);
    }
}
