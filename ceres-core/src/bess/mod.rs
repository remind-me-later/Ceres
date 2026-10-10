// Best Effort Save State (https://github.com/LIJI32/SameBoy/blob/master/BESS.md)
// Every integer is in little-endian byte order

mod read;
mod write;

/// CORE: the version, the model, the CPU registers and the I/O registers,
/// then a size and an offset for each of the 7 memories.
const CORE_BLOCK_SIZE: u32 = 0xD0;
/// The CPU state (0x10 bytes) and the I/O registers (0x80).
const CORE_REGISTERS_SIZE: usize = 0x90;
/// INFO: the title (0x10 bytes) and the global checksum.
const INFO_BLOCK_SIZE: u32 = 0x12;
const INFO_TITLE_SIZE: usize = 0x10;
/// RTC: the 10 clock registers (4 bytes each) and a 64-bit timestamp.
const RTC_BLOCK_SIZE: u32 = 0x30;
/// The CGB's background and object palette memories.
const CGB_PALETTES_SIZE: u32 = 0x40;
/// The footer: the offset of the first block, then "BESS".
const FOOTER_SIZE: usize = 8;

pub use read::Reader;
pub use write::Writer;
