mod mbc3_rtc;

use super::rom_size::ROMSize;
use crate::Error;
pub use mbc3_rtc::Mbc3RTC;

#[derive(Debug)]
pub enum Mbc {
    Mbc0,
    Mbc1 {
        // Alternative MBC1 wiring allows to address up to 2MB of ROM
        bank_mode: bool,
    },
    Mbc2,
    Mbc3 {
        // Real time clock
        rtc: Option<Mbc3RTC>,
        // Mbc30 is a variant of Mbc3 used in Pokémon Crystal that allows up to 4Mb of ROM and 64Kb of RAM
        is_mbc30: bool,
    },
    Mbc5,
}

impl Mbc {
    pub fn mbc_and_battery(mbc_byte: u8, rom_size: ROMSize) -> Result<(Self, bool), Error> {
        let bank_mode = matches!(
            rom_size,
            ROMSize::Mb1 | ROMSize::Mb2 | ROMSize::Mb4 | ROMSize::Mb8
        );

        let res = match mbc_byte {
            0x00 => (Self::Mbc0, false),
            0x01 | 0x02 => (Self::Mbc1 { bank_mode }, false),
            0x03 => (Self::Mbc1 { bank_mode }, true),
            0x05 => (Self::Mbc2, false),
            0x06 => (Self::Mbc2, true),
            0x0F => (
                Self::Mbc3 {
                    rtc: Some(Mbc3RTC::default()),
                    is_mbc30: false,
                },
                true,
            ),
            0x10 => (
                Self::Mbc3 {
                    rtc: Some(Mbc3RTC::default()),
                    is_mbc30: true,
                },
                true,
            ),
            0x11 => (
                Self::Mbc3 {
                    rtc: None,
                    is_mbc30: false,
                },
                false,
            ),
            0x12 => (
                Self::Mbc3 {
                    rtc: None,
                    is_mbc30: true,
                },
                false,
            ),
            0x13 => (
                Self::Mbc3 {
                    rtc: None,
                    is_mbc30: true,
                },
                true,
            ),
            0x19 | 0x1A => (Self::Mbc5, false),
            // rumble
            // 0x1C | 0x1D => (Mbc5, false),
            // 0x1E => (Mbc5, true),
            0x1B => (Self::Mbc5, true),
            _ => {
                return Err(Error::UnsupportedMBC {
                    mbc_hex_code: mbc_byte,
                });
            }
        };

        Ok(res)
    }
}
