use {crate::Error, core::fmt::Display};

const TITLE_START: usize = 0x134;
const OLD_TITLE_END: usize = 0x143;
const NEW_TITLE_END: usize = 0x13f;

pub struct Header {
    title: [u8; 15],
    ram_size: RAMSize,
    rom_size: ROMSize,
    licensee_code: LicenseeCode,
    cgb_flag: CgbFlag,
}

impl Display for Header {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let title = core::str::from_utf8(&self.title).unwrap();

        write!(
            f,
            "Title - {}\nRAM - {}\nROM - {}\nLicensee code - {}\nCGB flag: {}",
            title, self.ram_size, self.rom_size, self.licensee_code, self.cgb_flag
        )
    }
}

impl Header {
    /// # Errors
    ///
    /// Will return `Err` if the ROM header contains some illegal value
    pub fn new(rom: &[u8]) -> Result<Self, Error> {
        let licensee_code = LicenseeCode::new(rom);
        let cgb_flag = CgbFlag::new(rom);
        let rom_size = ROMSize::new(rom)?;
        let ram_size = RAMSize::new(rom)?;
        let mut title: [u8; 15] = [0; 15];

        match licensee_code {
            LicenseeCode::Old(_) => title.copy_from_slice(&rom[TITLE_START..OLD_TITLE_END]),
            LicenseeCode::New(_) => title[..(NEW_TITLE_END - TITLE_START)]
                .copy_from_slice(&rom[TITLE_START..NEW_TITLE_END]),
        };

        // Check title is valid ascii
        let _ = core::str::from_utf8(&title).map_err(|utf8_error| {
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
            licensee_code,
            cgb_flag,
        })
    }

    fn check_checksum(rom: &[u8]) -> Result<(), Error> {
        let expected = rom[0x14d];
        let mut computed: u8 = 0;

        for &byte in rom.iter().take(0x14c + 1).skip(0x134) {
            computed = computed.wrapping_sub(byte).wrapping_sub(1);
        }

        if expected == computed {
            Ok(())
        } else {
            Err(Error::InvalidChecksum { expected, computed })
        }
    }

    #[must_use]
    pub fn ram_size(&self) -> &RAMSize {
        &self.ram_size
    }

    #[must_use]
    pub fn cgb_flag(&self) -> &CgbFlag {
        &self.cgb_flag
    }

    /// # Panics
    ///
    /// panics on invalid ASCII title in header
    #[must_use]
    pub fn title(&self) -> &str {
        core::str::from_utf8(&self.title).unwrap()
    }

    #[must_use]
    pub fn rom_size(&self) -> ROMSize {
        self.rom_size
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ROMSize {
    Kb32,
    Kb64,
    Kb128,
    Kb256,
    Kb512,
    Mb1,
    Mb2,
    Mb4,
    Mb8,
}

impl Display for ROMSize {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "number of banks: {}, total: {}B",
            self.number_of_banks(),
            self.total_size_in_bytes()
        )
    }
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

    pub fn total_size_in_bytes(self) -> usize {
        const KIB_32_AS_BYTES: usize = 1 << 15;

        let exponent = match self {
            ROMSize::Kb32 => 0,
            ROMSize::Kb64 => 1,
            ROMSize::Kb128 => 2,
            ROMSize::Kb256 => 3,
            ROMSize::Kb512 => 4,
            ROMSize::Mb1 => 5,
            ROMSize::Mb2 => 6,
            ROMSize::Mb4 => 7,
            ROMSize::Mb8 => 8,
        };

        KIB_32_AS_BYTES << exponent
    }

    pub fn number_of_banks(self) -> usize {
        match self {
            ROMSize::Kb32 => 2,
            ROMSize::Kb64 => 4,
            ROMSize::Kb128 => 8,
            ROMSize::Kb256 => 16,
            ROMSize::Kb512 => 32,
            ROMSize::Mb1 => 64,
            ROMSize::Mb2 => 128,
            ROMSize::Mb4 => 256,
            ROMSize::Mb8 => 512,
        }
    }

    pub fn banks_bit_mask(self) -> usize {
        // log2(number_of_banks) - 1
        match self {
            ROMSize::Kb32 => 0x1,
            ROMSize::Kb64 => 0x3,
            ROMSize::Kb128 => 0x7,
            ROMSize::Kb256 => 0xf,
            ROMSize::Kb512 => 0x1f,
            ROMSize::Mb1 => 0x3f,
            ROMSize::Mb2 => 0x7f,
            ROMSize::Mb4 => 0xff,
            ROMSize::Mb8 => 0x1ff,
        }
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

impl Display for RAMSize {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "bank size: {}B, number of banks: {}, total: {}B",
            self.bank_size_in_bytes(),
            self.number_of_banks(),
            self.total_size_in_bytes()
        )
    }
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
        self.number_of_banks() as usize * self.bank_size_in_bytes() as usize
    }

    pub fn number_of_banks(self) -> usize {
        match self {
            RAMSize::None => 0,
            RAMSize::Kb2 | RAMSize::Kb8 => 1,
            RAMSize::Kb32 => 4,
            RAMSize::Kb128 => 16,
            RAMSize::Kb64 => 8,
        }
    }

    pub fn bank_size_in_bytes(self) -> usize {
        match self {
            RAMSize::None => 0,
            RAMSize::Kb2 => 0x800,
            RAMSize::Kb8 | RAMSize::Kb32 | RAMSize::Kb128 | RAMSize::Kb64 => 0x2000,
        }
    }
}

pub enum LicenseeCode {
    Old(u8),
    New([u8; 2]),
}

impl LicenseeCode {
    pub fn new(rom: &[u8]) -> Self {
        use LicenseeCode::{New, Old};

        match rom[0x14b] {
            0x33 => New([rom[0x144], rom[0x145]]),
            code => Old(code),
        }
    }
}

impl Display for LicenseeCode {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            LicenseeCode::Old(c) => write!(f, "old: {:#04x}", c),
            LicenseeCode::New(c) => write!(f, "new: {:#04x}{:#04x}", c[0], c[1]),
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum CgbFlag {
    NonCgb,
    CgbOnly,
    CgbFunctions,
}

impl Display for CgbFlag {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                CgbFlag::NonCgb => "no CGB support",
                CgbFlag::CgbOnly => "supports CGB functions",
                CgbFlag::CgbFunctions => "CGB only",
            }
        )
    }
}

impl CgbFlag {
    // Since both cgb flags are outside the ASCII range we don't need to check if the header is new or old
    #[must_use]
    pub fn new(rom: &[u8]) -> Self {
        use CgbFlag::{CgbFunctions, CgbOnly, NonCgb};
        match rom[0x143] {
            0x80 => CgbFunctions,
            0xc0 => CgbOnly,
            _ => NonCgb,
        }
    }
}
