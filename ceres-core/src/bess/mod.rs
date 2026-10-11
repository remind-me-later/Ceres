//! Best Effort Save State 1.0
//! (`external/reference-implementations/SameBoy/BESS.md`).
//!
//! Ceres has no native state format: a state is the memories followed by the
//! BESS blocks, which carry everything that is restored. Every integer is in
//! little-endian byte order.

mod read;
mod write;

use crate::Model;

/// CORE: the version, the model, the CPU registers and the I/O registers,
/// then a size and an offset for each of the 7 memories.
const CORE_BLOCK_SIZE: u32 = 0xD0;
/// The I/O registers, 0xFF00-0xFF7F.
const IO_SIZE: usize = 0x80;
/// INFO: the title (0x10 bytes) and the global checksum.
const INFO_BLOCK_SIZE: u32 = 0x12;
/// XOAM: the memory behind the unusable area, 0xFEA0-0xFEFF.
const XOAM_BLOCK_SIZE: u32 = 0x60;
/// RTC: the 10 clock registers (4 bytes each) and a 64-bit timestamp.
const RTC_BLOCK_SIZE: u32 = 0x30;
/// The other cartridges' clock and accessory blocks, checked but not used.
const HUC3_BLOCK_SIZE: u32 = 0x11;
const TPP1_BLOCK_SIZE: u32 = 0x11;
const MBC7_BLOCK_SIZE: u32 = 0x0A;
/// An MBC block entry: a 16-bit address and the byte written there.
const MBC_PAIR_SIZE: usize = 3;
/// The footer: the offset of the first block, then "BESS".
const FOOTER_SIZE: usize = 8;

const MAJOR_VERSION: u16 = 1;
const MINOR_VERSION: u16 = 1;

pub(crate) use read::Reader;
pub(crate) use write::Writer;

/// The memories of the CORE block, in the order of its sizes and offsets.
/// The writer puts their data in the same order, from offset 0.
#[derive(Clone, Copy)]
enum Memory {
    Ram,
    Vram,
    MbcRam,
    Oam,
    Hram,
    BgPalette,
    ObjPalette,
}

impl Memory {
    const COUNT: usize = 7;
}

/// A memory's size and offset in the state.
#[derive(Clone, Copy, Default)]
struct Buffer {
    size: u32,
    offset: u32,
}

/// Where each memory is in the state.
#[derive(Default)]
struct Layout([Buffer; Memory::COUNT]);

impl Layout {
    /// The memories of `sizes` one after the other, from offset 0.
    fn contiguous(sizes: [u32; Memory::COUNT]) -> Self {
        let mut offset = 0;
        Self(sizes.map(|size| {
            let buffer = Buffer { size, offset };
            offset += size;
            buffer
        }))
    }

    const fn get(&self, memory: Memory) -> Buffer {
        self.0[memory as usize]
    }
}

/// The CPU's execution state in the CORE block.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Execution {
    Running = 0,
    Halted = 1,
    Stopped = 2,
}

/// A model's four-character BESS identifier: the family (`G`, `S` or `C`),
/// the model in it, and the CPU revision where the family has them.
const fn model_id(model: Model) -> [u8; 4] {
    *match model {
        Model::Dmg0 => b"GD0 ",
        Model::DmgB => b"GDB ",
        Model::Mgb => b"GM  ",
        Model::Sgb => b"SN  ",
        Model::Sgb2 => b"S2  ",
        Model::Cgb0 => b"CC0 ",
        Model::CgbA => b"CCA ",
        Model::CgbB => b"CCB ",
        Model::CgbC => b"CCC ",
        Model::CgbD => b"CCD ",
        Model::CgbE => b"CCE ",
        Model::Agb => b"CA  ",
    }
}

/// A state from a model of `family` can be loaded on `model`: the families
/// are mutually incompatible, the models and revisions in one are not.
const fn same_family(family: u8, model: Model) -> bool {
    match family {
        b'G' => matches!(model, Model::Dmg0 | Model::DmgB | Model::Mgb),
        b'S' => matches!(model, Model::Sgb | Model::Sgb2),
        b'C' => model.is_cgb_hardware(),
        _ => false,
    }
}

#[cfg(test)]
mod tests;
