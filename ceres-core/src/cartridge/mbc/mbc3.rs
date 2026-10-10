//! The MBC3 and MBC30, with the real-time clock.

use super::{
    Mapping, Mbc3RTC,
    mbc3_rtc::{RTC_REG_FIRST, RTC_REG_LAST},
    ram_bank, rom_bank,
};
use crate::cartridge::{ram_size::RAMSize, rom_size::ROMSize};

#[derive(Debug)]
pub(in crate::cartridge) struct Mbc3 {
    rtc: Option<Mbc3RTC>,
    /// The MBC30 (Pokémon Crystal's) maps up to 4 MiB of ROM and 64 KiB of
    /// RAM. The header does not tell: a cartridge with more than 2 MiB of ROM
    /// or 32 KiB of RAM can only be one.
    is_mbc30: bool,
}

impl Mbc3 {
    pub(super) fn new(rtc: bool, rom_size: ROMSize, ram_size: RAMSize) -> Self {
        Self {
            rtc: rtc.then(Mbc3RTC::default),
            is_mbc30: rom_size.size_bytes() > 0x20_0000 || ram_size.size_bytes() > 0x8000,
        }
    }

    pub(in crate::cartridge) const fn rtc(&self) -> Option<&Mbc3RTC> {
        self.rtc.as_ref()
    }

    pub(in crate::cartridge) const fn rtc_mut(&mut self) -> Option<&mut Mbc3RTC> {
        self.rtc.as_mut()
    }

    pub(super) fn write(
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
                // 7 bits (8 on the MBC30), 0 maps bank 1.
                let mask = if self.is_mbc30 { 0xFF } else { 0x7F };
                #[expect(clippy::cast_possible_truncation, reason = "masked to 8 bits")]
                let bank = val & (rom_size.mask() & mask) as u8;
                map.rom = (0, rom_bank(u16::from(bank.max(1))));
            }
            0x4000..=0x5FFF => {
                if (RTC_REG_FIRST..=RTC_REG_LAST).contains(&val) {
                    // Map a clock register.
                    if let Some(ref mut rtc) = self.rtc {
                        #[expect(
                            clippy::unwrap_used,
                            reason = "val is in RTC_REG_FIRST..=RTC_REG_LAST: it would only panic on 0"
                        )]
                        rtc.map_reg(val).unwrap();
                    }
                } else {
                    // Map a RAM bank: 3 bits (4 on the MBC30).
                    let mask = if self.is_mbc30 { 0xF } else { 0x7 };
                    map.ram = ram_bank(val & mask & ram_size.mask());
                    if let Some(ref mut rtc) = self.rtc {
                        rtc.unmap_reg();
                    }
                }
            }
            0x6000..=0x7FFF => {
                // Any write latches the clock into the registers the CPU reads.
                if let Some(ref mut rtc) = self.rtc {
                    rtc.latch();
                }
            }
            _ => (),
        }
    }
}
