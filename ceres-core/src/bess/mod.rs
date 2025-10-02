// Best Effort Save State (https://github.com/LIJI32/SameBoy/blob/master/BESS.md)
// Every integer is in little-endian byte order

mod read;
mod write;

pub use read::Reader;
pub use write::Writer;
