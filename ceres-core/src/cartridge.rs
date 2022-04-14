extern crate alloc;

mod header_info;
mod mbc1;
mod mbc2;
mod mbc3;
mod mbc5;

pub use self::header_info::{CgbFlag, HeaderInfo};
use self::{mbc1::Mbc1, mbc2::Mbc2, mbc3::Mbc3, mbc5::Mbc5};
use crate::Error;
use alloc::boxed::Box;
use alloc::vec;

pub const ROM_BANK_SIZE: usize = 0x4000;
pub const RAM_BANK_SIZE: usize = 0x2000;

pub enum Mbc {
    None,
    One(Mbc1),
    Two(Mbc2),
    Three(Mbc3),
    Five { mbc: Mbc5, has_rumble: bool },
}

impl core::fmt::Display for Mbc {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Mbc::None => write!(f, "no MBC"),
            Mbc::One(_) => write!(f, "MBC 1"),
            Mbc::Two(_) => write!(f, "MBC 2"),
            Mbc::Three(_) => write!(f, "MBC 3"),
            Mbc::Five { .. } => write!(f, "MBC 5"),
        }
    }
}

pub struct Cartridge<R: RumbleCallbacks> {
    mbc: Mbc,
    rom: Box<[u8]>,
    header_info: HeaderInfo,
    has_battery: bool,
    ram: Box<[u8]>,
    rom_offsets: (usize, usize),
    ram_offset: usize,
    rumble_callbacks: R,
}

impl<R: RumbleCallbacks> Cartridge<R> {
    pub fn new(
        rom: Box<[u8]>,
        ram: Option<Box<[u8]>>,
        rumble_callbacks: R,
    ) -> Result<Cartridge<R>, Error> {
        let header_info = HeaderInfo::new(&rom)?;
        let mbc30 = header_info.ram_size().number_of_banks() >= 8;
        let rom_bit_mask = header_info.rom_size().banks_bit_mask();

        let (mbc, has_battery) = match rom[0x147] {
            0x00 => (Mbc::None, false),
            0x01 | 0x02 => (Mbc::One(Mbc1::new()), false),
            0x03 => (Mbc::One(Mbc1::new()), true),
            0x05 => (Mbc::Two(Mbc2::new()), false),
            0x06 => (Mbc::Two(Mbc2::new()), true),
            0x0f | 0x10 | 0x13 => (Mbc::Three(Mbc3::new(mbc30)), true),
            0x11 | 0x12 => (Mbc::Three(Mbc3::new(mbc30)), false),
            0x19 | 0x1a => (
                Mbc::Five {
                    mbc: Mbc5::new(rom_bit_mask),
                    has_rumble: false,
                },
                false,
            ),
            0x1b => (
                Mbc::Five {
                    mbc: Mbc5::new(rom_bit_mask),
                    has_rumble: false,
                },
                true,
            ),
            0x1c | 0x1d => (
                Mbc::Five {
                    mbc: Mbc5::new(rom_bit_mask),
                    has_rumble: true,
                },
                false,
            ),
            0x1e => (
                Mbc::Five {
                    mbc: Mbc5::new(rom_bit_mask),
                    has_rumble: true,
                },
                true,
            ),
            mbc_byte => return Err(Error::InvalidMBC { mbc_byte }),
        };

        let ram = if let Some(ram) = ram {
            ram
        } else {
            let cap = header_info.ram_size().total_size_in_bytes();
            vec![0; cap].into_boxed_slice()
        };

        let rom_offsets = (0x0000, 0x4000);
        let ram_offset = 0;

        Ok(Self {
            rom,
            mbc,
            has_battery,
            header_info,
            ram,
            rom_offsets,
            ram_offset,
            rumble_callbacks,
        })
    }

    pub fn has_battery(&self) -> bool {
        self.has_battery
    }

    pub fn header_info(&self) -> &HeaderInfo {
        &self.header_info
    }

    pub fn read_rom(&self, address: u16) -> u8 {
        let bank_address = match address {
            0x0000..=0x3fff => {
                let (rom_lower, _) = self.rom_offsets;
                rom_lower as usize | (address as usize & 0x3fff)
            }
            0x4000..=0x7fff => {
                let (_, rom_upper) = self.rom_offsets;
                rom_upper as usize | (address as usize & 0x3fff)
            }
            _ => 0,
        };

        self.rom[bank_address as usize]
    }

    pub fn ram_address(&self, address: u16) -> usize {
        self.ram_offset | (address as usize & 0x1fff)
    }

    fn mbc_read_ram(&self, ram_enabled: bool, address: u16) -> u8 {
        if !self.ram.is_empty() && ram_enabled {
            let addr = self.ram_address(address);
            self.ram[addr]
        } else {
            0xff
        }
    }

    pub fn read_ram(&self, address: u16) -> u8 {
        match self.mbc {
            Mbc::None => 0xff,
            Mbc::One(ref mbc1) => self.mbc_read_ram(mbc1.ramg(), address),
            Mbc::Two(ref mbc2) => (self.mbc_read_ram(mbc2.is_ram_enabled(), address) & 0xf) | 0xf0,
            Mbc::Three(ref mbc3) => {
                let map_select = mbc3.map_select();
                let map_en = mbc3.map_en();
                let mbc30 = mbc3.mbc30();

                match map_select {
                    0x00..=0x03 => self.mbc_read_ram(map_en, address),
                    0x04..=0x07 => self.mbc_read_ram(map_en && mbc30, address),
                    _ => 0xff,
                }
            }
            Mbc::Five { ref mbc, .. } => self.mbc_read_ram(mbc.is_ram_enabled(), address),
        }
    }

    pub fn write_rom(&mut self, address: u16, value: u8) {
        match self.mbc {
            Mbc::None => (),
            Mbc::One(ref mut mbc1) => {
                mbc1.write_rom(address, value, &mut self.rom_offsets, &mut self.ram_offset)
            }
            Mbc::Two(ref mut mbc2) => mbc2.write_rom(address, value, &mut self.rom_offsets),
            Mbc::Three(ref mut mbc3) => {
                mbc3.write_rom(address, value, &mut self.rom_offsets, &mut self.ram_offset)
            }
            Mbc::Five { ref mut mbc, .. } => {
                mbc.write_rom(address, value, &mut self.rom_offsets, &mut self.ram_offset)
            }
        }
    }

    pub fn mbc_write_ram(&mut self, ram_enabled: bool, address: u16, value: u8) {
        if !self.ram.is_empty() && ram_enabled {
            let addr = self.ram_address(address);
            self.ram[addr] = value;
        }
    }

    pub fn write_ram(&mut self, address: u16, value: u8) {
        match self.mbc {
            Mbc::None => (),
            Mbc::One(ref mbc1) => {
                let is_ram_enabled = mbc1.ramg();
                self.mbc_write_ram(is_ram_enabled, address, value)
            }
            Mbc::Two(ref mbc2) => {
                let is_ram_enabled = mbc2.is_ram_enabled();
                self.mbc_write_ram(is_ram_enabled, address, value)
            }
            Mbc::Three(ref mbc3) => {
                let map_en = mbc3.map_en();
                let map_select = mbc3.map_select();
                let mbc30 = mbc3.mbc30();

                match map_select {
                    0x00..=0x03 => self.mbc_write_ram(map_en, address, value),
                    0x04..=0x07 => self.mbc_write_ram(map_en && mbc30, address, value),
                    _ => (),
                }
            }
            Mbc::Five {
                ref mbc,
                has_rumble,
            } => {
                let is_ram_enabled = mbc.is_ram_enabled();
                self.mbc_write_ram(is_ram_enabled, address, value);
                if has_rumble {
                    if value & 0x8 != 0 {
                        self.rumble_callbacks.start_rumble();
                    } else {
                        self.rumble_callbacks.stop_rumble();
                    }
                }
            }
        }
    }

    pub fn ram(&self) -> &[u8] {
        &self.ram
    }
}

impl<R: RumbleCallbacks> core::fmt::Display for Cartridge<R> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let has_rumble = match self.mbc {
            Mbc::Five { has_rumble, .. } => has_rumble,
            _ => false,
        };

        write!(
            f,
            "{}\n{}\nBattery: {}, Rumble: {}\n",
            self.mbc,
            self.header_info(),
            self.has_battery,
            has_rumble
        )
    }
}

pub trait RumbleCallbacks {
    fn start_rumble(&mut self);
    fn stop_rumble(&mut self);
}
