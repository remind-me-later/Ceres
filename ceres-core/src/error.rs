use core::{error, fmt};
use std::fmt::Display;

#[derive(Debug)]
pub enum Error {
    InvalidRomSize,
    InvalidRamSize,
    NonAsciiTitleString,
    UnsupportedMBC { mbc_hex_code: u8 },
    RomSizeDifferentThanActual { expected: usize, actual: usize },
    RamSizeDifferentThanActual { expected: usize, actual: usize },
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRomSize => {
                write!(f, "invalid ROM size in cartridge header")
            }
            Self::InvalidRamSize => {
                write!(f, "invalid RAM size in cartridge header")
            }
            Self::NonAsciiTitleString => write!(
                f,
                "invalid title string in cartridge header, contains non ASCII \
         characters"
            ),
            Self::UnsupportedMBC { mbc_hex_code } => {
                write!(f, "unsupported MBC: {mbc_hex_code:#0x}")
            }
            Self::RomSizeDifferentThanActual { expected, actual } => write!(
                f,
                "header ROM size is different from the size of the supplied file: expected {expected} bytes, got {actual} bytes"
            ),
            Self::RamSizeDifferentThanActual { expected, actual } => write!(
                f,
                "header RAM size is different from the size of the supplied file: expected {expected} bytes, got {actual} bytes"
            ),
        }
    }
}

impl error::Error for Error {}
