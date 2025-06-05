use crate::ppu::{Mode, Ppu};

#[derive(Debug)]
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

    pub const fn read(&self, addr: u16) -> u8 {
        self.bytes[(addr & 0xFF) as usize]
    }

    pub const fn write(&mut self, addr: u16, val: u8) {
        self.bytes[(addr & 0xFF) as usize] = val;
    }

    #[must_use]
    pub const fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    #[must_use]
    pub const fn bytes_mut(&mut self) -> &mut [u8] {
        &mut self.bytes
    }
}

impl Ppu {
    // TODO: why does read check for enabled DMA transfer and write for active DMA?
    #[must_use]
    pub const fn read_oam(&self, addr: u16, dma_on: bool) -> u8 {
        match self.mode() {
            Mode::HBlank | Mode::VBlank if !dma_on => self.oam.read(addr),
            _ => 0xFF,
        }
    }

    pub const fn write_oam(&mut self, addr: u16, val: u8, dma_active: bool) {
        match self.mode() {
            Mode::HBlank | Mode::VBlank if !dma_active => self.oam.write(addr, val),
            _ => (),
        }
    }

    pub const fn write_oam_by_dma(&mut self, addr: u16, val: u8) {
        // self.oam[(addr & 0xFF) as usize] = val;
        self.oam.write(addr, val);
    }

    #[must_use]
    pub const fn oam(&self) -> &Oam {
        &self.oam
    }

    #[must_use]
    pub const fn oam_mut(&mut self) -> &mut Oam {
        &mut self.oam
    }
}
