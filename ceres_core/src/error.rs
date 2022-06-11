#[derive(Debug)]
pub enum Error {
    InvalidRomSize,
    InvalidRamSize,
    NonAsciiTitleString,
    InvalidMBC,
    InvalidChecksum,
    InvalidLicenseeCode,
}
