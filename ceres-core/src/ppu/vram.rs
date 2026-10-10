use crate::ppu::Ppu;

pub struct Vram {
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
    pub const SIZE_CGB: u16 = Self::SIZE_GB * 2;
    pub const SIZE_GB: u16 = 0x2000;

    #[must_use]
    pub const fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    #[must_use]
    pub const fn bytes_mut(&mut self) -> &mut [u8] {
        &mut self.bytes
    }

    pub const fn read(&self, addr: u16) -> u8 {
        self.vram_at_bank(addr, self.vbk as u8)
    }

    #[must_use]
    pub const fn read_vbk(&self) -> u8 {
        (self.vbk as u8) | 0xFE
    }

    /// Where `addr` is kept in `bank`.
    const fn index(addr: u16, bank: u16) -> usize {
        ((addr & 0x1FFF) + bank * Self::SIZE_GB) as usize
    }

    #[must_use]
    pub const fn vram_at_bank(&self, addr: u16, bank: u8) -> u8 {
        self.bytes[Self::index(addr, bank as u16)]
    }

    pub const fn write(&mut self, addr: u16, val: u8) {
        self.bytes[Self::index(addr, self.vbk as u16)] = val;
    }

    /// HDMA write into the selected bank; `mirror` also writes the other bank.
    pub const fn write_hdma(&mut self, addr: u16, val: u8, mirror: bool) {
        self.bytes[Self::index(addr, self.vbk as u16)] = val;
        if mirror {
            self.bytes[Self::index(addr, !self.vbk as u16)] = val;
        }
    }

    pub const fn write_vbk(&mut self, val: u8) {
        self.vbk = val & 1 != 0;
    }
}

impl Ppu {
    #[must_use]
    pub fn read_vram(&self, addr: u16) -> u8 {
        if self.vram_read_blocked() {
            0xFF
        } else {
            self.vram.read(addr)
        }
    }

    pub const fn vram(&self) -> &Vram {
        &self.vram
    }

    pub const fn vram_mut(&mut self) -> &mut Vram {
        &mut self.vram
    }

    pub fn write_vram(&mut self, addr: u16, val: u8) {
        if !self.vram_write_blocked() {
            self.vram.write(addr, val);
        }
    }
}
