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
        // The bank count minus one: at most (2 << 8) - 1 = 0x1FF.
        (2_u16 << (self as u8)) - 1
    }

    /// The size the ROM is mapped with: the file length rounded up to a power
    /// of two, whatever the header says (as hardware does: the header is only
    /// read by the boot ROM and the mapper never sees it).
    pub const fn from_len(len: usize) -> Result<Self, Error> {
        use ROMSize::{Kb32, Kb64, Kb128, Kb256, Kb512, Mb1, Mb2, Mb4, Mb8};
        let rom_size = match len {
            0..=0x8000 => Kb32,
            0x8001..=0x1_0000 => Kb64,
            0x1_0001..=0x2_0000 => Kb128,
            0x2_0001..=0x4_0000 => Kb256,
            0x4_0001..=0x8_0000 => Kb512,
            0x8_0001..=0x10_0000 => Mb1,
            0x10_0001..=0x20_0000 => Mb2,
            0x20_0001..=0x40_0000 => Mb4,
            0x40_0001..=0x80_0000 => Mb8,
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
