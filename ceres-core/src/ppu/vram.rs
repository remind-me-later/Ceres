use crate::ppu::{Mode, Ppu};

#[derive(Debug)]
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

    #[must_use]
    pub const fn vram_at_bank(&self, addr: u16, bank: u8) -> u8 {
        let bank = bank as u16 * Self::SIZE_GB;
        let i = (addr & 0x1FFF) + bank;
        self.bytes[i as usize]
    }

    pub fn write(&mut self, addr: u16, val: u8) {
        let bank = u16::from(self.vbk) * Self::SIZE_GB;
        let i = (addr & 0x1FFF) + bank;
        self.bytes[i as usize] = val;
    }

    pub const fn write_vbk(&mut self, val: u8) {
        self.vbk = val & 1 != 0;
    }
}

impl Ppu {
    #[must_use]
    pub const fn read_vram(&self, addr: u16) -> u8 {
        if matches!(self.mode(), Mode::Drawing) {
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
        if !matches!(self.mode(), Mode::Drawing) {
            self.vram.write(addr, val);
        }
    }
}
