#[derive(Debug)]
pub enum Error {
    InvalidRomSize { rom_size_byte: u8 },
    InvalidRamSize { ram_size_byte: u8 },
    NonAsciiTitleString,
    InvalidMBC { mbc_byte: u8 },
    InvalidChecksum { expected: u8, computed: u8 },
    InvalidLicenseeCode,
}
