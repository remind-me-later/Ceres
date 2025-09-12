use crate::Error;

#[expect(
    clippy::arbitrary_source_item_ordering,
    reason = "The order follows the ROM size"
)]
#[derive(Clone, Copy, Debug, Default)]
pub enum ROMSize {
    #[default]
    Kb32 = 0,
    Kb64 = 1,
    Kb128 = 2,
    Kb256 = 3,
    Kb512 = 4,
    Mb1 = 5,
    Mb2 = 6,
    Mb4 = 7,
    Mb8 = 8,
}

impl ROMSize {
    pub const BANK_SIZE: u16 = 0x4000;

    #[must_use]
    pub const fn mask(self) -> u16 {
        // maximum is 2 << 8 - 1 = 1FF
        (2_u16 << (self as u8)) - 1
    }

    pub const fn new(byte: u8) -> Result<Self, Error> {
        use ROMSize::{Kb32, Kb64, Kb128, Kb256, Kb512, Mb1, Mb2, Mb4, Mb8};
        let rom_size = match byte {
            0 => Kb32,
            1 => Kb64,
            2 => Kb128,
            3 => Kb256,
            4 => Kb512,
            5 => Mb1,
            6 => Mb2,
            7 => Mb4,
            8 => Mb8,
            _ => return Err(Error::InvalidRomSize),
        };

        Ok(rom_size)
    }

    #[must_use]
    pub const fn size_bytes(self) -> u32 {
        // maximum is 0x8000 << 8
        (Self::BANK_SIZE as u32 * 2) << (self as u8)
    }
}
