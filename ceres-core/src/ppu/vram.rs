use crate::ppu::Ppu;

pub(crate) struct Vram {
    bytes: [u8; Self::SIZE_CGB as usize],
    vbk: bool,
}

impl Default for Vram {
    fn default() -> Self {
        Self {
            vbk: false,
            bytes: [0; Self::SIZE_CGB as usize],
        }
    }
}

impl Vram {
    pub(crate) const SIZE_CGB: u16 = Self::SIZE_GB * 2;
    pub(crate) const SIZE_GB: u16 = 0x2000;

    #[must_use]
    pub(crate) const fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    #[must_use]
    pub(crate) const fn bytes_mut(&mut self) -> &mut [u8] {
        &mut self.bytes
    }

    pub(crate) const fn read(&self, addr: u16) -> u8 {
        self.vram_at_bank(addr, self.vbk as u8)
    }

    #[must_use]
    pub(crate) const fn read_vbk(&self) -> u8 {
        // The unused bits read 1.
        (self.vbk as u8) | 0xFE
    }

    /// Where `addr` is kept in `bank`.
    const fn index(addr: u16, bank: u16) -> usize {
        ((addr & 0x1FFF) + bank * Self::SIZE_GB) as usize
    }

    #[must_use]
    pub(crate) const fn vram_at_bank(&self, addr: u16, bank: u8) -> u8 {
        self.bytes[Self::index(addr, bank as u16)]
    }

    pub(crate) const fn write(&mut self, addr: u16, val: u8) {
        self.bytes[Self::index(addr, self.vbk as u16)] = val;
    }

    /// HDMA write into the selected bank; `mirror` also writes the other bank.
    pub(crate) const fn write_hdma(&mut self, addr: u16, val: u8, mirror: bool) {
        self.bytes[Self::index(addr, self.vbk as u16)] = val;
        if mirror {
            self.bytes[Self::index(addr, !self.vbk as u16)] = val;
        }
    }

    pub(crate) const fn write_vbk(&mut self, val: u8) {
        self.vbk = val & 1 != 0;
    }
}

impl Ppu {
    #[must_use]
    pub(crate) fn read_vram(&self, addr: u16) -> u8 {
        if self.vram_read_blocked() {
            0xFF
        } else {
            self.vram.read(addr)
        }
    }

    pub(crate) const fn vram(&self) -> &Vram {
        &self.vram
    }

    pub(crate) const fn vram_mut(&mut self) -> &mut Vram {
        &mut self.vram
    }

    pub(crate) fn write_vram(&mut self, addr: u16, val: u8) {
        if !self.vram_write_blocked() {
            self.vram.write(addr, val);
        }
    }
}
