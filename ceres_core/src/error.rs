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
