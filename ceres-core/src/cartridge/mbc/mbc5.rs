//! The MBC5.

use super::{Mapping, ram_bank, rom_bank};
use crate::cartridge::{ram_size::RAMSize, rom_size::ROMSize};

#[derive(Debug)]
pub(in crate::cartridge) struct Mbc5 {
    /// The low 8 bits of the ROM bank (0x2000-0x2FFF).
    lo: u8,
    /// Bit 8 of the ROM bank (0x3000-0x3FFF).
    hi: u8,
}

impl Default for Mbc5 {
    fn default() -> Self {
        Self { lo: 1, hi: 0 }
    }
}

impl Mbc5 {
    pub(super) const fn write(
        &mut self,
        addr: u16,
        val: u8,
        map: &mut Mapping,
        rom_size: ROMSize,
        ram_size: RAMSize,
    ) {
        match addr {
            0x0000..=0x1FFF => map.write_ram_enable(val),
            0x2000..=0x3FFF => {
                if addr < 0x3000 {
                    self.lo = val;
                } else {
                    self.hi = val;
                }
                // Bank 0 is not corrected to 1.
                let bank = u16::from_le_bytes([self.lo, self.hi]) & rom_size.mask();
                map.rom = (0, rom_bank(bank));
            }
            0x4000..=0x5FFF => map.ram = ram_bank(val & ram_size.mask()),
            _ => (),
        }
    }
}
