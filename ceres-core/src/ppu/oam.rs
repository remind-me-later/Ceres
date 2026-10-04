use crate::ppu::{LCDC_ON_B, Ppu};

pub struct Oam {
    bytes: [u8; Self::SIZE as usize],
    /// The CGB's memory behind the unusable area (0xFEA0..=0xFEFF).
    extra: [u8; 0x60],
}

impl Default for Oam {
    fn default() -> Self {
        Self {
            bytes: [0; Self::SIZE as usize],
            extra: [0; 0x60],
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

    #[must_use]
    pub const fn extra(&self) -> &[u8; 0x60] {
        &self.extra
    }

    pub const fn extra_mut(&mut self) -> &mut [u8; 0x60] {
        &mut self.extra
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

    /// CPU read of OAM; blocked while the PPU owns it (see the flags set by
    /// the display engine).
    #[must_use]
    pub const fn read_oam(&self, addr: u16) -> u8 {
        if self.lcdc & LCDC_ON_B != 0 && self.oam_read_blocked() {
            0xFF
        } else {
            self.oam.read(addr)
        }
    }

    pub const fn write_oam_by_dma(&mut self, addr: u16, val: u8) {
        // self.oam[(addr & 0xFF) as usize] = val;
        self.oam.write(addr, val);
    }
}
