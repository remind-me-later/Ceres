use crate::ppu::{LCDC_ON_B, Ppu};

pub(crate) struct Oam {
    bytes: [u8; Self::SIZE as usize],
    /// The CGB's memory behind the unusable area (0xFEA0..=0xFEFF).
    extra: [u8; Self::EXTRA_SIZE as usize],
}

impl Default for Oam {
    fn default() -> Self {
        Self {
            bytes: [0; Self::SIZE as usize],
            extra: [0; Self::EXTRA_SIZE as usize],
        }
    }
}

impl Oam {
    pub(crate) const SIZE: u8 = 0xA0;
    /// The unusable area after the OAM, up to the I/O registers.
    pub(crate) const EXTRA_SIZE: u8 = 0x60;
    /// The objects, 4 bytes each: Y, X, tile number and attributes.
    pub(crate) const OBJECTS: u8 = 40;
    pub(crate) const ENTRY_SIZE: u16 = 4;
    pub(crate) const ENTRY_Y: u16 = 0;
    pub(crate) const ENTRY_X: u16 = 1;
    pub(crate) const ENTRY_TILE: u16 = 2;
    pub(crate) const ENTRY_ATTRIBUTES: u16 = 3;

    #[must_use]
    pub(crate) const fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    #[must_use]
    pub(crate) const fn bytes_mut(&mut self) -> &mut [u8] {
        &mut self.bytes
    }

    #[must_use]
    pub(crate) const fn extra(&self) -> &[u8; Self::EXTRA_SIZE as usize] {
        &self.extra
    }

    pub(crate) const fn extra_mut(&mut self) -> &mut [u8; Self::EXTRA_SIZE as usize] {
        &mut self.extra
    }

    pub(crate) const fn read(&self, addr: u16) -> u8 {
        self.bytes[(addr & 0xFF) as usize]
    }

    pub(crate) const fn write(&mut self, addr: u16, val: u8) {
        self.bytes[(addr & 0xFF) as usize] = val;
    }
}

impl Ppu {
    #[must_use]
    pub(crate) const fn oam(&self) -> &Oam {
        &self.oam
    }

    #[must_use]
    pub(crate) const fn oam_mut(&mut self) -> &mut Oam {
        &mut self.oam
    }

    /// CPU read of OAM; blocked while the PPU owns it (see the flags set by
    /// the display engine).
    #[must_use]
    pub(crate) const fn read_oam(&self, addr: u16) -> u8 {
        if self.lcdc & LCDC_ON_B != 0 && self.oam_read_blocked() {
            0xFF
        } else {
            self.oam.read(addr)
        }
    }

    pub(crate) const fn write_oam_by_dma(&mut self, addr: u16, val: u8) {
        self.oam.write(addr, val);
    }
}
