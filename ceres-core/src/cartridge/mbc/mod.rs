//! The memory bank controllers: each one owns its registers and sets the
//! banks the CPU sees through the shared `Mapping`.

#![expect(clippy::similar_names, reason = "The ROM and RAM sizes go together")]

mod mbc1;
mod mbc3;
mod mbc3_rtc;
mod mbc5;

use super::{ram_size::RAMSize, rom_size::ROMSize};
use crate::Error;
pub(super) use mbc1::Mbc1;
pub(super) use mbc3::Mbc3;
pub(super) use mbc3_rtc::Mbc3RTC;
pub(super) use mbc5::Mbc5;

/// Writing it in the low nibble to 0x0000-0x1FFF enables the cartridge RAM.
const RAM_ENABLE_KEY: u8 = 0x0A;

/// What the CPU sees of the cartridge: set by the mapper's registers.
#[derive(Debug)]
pub(super) struct Mapping {
    /// The ROM offsets of 0x0000-0x3FFF and 0x4000-0x7FFF.
    pub rom: (u32, u32),
    /// The RAM offset of 0xA000-0xBFFF (masked by the RAM size on access).
    pub ram: u32,
    pub ram_enabled: bool,
}

impl Default for Mapping {
    fn default() -> Self {
        Self {
            rom: (0, rom_bank(1)),
            ram: 0,
            ram_enabled: false,
        }
    }
}

impl Mapping {
    /// A write to 0x0000-0x1FFF, the RAM enable register of every mapper but
    /// the MBC2.
    pub(super) const fn write_ram_enable(&mut self, val: u8) {
        self.ram_enabled = val & 0xF == RAM_ENABLE_KEY;
    }
}

/// The ROM offset of `bank`.
const fn rom_bank(bank: u16) -> u32 {
    ROMSize::BANK_SIZE as u32 * bank as u32
}

/// The RAM offset of `bank`.
const fn ram_bank(bank: u8) -> u32 {
    RAMSize::BANK_SIZE as u32 * bank as u32
}

#[derive(Debug, Default)]
pub(super) enum Mbc {
    #[default]
    Mbc0,
    Mbc1(Mbc1),
    Mbc2,
    Mbc3(Mbc3),
    Mbc5(Mbc5),
}

impl Mbc {
    /// The mapper of a cartridge type, and whether it has a battery.
    pub(super) fn new(
        cart_type: u8,
        rom: &[u8],
        rom_size: ROMSize,
        ram_size: RAMSize,
    ) -> Result<(Self, bool), Error> {
        let mbc1 = || Self::Mbc1(Mbc1::new(rom, rom_size));
        let mbc3 = |rtc| Self::Mbc3(Mbc3::new(rtc, rom_size, ram_size));
        let res = match cart_type {
            0x00 => (Self::Mbc0, false),
            0x01 | 0x02 => (mbc1(), false),
            0x03 => (mbc1(), true),
            0x05 => (Self::Mbc2, false),
            0x06 => (Self::Mbc2, true),
            0x0F | 0x10 => (mbc3(true), true),
            0x11 | 0x12 => (mbc3(false), false),
            0x13 => (mbc3(false), true),
            0x19 | 0x1A => (Self::Mbc5(Mbc5::default()), false),
            0x1B => (Self::Mbc5(Mbc5::default()), true),
            // MBC5 with rumble (0x1C-0x1E) and the other mappers are not
            // supported.
            _ => {
                return Err(Error::UnsupportedMBC {
                    mbc_hex_code: cart_type,
                });
            }
        };

        Ok(res)
    }

    /// A write to the mapper's registers (0x0000-0x7FFF).
    pub(super) fn write(
        &mut self,
        addr: u16,
        val: u8,
        map: &mut Mapping,
        rom_size: ROMSize,
        ram_size: RAMSize,
    ) {
        match *self {
            Self::Mbc0 => (),
            Self::Mbc1(ref mut mbc1) => mbc1.write(addr, val, map, rom_size),
            Self::Mbc2 => mbc2_write(addr, val, map, rom_size),
            Self::Mbc3(ref mut mbc3) => mbc3.write(addr, val, map, rom_size, ram_size),
            Self::Mbc5(ref mut mbc5) => mbc5.write(addr, val, map, rom_size, ram_size),
        }
    }
}

/// The MBC2 has no state beyond the mapping: bit 8 of the address picks
/// the RAM enable or the ROM bank register.
fn mbc2_write(addr: u16, val: u8, map: &mut Mapping, rom_size: ROMSize) {
    if addr > 0x3FFF {
        return;
    }
    if (addr >> 8) & 1 == 0 {
        map.write_ram_enable(val);
    } else {
        // 4 bits, 0 maps bank 1.
        let bank = val & 0xF;
        let bank = if bank == 0 { 1 } else { bank };
        map.rom = (0, rom_bank(u16::from(bank) & rom_size.mask()));
    }
}
