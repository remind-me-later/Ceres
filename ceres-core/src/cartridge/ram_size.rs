use crate::Error;

#[expect(
    clippy::arbitrary_source_item_ordering,
    reason = "The order follows the RAM size"
)]
#[derive(Debug, Default)]
pub enum RAMSize {
    #[default]
    NoRAM,
    Kb8,
    Kb32,
    Kb64,
    Kb128,
}

impl RAMSize {
    pub const BANK_SIZE: u16 = 0x2000;

    #[must_use]
    pub const fn has_ram(&self) -> bool {
        !matches!(self, Self::NoRAM)
    }

    #[must_use]
    pub const fn mask(&self) -> u8 {
        match self {
            Self::NoRAM | Self::Kb8 => 0x0,
            Self::Kb32 => 0x3,
            Self::Kb128 => 0xF,
            Self::Kb64 => 0x7,
        }
    }

    pub const fn new(byte: u8) -> Result<Self, Error> {
        use RAMSize::{Kb8, Kb32, Kb64, Kb128, NoRAM};
        let ram_size = match byte {
            0 => NoRAM,
            2 => Kb8,
            3 => Kb32,
            4 => Kb128,
            5 => Kb64,
            _ => return Err(Error::InvalidRamSize),
        };

        Ok(ram_size)
    }

    #[must_use]
    const fn num_banks(&self) -> u8 {
        match self {
            Self::NoRAM => 0x0,
            Self::Kb8 => 0x1,
            Self::Kb32 => 0x4,
            Self::Kb64 => 0x8,
            Self::Kb128 => 0x10,
        }
    }

    #[must_use]
    pub const fn size_bytes(&self) -> u32 {
        // Max size is 0x2000 * 0x10 = 0x20000 so it fits in a u32
        self.num_banks() as u32 * Self::BANK_SIZE as u32
    }
}
