#[derive(Debug)]
pub enum Error {
    InvalidRomSize {
        rom_size_byte: u8,
    },
    InvalidRamSize {
        ram_size_byte: u8,
    },
    InvalidTitleString {
        invalid_byte: u8,
        invalid_byte_position: usize,
    },
    InvalidMBC {
        mbc_byte: u8,
    },
    InvalidChecksum {
        expected: u8,
        computed: u8,
    },
    InvalidLicenseeCode,
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Error::InvalidRomSize {
                rom_size_byte: rom_size,
            } => {
                write!(f, "Illegal value for ROM size '{rom_size}' at byte 0x148")
            }
            Error::InvalidRamSize {
                ram_size_byte: ram_size,
            } => {
                write!(f, "Illegal value for RAM size '{ram_size}' at byte 0x149")
            }
            Error::InvalidTitleString {
                invalid_byte,
                invalid_byte_position,
            } => {
                write!(f, "Illegal value for title string '{invalid_byte}' at byte '{invalid_byte_position:#x}' is not a legal ASCII value")
            }
            Error::InvalidMBC { mbc_byte } => {
                write!(f, "Unrecognized MBC '{mbc_byte}' at byte 0x147")
            }
            Error::InvalidChecksum { expected, computed } => {
                write!(
                    f,
                    "Invalid header checksum at byte 0x14d, expected: {expected}, computed: {computed}"
                )
            }
            // TODO: more info
            Error::InvalidLicenseeCode => {
                write!(f, "Invalid licensee code")
            }
        }
    }
}
