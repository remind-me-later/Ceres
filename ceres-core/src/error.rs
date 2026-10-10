use core::{error, fmt};
use fmt::Display;

#[cfg(feature = "game_genie")]
const COMMON_GAME_GENIE_FORMAT_STRING: &str =
    "expected a game genie code of the form ABC-DEF-GHI, where A..I are hex digits";

#[non_exhaustive]
#[derive(Debug)]
pub enum Error {
    #[cfg(feature = "game_genie")]
    InvalidGameGenieCodeExpectedHyphen {
        pos: u8,
    },
    #[cfg(feature = "game_genie")]
    InvalidGameGenieCodeLength {
        actual: usize,
    },
    #[cfg(feature = "game_genie")]
    InvalidGameGenieCodeNotHexDigit {
        pos: u8,
    },
    InvalidRamSize,
    InvalidRomHeaderSize,
    InvalidRomSize,
    InvalidSaveState,
    #[cfg(feature = "game_genie")]
    TooManyGameGenieCodes,
    UnsupportedMBC {
        mbc_hex_code: u8,
    },
}

impl Display for Error {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            #[cfg(feature = "game_genie")]
            Self::InvalidGameGenieCodeLength { actual } => write!(
                f,
                "{COMMON_GAME_GENIE_FORMAT_STRING}: expected length 11, got {actual}",
            ),
            #[cfg(feature = "game_genie")]
            Self::InvalidGameGenieCodeExpectedHyphen { pos } => {
                write!(
                    f,
                    "{COMMON_GAME_GENIE_FORMAT_STRING}: missing hyphen at character position {}",
                    pos + 1
                )
            }
            #[cfg(feature = "game_genie")]
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
            Self::UnsupportedMBC { mbc_hex_code } => {
                write!(f, "unsupported MBC: {mbc_hex_code:02X}")
            }
            #[cfg(feature = "game_genie")]
            Self::TooManyGameGenieCodes => {
                write!(f, "too many Game Genie codes activated (maximum is 3)")
            }
        }
    }
}

impl error::Error for Error {}
