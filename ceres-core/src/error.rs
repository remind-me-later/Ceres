use core::{error, fmt};
use fmt::Display;

const COMMON_GAME_GENIE_FORMAT_STRING: &str =
    "expected a game genie code of the form ABC-DEF-GHI, where A..I are hex digits";

#[non_exhaustive]
#[derive(Debug)]
pub enum Error {
    InvalidGameGenieCodeExpectedHyphen { pos: u8 },
    InvalidGameGenieCodeLength { actual: usize },
    InvalidGameGenieCodeNotHexDigit { pos: u8 },
    InvalidRamSize,
    InvalidRomHeaderSize,
    InvalidRomSize,
    // FIXME: add variants for invalid save state details
    InvalidSaveState,
    NonAsciiTitleString,
    RamSizeDifferentThanActual { expected: u32, actual: u32 },
    RomSizeDifferentThanActual { expected: u32, actual: u32 },
    TooManyGameGenieCodes,
    UnsupportedMBC { mbc_hex_code: u8 },
}

impl Display for Error {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::InvalidGameGenieCodeLength { actual } => write!(
                f,
                "{COMMON_GAME_GENIE_FORMAT_STRING}: expected length 11, got {actual}",
            ),
            Self::InvalidGameGenieCodeExpectedHyphen { pos } => {
                write!(
                    f,
                    "{COMMON_GAME_GENIE_FORMAT_STRING}: missing hyphen at character position {}",
                    pos + 1
                )
            }
            Self::InvalidGameGenieCodeNotHexDigit { pos } => {
                write!(
                    f,
                    "{COMMON_GAME_GENIE_FORMAT_STRING}: expected hex digit at character position {}",
                    pos + 1
                )
            }
            Self::InvalidRomHeaderSize => {
                write!(
                    f,
                    "ROM is too small to be a valid cartridge, header must be at least 0x150 bytes"
                )
            }
            Self::InvalidRomSize => {
                write!(f, "invalid ROM size in cartridge header")
            }
            Self::InvalidRamSize => {
                write!(f, "invalid RAM size in cartridge header")
            }
            Self::InvalidSaveState => {
                write!(f, "invalid save state")
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
