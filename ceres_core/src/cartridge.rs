use {
    crate::Error,
    alloc::{boxed::Box, vec},
    core::str,
    Mbc::{Mbc1, Mbc2, Mbc3, Mbc5, MbcNone},
};

const TITLE_START: usize = 0x134;
const OLD_TITLE_END: usize = 0x143;
const NEW_TITLE_END: usize = 0x13F;

const ROM_BANK_SIZE: usize = 0x4000;
const RAM_BANK_SIZE: usize = 0x2000;

#[allow(clippy::enum_variant_names)]
enum Mbc {
    MbcNone,
    Mbc1,
    Mbc2,
    Mbc3,
    Mbc5,
}

pub struct Cartridge {
    header_info: Header,
    mbc: Mbc,

    rom: Box<[u8]>,
    rom_bank_lo: u8,
    rom_bank_hi: u8,
    rom_offsets: (usize, usize),
    // bit mask of rom bank, anded with the rom bank selected gets
    // the actual rom bank depending on the ROM size
    rom_bank_mask: usize,

    ram: Box<[u8]>,
    ram_enabled: bool,
    ram_bank: u8,
    ram_offset: usize,

    mbc1_bank_mode: bool,
    has_battery: bool,

    mbc30: bool,
    mbc1_multicart: bool,
}

impl Cartridge {
    /// # Errors
    ///
    /// Will return `Err` if the ROM header contains some
    /// illegal value
    pub fn new(rom: Box<[u8]>, ram: Option<Box<[u8]>>) -> Result<Cartridge, Error> {
        let header_info = Header::new(&rom)?;
        let mbc30 = header_info.ram_size().num_banks() >= 8;
        let rom_bank_mask = header_info.rom_size().bank_bit_mask();

        let (mbc, has_battery) = match rom[0x147] {
            0x00 => (MbcNone, false),
            0x01 | 0x02 => (Mbc1, false),
            0x03 => (Mbc1, true),
            0x05 => (Mbc2, false),
            0x06 => (Mbc2, true),
            0x0F | 0x10 | 0x13 => (Mbc3, true),
            0x11 | 0x12 => (Mbc3, false),
            0x19 | 0x1A | 0x1C | 0x1D => (Mbc5, false),
            0x1B | 0x1E => (Mbc5, true),
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
            mbc,
            rom,
            header_info,
            has_battery,
            ram,
            rom_offsets,
            ram_offset,
            ram_enabled: false,
            mbc30,
            rom_bank_lo: 1,
            rom_bank_hi: 0,
            mbc1_bank_mode: false,
            mbc1_multicart: false,
            rom_bank_mask,
            ram_bank: 0,
        })
    }

    #[must_use]
    pub fn has_battery(&self) -> bool {
        self.has_battery
    }

    #[must_use]
    pub fn header_info(&self) -> &Header {
        &self.header_info
    }

    #[must_use]
    pub fn read_rom(&self, addr: u16) -> u8 {
        let bank_addr = match addr {
            0x0000..=0x3FFF => {
                let (rom_lower, _) = self.rom_offsets;
                rom_lower as usize | (addr as usize & 0x3FFF)
            }
            0x4000..=0x7FFF => {
                let (_, rom_upper) = self.rom_offsets;
                rom_upper as usize | (addr as usize & 0x3FFF)
            }
            _ => 0,
        };

        self.rom[bank_addr as usize]
    }

    #[must_use]
    pub fn ram_addr(&self, addr: u16) -> usize {
        self.ram_offset | (addr as usize & 0x1FFF)
    }

    fn mbc_read_ram(&self, ram_enabled: bool, addr: u16) -> u8 {
        if !self.ram.is_empty() && ram_enabled {
            let addr = self.ram_addr(addr);
            self.ram[addr]
        } else {
            0xFF
        }
    }

    #[must_use]
    pub fn read_ram(&self, addr: u16) -> u8 {
        match self.mbc {
            MbcNone => 0xFF,
            Mbc1 | Mbc5 => self.mbc_read_ram(self.ram_enabled, addr),
            Mbc2 => (self.mbc_read_ram(self.ram_enabled, addr) & 0xF) | 0xF0,
            Mbc3 => match self.ram_bank {
                0x00..=0x03 => self.mbc_read_ram(self.ram_enabled, addr),
                0x04..=0x07 => self.mbc_read_ram(self.ram_enabled && self.mbc30, addr),
                _ => 0xFF,
            },
        }
    }

    fn mbc1_rom_offsets(&self) -> (usize, usize) {
        let upper_bits = if self.mbc1_multicart {
            self.rom_bank_hi << 4
        } else {
            self.rom_bank_hi << 5
        };
        let lower_bits = if self.mbc1_multicart {
            self.rom_bank_lo & 0xF
        } else {
            self.rom_bank_lo
        };

        let lower_bank = if self.mbc1_bank_mode {
            upper_bits as usize
        } else {
            0
        };
        let upper_bank = (upper_bits | lower_bits) as usize;
        (ROM_BANK_SIZE * lower_bank, ROM_BANK_SIZE * upper_bank)
    }

    fn mbc1_ram_offset(&self) -> usize {
        let bank = if self.mbc1_bank_mode {
            self.rom_bank_hi as usize
        } else {
            0
        };
        RAM_BANK_SIZE * bank
    }

    fn mbc5_rom_offsets(&self) -> (usize, usize) {
        let lower_bits = self.rom_bank_lo as usize;
        let upper_bits = (self.rom_bank_hi as usize) << 8;
        let rom_bank = (upper_bits | lower_bits) & self.rom_bank_mask;
        // let rom_bank = if rom_bank == 0 { 1 } else { rom_bank };
        (0x0000, ROM_BANK_SIZE * rom_bank)
    }

    pub fn write_rom(&mut self, addr: u16, val: u8) {
        match self.mbc {
            MbcNone => (),
            Mbc1 => match addr {
                0x0000..=0x1FFF => self.ram_enabled = (val & 0xF) == 0xA,
                0x2000..=0x3FFF => {
                    let val = val & 0x1F;
                    self.rom_bank_lo = if val == 0 { 1 } else { val };
                    self.rom_offsets = self.mbc1_rom_offsets();
                }
                0x4000..=0x5FFF => {
                    self.rom_bank_hi = val & 3;
                    self.rom_offsets = self.mbc1_rom_offsets();
                    self.ram_offset = self.mbc1_ram_offset();
                }
                0x6000..=0x7FFF => {
                    self.mbc1_bank_mode = (val & 1) == 1;
                    self.rom_offsets = self.mbc1_rom_offsets();
                    self.ram_offset = self.mbc1_ram_offset();
                }
                _ => (),
            },
            Mbc2 => {
                if addr <= 0x3FFF {
                    if (addr >> 8) & 1 == 0 {
                        self.ram_enabled = (val & 0xF) == 0xA;
                    } else {
                        let val = val & 0xF;
                        self.rom_bank_lo = if val == 0 { 1 } else { val };
                        self.rom_offsets = (0x0000, ROM_BANK_SIZE * self.rom_bank_lo as usize);
                    }
                }
            }
            Mbc3 => match addr {
                0x0000..=0x1FFF => self.ram_enabled = (val & 0x0F) == 0x0A,
                0x2000..=0x3FFF => {
                    self.rom_bank_lo = if val == 0 { 1 } else { val & 0x7F };
                    self.rom_offsets = (0x0000, ROM_BANK_SIZE * self.rom_bank_lo as usize);
                }
                0x4000..=0x5FFF => {
                    self.ram_bank = val & 0x7;
                    if self.mbc30 {
                        self.ram_offset = RAM_BANK_SIZE * self.ram_bank as usize;
                    } else {
                        self.ram_offset = RAM_BANK_SIZE * (self.ram_bank & 0x3) as usize;
                    }
                }

                _ => (),
            },
            Mbc5 => match addr {
                0x0000..=0x1FFF => self.ram_enabled = val & 0xF == 0xA,
                0x2000..=0x2FFF => {
                    self.rom_bank_lo = val;
                    self.rom_offsets = self.mbc5_rom_offsets();
                }
                0x3000..=0x3FFF => {
                    self.rom_bank_hi = val & 1;
                    self.rom_offsets = self.mbc5_rom_offsets();
                }
                0x4000..=0x5FFF => {
                    self.ram_bank = val & 0xF;
                    self.ram_offset = RAM_BANK_SIZE * self.ram_bank as usize;
                }
                _ => (),
            },
        }
    }

    pub fn mbc_write_ram(&mut self, ram_enabled: bool, addr: u16, val: u8) {
        if !self.ram.is_empty() && ram_enabled {
            let addr = self.ram_addr(addr);
            self.ram[addr] = val;
        }
    }

    pub fn write_ram(&mut self, addr: u16, val: u8) {
        match self.mbc {
            MbcNone => (),
            Mbc1 | Mbc2 | Mbc5 => {
                self.mbc_write_ram(self.ram_enabled, addr, val);
            }
            Mbc3 => match self.ram_bank {
                0x00..=0x03 => self.mbc_write_ram(self.ram_enabled, addr, val),
                0x04..=0x07 => self.mbc_write_ram(self.ram_enabled && self.mbc30, addr, val),
                _ => (),
            },
        }
    }

    #[must_use]
    pub fn ram(&self) -> &[u8] {
        &self.ram
    }
}

pub struct Header {
    title: [u8; 15],
    ram_size: RAMSize,
    rom_size: ROMSize,
    cgb_flag: CgbFlag,
}

impl Header {
    /// # Errors
    ///
    /// Will return `Err` if the ROM header contains some
    /// illegal value
    pub fn new(rom: &[u8]) -> Result<Self, Error> {
        let cgb_flag = CgbFlag::new(rom);
        let rom_size = ROMSize::new(rom)?;
        let ram_size = RAMSize::new(rom)?;
        let mut title: [u8; 15] = [0; 15];

        // length of title depends on licensee code format:
        // - 0x33: new value, short title
        // - any other: old value, long title
        match rom[0x14B] {
            0x33 => title[..(NEW_TITLE_END - TITLE_START)]
                .copy_from_slice(&rom[TITLE_START..NEW_TITLE_END]),
            _ => title.copy_from_slice(&rom[TITLE_START..OLD_TITLE_END]),
        };

        // Check title is valid ascii
        let _ = str::from_utf8(&title).map_err(|utf8_error| {
            let invalid_byte_position = TITLE_START + utf8_error.valid_up_to();
            let invalid_byte = rom[TITLE_START + invalid_byte_position];
            Error::InvalidTitleString {
                invalid_byte,
                invalid_byte_position,
            }
        })?;

        Self::check_checksum(rom)?;

        Ok(Self {
            title,
            ram_size,
            rom_size,
            cgb_flag,
        })
    }

    fn check_checksum(rom: &[u8]) -> Result<(), Error> {
        let expected = rom[0x14D];
        let mut computed: u8 = 0;

        for &byte in rom.iter().take(0x14C + 1).skip(0x134) {
            computed = computed.wrapping_sub(byte).wrapping_sub(1);
        }

        if expected == computed {
            Ok(())
        } else {
            Err(Error::InvalidChecksum { expected, computed })
        }
    }

    #[must_use]
    pub fn ram_size(&self) -> RAMSize {
        self.ram_size
    }

    #[must_use]
    pub fn cgb_flag(&self) -> CgbFlag {
        self.cgb_flag
    }

    /// # Panics
    ///
    /// panics on invalid ASCII title in header
    #[must_use]
    pub fn title(&self) -> &str {
        str::from_utf8(&self.title).unwrap()
    }

    #[must_use]
    pub fn rom_size(&self) -> ROMSize {
        self.rom_size
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ROMSize {
    Kb32  = 0,
    Kb64  = 1,
    Kb128 = 2,
    Kb256 = 3,
    Kb512 = 4,
    Mb1   = 5,
    Mb2   = 6,
    Mb4   = 7,
    Mb8   = 8,
}

impl ROMSize {
    pub fn new(slice: &[u8]) -> Result<Self, Error> {
        use ROMSize::{Kb128, Kb256, Kb32, Kb512, Kb64, Mb1, Mb2, Mb4, Mb8};
        let rom_size_byte = slice[0x148];
        let rom_size = match rom_size_byte {
            0 => Kb32,
            1 => Kb64,
            2 => Kb128,
            3 => Kb256,
            4 => Kb512,
            5 => Mb1,
            6 => Mb2,
            7 => Mb4,
            8 => Mb8,
            _ => return Err(Error::InvalidRomSize { rom_size_byte }),
        };

        Ok(rom_size)
    }

    // total size in  bytes
    pub fn size_bytes(self) -> usize {
        let kib_32 = 1 << 15;
        kib_32 << (self as usize)
    }

    pub fn num_banks(self) -> usize {
        2 << (self as usize)
    }

    pub fn bank_bit_mask(self) -> usize {
        (2 << (self as usize)) - 1
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RAMSize {
    None,
    Kb2,
    Kb8,
    Kb32,
    Kb128,
    Kb64,
}

impl RAMSize {
    pub fn new(slice: &[u8]) -> Result<Self, Error> {
        use RAMSize::{Kb128, Kb2, Kb32, Kb64, Kb8, None};
        let ram_size_byte = slice[0x149];
        let ram_size = match ram_size_byte {
            0 => None,
            1 => Kb2,
            2 => Kb8,
            3 => Kb32,
            4 => Kb128,
            5 => Kb64,
            _ => return Err(Error::InvalidRamSize { ram_size_byte }),
        };
        Ok(ram_size)
    }

    pub fn total_size_in_bytes(self) -> usize {
        self.num_banks() as usize * self.bank_size_in_bytes() as usize
    }

    pub fn num_banks(self) -> usize {
        match self {
            RAMSize::None => 0,
            RAMSize::Kb2 | RAMSize::Kb8 => 1,
            RAMSize::Kb32 => 4,
            RAMSize::Kb128 => 16,
            RAMSize::Kb64 => 8,
        }
    }

    pub fn bank_size_in_bytes(self) -> usize {
        use RAMSize::{Kb128, Kb2, Kb32, Kb64, Kb8, None};
        match self {
            None => 0,
            Kb2 => 0x800,
            Kb8 | Kb32 | Kb128 | Kb64 => 0x2000,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum CgbFlag {
    NonCgb,
    CgbOnly,
    CgbFunctions,
}

impl CgbFlag {
    // Since both cgb flags are outside the ASCII range we don't
    // need to check if the header is new or old
    #[must_use]
    pub fn new(rom: &[u8]) -> Self {
        use CgbFlag::{CgbFunctions, CgbOnly, NonCgb};
        match rom[0x143] {
            0x80 => CgbFunctions,
            0xC0 => CgbOnly,
            _ => NonCgb,
        }
    }
}
