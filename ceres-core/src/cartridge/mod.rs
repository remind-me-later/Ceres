mod mbc;
mod ram_size;
mod rom_size;

use {
    crate::Error,
    alloc::boxed::Box,
    mbc::{Mapping, Mbc, Mbc3RTC},
    ram_size::RAMSize,
    rom_size::ROMSize,
};

#[derive(Debug)]
pub(crate) struct Cartridge {
    has_battery: bool,
    mbc: Mbc,
    map: Mapping,
    ram: Box<[u8]>,
    ram_size: RAMSize,
    rom: Box<[u8]>,
    rom_size: ROMSize,
}

/// No cartridge: 32 KiB of ROM reading $FF, no mapper and no RAM.
impl Default for Cartridge {
    fn default() -> Self {
        let rom_size = ROMSize::default();
        Self {
            has_battery: false,
            mbc: Mbc::default(),
            map: Mapping::default(),
            ram: Box::default(),
            ram_size: RAMSize::default(),
            rom: alloc::vec![0xFF; rom_size.size_bytes() as usize].into_boxed_slice(),
            rom_size,
        }
    }
}

// Cartridge header offsets
const HEADER_LOGO: usize = 0x104;
const HEADER_TITLE: usize = 0x134;
/// The title is 16 bytes long with an old licensee code, 11 with a new one.
const HEADER_TITLE_END_OLD: usize = 0x144;
const HEADER_TITLE_END_NEW: usize = 0x13F;
/// Bit 7 set: the game supports the CGB.
pub(crate) const HEADER_CGB_FLAG: u16 = 0x143;
pub(crate) const HEADER_CGB_B: u8 = 0x80;
const HEADER_CART_TYPE: usize = 0x147;
const HEADER_RAM_SIZE: usize = 0x149;
/// 0x33 means the new licensee code (at 0x144) is used instead.
const HEADER_OLD_LICENSEE: usize = 0x14B;
const NEW_LICENSEE: u8 = 0x33;
const HEADER_VERSION: usize = 0x14C;
const HEADER_CHECKSUM: usize = 0x14D;
/// Big endian, unlike everything else in the header.
const HEADER_GLOBAL_CHECKSUM: usize = 0x14E;
const HEADER_END: usize = 0x150;

/// Nintendo logo bytes at $0104-$0133, used by the boot ROM to verify a
/// legitimate cartridge. Real MBC1 multicarts typically repeat this logo
/// in every ROM bank; standard MBC1 carts only have it in bank 0.
const NINTENDO_LOGO: [u8; 48] = [
    0xCE, 0xED, 0x66, 0x66, 0xCC, 0x0D, 0x00, 0x0B, 0x03, 0x73, 0x00, 0x83, 0x00, 0x0C, 0x00, 0x0D,
    0x00, 0x08, 0x11, 0x1F, 0x88, 0x89, 0x00, 0x0E, 0xDC, 0xCC, 0x6E, 0xE6, 0xDD, 0xDD, 0xD9, 0x99,
    0xBB, 0xBB, 0x67, 0x63, 0x6E, 0x0E, 0xEC, 0xCC, 0xDD, 0xDC, 0x99, 0x9F, 0xBB, 0xB9, 0x33, 0x3E,
];

impl Cartridge {
    #[must_use]
    pub(crate) fn ascii_title(&self) -> &[u8] {
        let range = if self.is_old_licensee_code() {
            HEADER_TITLE..HEADER_TITLE_END_OLD
        } else {
            HEADER_TITLE..HEADER_TITLE_END_NEW
        };

        let title = &self.rom[range];
        let len = title.iter().position(|&b| b == 0).unwrap_or(title.len());
        &title[..len]
    }

    #[must_use]
    pub(crate) const fn global_checksum(&self) -> u16 {
        u16::from_le_bytes([
            self.rom[HEADER_GLOBAL_CHECKSUM + 1],
            self.rom[HEADER_GLOBAL_CHECKSUM],
        ])
    }

    #[must_use]
    pub(crate) const fn has_battery(&self) -> bool {
        self.has_battery
    }

    #[must_use]
    pub(crate) const fn header_checksum(&self) -> u8 {
        self.rom[HEADER_CHECKSUM]
    }

    #[must_use]
    pub(crate) const fn is_old_licensee_code(&self) -> bool {
        self.rom[HEADER_OLD_LICENSEE] != NEW_LICENSEE
    }

    /// The cartridge RAM, with or without a battery (MBC2's built-in 512
    /// half-bytes included).
    #[must_use]
    pub(crate) fn ram(&self) -> &[u8] {
        &self.ram
    }

    #[must_use]
    pub(crate) fn ram_mut(&mut self) -> &mut [u8] {
        &mut self.ram
    }

    #[expect(
        clippy::similar_names,
        reason = "ROM and RAM are common names in this context"
    )]
    pub(crate) fn new(rom: Box<[u8]>) -> Result<Self, Error> {
        if rom.len() < HEADER_END {
            return Err(Error::InvalidRomHeaderSize);
        }

        let rom_size = ROMSize::from_len(rom.len())?;
        let ram_size = RAMSize::new(rom[HEADER_RAM_SIZE])?;

        // Unused space reads as $FF.
        let rom = if rom.len() == rom_size.size_bytes() as usize {
            rom
        } else {
            let mut padded = alloc::vec![0xFF; rom_size.size_bytes() as usize];
            padded[..rom.len()].copy_from_slice(&rom);
            padded.into_boxed_slice()
        };

        let (mbc, has_battery) = Mbc::new(rom[HEADER_CART_TYPE], &rom, rom_size, ram_size)?;

        // The MBC2 has 512 half-bytes of RAM built in, whatever the header says.
        let ram_len = if matches!(mbc, Mbc::Mbc2) {
            0x200
        } else {
            ram_size.size_bytes() as usize
        };

        Ok(Self {
            has_battery,
            mbc,
            map: Mapping::default(),
            ram: alloc::vec![0xFF; ram_len].into_boxed_slice(),
            ram_size,
            rom,
            rom_size,
        })
    }

    /// The RAM index of `addr` (0xA000-0xBFFF) in the mapped bank, wrapped
    /// by the RAM size.
    #[must_use]
    const fn ram_index(&self, addr: u16) -> usize {
        let addr = self.map.ram | (addr & 0x1FFF) as u32;
        (addr & (self.ram_size.size_bytes() - 1)) as usize
    }

    /// The 4-bit RAM of the MBC2, mirrored over 0xA000-0xBFFF.
    const fn mbc2_ram_index(addr: u16) -> usize {
        (addr & 0x1FF) as usize
    }

    #[must_use]
    pub(crate) fn read_ram(&self, addr: u16) -> u8 {
        let enabled = self.map.ram_enabled;
        let banked = || {
            if self.ram_size.has_ram() && enabled {
                self.ram[self.ram_index(addr)]
            } else {
                0xFF
            }
        };

        match self.mbc {
            Mbc::Mbc0 => 0xFF,
            Mbc::Mbc1(_) | Mbc::Mbc5(_) => banked(),
            // The upper 4 bits read 1.
            Mbc::Mbc2 => {
                if enabled {
                    (self.ram[Self::mbc2_ram_index(addr)] & 0xF) | 0xF0
                } else {
                    0xFF
                }
            }
            Mbc::Mbc3(ref mbc3) => mbc3
                .rtc()
                .and_then(|r| r.read(enabled))
                .unwrap_or_else(banked),
        }
    }

    #[must_use]
    pub(crate) const fn read_rom(&self, addr: u16) -> u8 {
        let (lo, hi) = self.map.rom;

        let bank_addr = match addr {
            0x0000..=0x3FFF => lo | (addr & 0x3FFF) as u32,
            0x4000..=0x7FFF => hi | (addr & 0x3FFF) as u32,
            _ => unreachable!(),
        };

        self.rom[bank_addr as usize]
    }

    #[must_use]
    pub(crate) const fn rtc(&self) -> Option<&Mbc3RTC> {
        if let Mbc::Mbc3(ref mbc3) = self.mbc {
            mbc3.rtc()
        } else {
            None
        }
    }

    #[must_use]
    pub(crate) const fn rtc_mut(&mut self) -> Option<&mut Mbc3RTC> {
        if let Mbc::Mbc3(ref mut mbc3) = self.mbc {
            mbc3.rtc_mut()
        } else {
            None
        }
    }

    pub(crate) const fn run_rtc(&mut self, units: u32) {
        if let Some(rtc) = self.rtc_mut() {
            rtc.run(units);
        }
    }

    #[must_use]
    pub(crate) const fn version(&self) -> u8 {
        self.rom[HEADER_VERSION]
    }

    pub(crate) fn write_ram(&mut self, addr: u16, val: u8) {
        let enabled = self.map.ram_enabled;
        // A mapped clock register takes the write.
        if let Some(rtc) = self.rtc_mut()
            && rtc.write(enabled, val).is_some()
        {
            return;
        }

        match self.mbc {
            Mbc::Mbc0 => (),
            Mbc::Mbc1(_) | Mbc::Mbc3(_) | Mbc::Mbc5(_) => {
                if self.ram_size.has_ram() && enabled {
                    let index = self.ram_index(addr);
                    self.ram[index] = val;
                }
            }
            // Only the low 4 bits are stored.
            Mbc::Mbc2 => {
                if enabled {
                    self.ram[Self::mbc2_ram_index(addr)] = val & 0xF;
                }
            }
        }
    }

    /// A write to the mapper's registers (0x0000-0x7FFF).
    pub(crate) fn write_rom(&mut self, addr: u16, val: u8) {
        self.mbc
            .write(addr, val, &mut self.map, self.rom_size, self.ram_size);
    }
}

#[cfg(test)]
mod tests;
