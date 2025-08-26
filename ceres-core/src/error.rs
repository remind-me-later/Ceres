use core::{error, fmt};
use fmt::Display;

#[derive(Debug)]
pub enum Error {
    InvalidGameGenieCodeFormat { code: String },
    InvalidGameGenieCodeLength { code: String },
    InvalidRamSize,
    InvalidRomSize,
    NonAsciiTitleString,
    RamSizeDifferentThanActual { expected: u32, actual: u32 },
    RomSizeDifferentThanActual { expected: u32, actual: u32 },
    TooManyGameGenieCodes,
    UnsupportedMBC { mbc_hex_code: u8 },
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidGameGenieCodeLength { code } => {
                write!(f, "invalid Game Genie code length: {code}")
            }
            Self::InvalidGameGenieCodeFormat { code } => {
                write!(f, "invalid Game Genie code format: {code}")
            }
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
                write!(f, "unsupported MBC: {mbc_hex_code:02X}")
            }
            Self::RomSizeDifferentThanActual { expected, actual } => write!(
                f,
                "header ROM size is different from the size of the supplied file: expected {expected} bytes, got {actual} bytes"
            ),
            Self::RamSizeDifferentThanActual { expected, actual } => write!(
                f,
                "header RAM size is different from the size of the supplied file: expected {expected} bytes, got {actual} bytes"
            ),
            Self::TooManyGameGenieCodes => {
                write!(f, "too many Game Genie codes activated (maximum is 3)")
            }
        }
    }
}

impl error::Error for Error {}
