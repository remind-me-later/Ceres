/// Represents a cartridge initialization error.
#[allow(clippy::module_name_repetitions)]
#[derive(Debug)]
pub enum CartridgeInitError {
    InvalidRomSize,
    InvalidRamSize,
    NonAsciiTitleString,
    UnsupportedMBC,
}
