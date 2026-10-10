//! The MBC1, with the multicart wiring.

use super::{Mapping, ram_bank, rom_bank};
use crate::cartridge::{HEADER_LOGO, NINTENDO_LOGO, rom_size::ROMSize};

#[derive(Debug)]
pub(in crate::cartridge) struct Mbc1 {
    /// The ROM bank register (0x2000-0x3FFF), stored raw: the 5-bit mask and
    /// the 0 to 1 correction are applied when the offsets are computed.
    lo: u8,
    /// The 2-bit register (0x4000-0x5FFF): the upper ROM bank bits, or the
    /// RAM bank.
    hi: u8,
    /// Mode 1 (0x6000-0x7FFF): `hi` also maps 0x0000-0x3FFF and the RAM.
    bank_mode: bool,
    /// The multicart wiring of some 1 MiB (64-bank) cartridges: 4 quadrants
    /// of 16 banks instead of 8 groups of 8. The header is the one of a
    /// normal cartridge, so it is detected with the heuristic that every bank
    /// carries the Nintendo logo, as real multicarts tend to do.
    multicart: bool,
}

impl Mbc1 {
    pub(super) fn new(rom: &[u8], rom_size: ROMSize) -> Self {
        Self {
            lo: 1,
            hi: 0,
            bank_mode: false,
            multicart: is_multicart(rom, rom_size),
        }
    }

    pub(super) const fn write(&mut self, addr: u16, val: u8, map: &mut Mapping, rom_size: ROMSize) {
        match addr {
            0x0000..=0x1FFF => map.write_ram_enable(val),
            0x2000..=0x3FFF => {
                self.lo = val;
                map.rom = self.rom_offsets(rom_size);
            }
            0x4000..=0x7FFF => {
                if addr < 0x6000 {
                    self.hi = val & 3;
                } else {
                    self.bank_mode = val & 1 != 0;
                }
                map.rom = self.rom_offsets(rom_size);
                map.ram = ram_bank(if self.bank_mode { self.hi } else { 0 });
            }
            _ => (),
        }
    }

    const fn rom_offsets(&self, rom_size: ROMSize) -> (u32, u32) {
        let lo = self.lo & 0x1F;

        if self.multicart {
            // The low 4 bits index the quadrant `hi` selects. Bit 4 picks
            // one of two halves that both map the same 16 banks; the 0 to 1
            // correction only applies when it is clear.
            let mut lo_bank = lo & 0x0F;
            if lo & 0x10 == 0 && lo_bank == 0 {
                lo_bank = 1;
            }
            let quadrant = (self.hi & 3) as u16 * 16;
            // In mode 1, 0x0000-0x3FFF maps the quadrant's first bank.
            let first = if self.bank_mode { quadrant } else { 0 };
            return (rom_bank(first), rom_bank(quadrant + lo_bank as u16));
        }

        let hi = self.hi << 5;
        let first = if self.bank_mode {
            hi as u16 & rom_size.mask()
        } else {
            0
        };

        // The 0 to 1 correction looks at the 5 low bits before the ROM size
        // mask.
        let mut bank = (hi | lo) as u16;
        if bank.trailing_zeros() >= 5 {
            bank += 1;
        }

        (rom_bank(first), rom_bank(bank & rom_size.mask()))
    }
}

fn is_multicart(rom: &[u8], rom_size: ROMSize) -> bool {
    if !matches!(rom_size, ROMSize::Mb1) {
        return false;
    }
    let bank_size = ROMSize::BANK_SIZE as usize;
    let num_banks = rom.len() / bank_size;
    if num_banks < 4 {
        return false;
    }
    (0..num_banks).all(|bank| {
        let start = bank * bank_size + HEADER_LOGO;
        rom.get(start..start + NINTENDO_LOGO.len()) == Some(&NINTENDO_LOGO[..])
    })
}
