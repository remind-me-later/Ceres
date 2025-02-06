// Best Effort Save State (https://github.com/LIJI32/SameBoy/blob/master/BESS.md)
// Every integer is in little-endian byte order

use crate::{
    ppu::{OAM_SIZE, VRAM_SIZE_CGB, VRAM_SIZE_GB},
    AudioCallback, Cart, CgbMode, Gb,
};
use core::slice::IterMut;

fn write_footer(iter: &mut IterMut<u8>, offset_to_first_block: u32) {
    const LITERAL: &str = "BESS";

    for byte in offset_to_first_block.to_le_bytes() {
        *iter.next().unwrap() = byte;
    }

    for byte in LITERAL.as_bytes() {
        *iter.next().unwrap() = *byte;
    }
}

fn write_block_header(iter: &mut IterMut<u8>, name: &str, size: u32) {
    for byte in name.as_bytes() {
        *iter.next().unwrap() = *byte;
    }

    for byte in size.to_le_bytes() {
        *iter.next().unwrap() = byte;
    }
}

fn write_name_block(iter: &mut IterMut<u8>) {
    const EMULATOR_NAME: &str = "Ceres, 0.1.0";

    write_block_header(iter, "NAME", EMULATOR_NAME.len() as u32);
    for byte in EMULATOR_NAME.as_bytes() {
        *iter.next().unwrap() = *byte;
    }
}

fn write_info_block(iter: &mut IterMut<u8>, cart: &Cart) {
    const INFO_BLOCK_SIZE: u32 = 0x12;

    write_block_header(iter, "INFO", INFO_BLOCK_SIZE);

    // pad title to 0x10 bytes
    let mut title = [0; 0x10];
    let title_len = cart.ascii_title().len();
    title[0..title_len].copy_from_slice(cart.ascii_title());

    for byte in title.iter() {
        *iter.next().unwrap() = *byte;
    }

    for byte in cart.global_checksum().to_le_bytes() {
        *iter.next().unwrap() = byte;
    }
}

fn write_core_block<C: AudioCallback>(gb: &Gb<C>, iter: &mut IterMut<u8>) {
    write_block_header(iter, "CORE", 0xD0);

    // BESS Version
    {
        const MAJOR_VERSION: u16 = 1;
        const MINOR_VERSION: u16 = 1;

        for byte in MAJOR_VERSION.to_le_bytes() {
            *iter.next().unwrap() = byte;
        }

        for byte in MINOR_VERSION.to_le_bytes() {
            *iter.next().unwrap() = byte;
        }
    }

    // Model
    {
        let model = match gb.model {
            crate::Model::Dmg => "GD  ",
            crate::Model::Mgb => "GM  ",
            crate::Model::Cgb => "CC  ",
        };

        for byte in model.as_bytes() {
            *iter.next().unwrap() = *byte;
        }
    }

    // CPU Registers
    {
        // PC
        for byte in gb.pc.to_le_bytes() {
            *iter.next().unwrap() = byte;
        }

        // AF
        for byte in gb.af.to_le_bytes() {
            *iter.next().unwrap() = byte;
        }

        // BC
        for byte in gb.bc.to_le_bytes() {
            *iter.next().unwrap() = byte;
        }

        // DE
        for byte in gb.de.to_le_bytes() {
            *iter.next().unwrap() = byte;
        }

        // HL
        for byte in gb.hl.to_le_bytes() {
            *iter.next().unwrap() = byte;
        }

        // SP
        for byte in gb.sp.to_le_bytes() {
            *iter.next().unwrap() = byte;
        }

        // IME
        *iter.next().unwrap() = gb.ints.are_enabled() as u8;

        // IE
        *iter.next().unwrap() = gb.ints.read_ie();

        // Excution state
        // TODO: stopped
        *iter.next().unwrap() = if gb.cpu_halted { 1 } else { 0 };

        // Reserved 0
        *iter.next().unwrap() = 0;

        // Every memory mapped register
        for i in 0xFF00..0xFF80 {
            *iter.next().unwrap() = gb.read_mem(i);
        }

        // Size of RAM, depends on CGB mode
        let ram_size = match gb.cgb_mode {
            CgbMode::Dmg | CgbMode::Compat => u32::from(crate::WRAM_SIZE_GB),
            CgbMode::Cgb => u32::from(crate::WRAM_SIZE_CGB),
        };

        for byte in ram_size.to_le_bytes() {
            *iter.next().unwrap() = byte;
        }

        // Offset from file start to RAM, always 0
        for byte in 0_u32.to_le_bytes() {
            *iter.next().unwrap() = byte;
        }

        // Size of VRAM, depends only on CGB mode right now
        let vram_size = match gb.cgb_mode {
            CgbMode::Dmg | CgbMode::Compat => u32::from(VRAM_SIZE_GB),
            CgbMode::Cgb => u32::from(VRAM_SIZE_CGB),
        };

        for byte in vram_size.to_le_bytes() {
            *iter.next().unwrap() = byte;
        }

        // Offset to VRAM from file start, is stored right after RAM
        let vram_offset = ram_size;

        for byte in vram_offset.to_le_bytes() {
            *iter.next().unwrap() = byte;
        }

        // MBC RAM size
        let mbc_ram_size = gb.cart.ram_size_bytes();

        for byte in mbc_ram_size.to_le_bytes() {
            *iter.next().unwrap() = byte;
        }

        // Offset to MBC RAM from file start, is stored right after VRAM
        let mbc_ram_offset = vram_offset + vram_size;

        for byte in mbc_ram_offset.to_le_bytes() {
            *iter.next().unwrap() = byte;
        }

        // OAM Size, constant
        let oam_size = u32::from(OAM_SIZE);

        for byte in oam_size.to_le_bytes() {
            *iter.next().unwrap() = byte;
        }

        // Offset to OAM from file start, is stored right after MBC RAM
        let oam_offset = mbc_ram_offset + mbc_ram_size;

        for byte in oam_offset.to_le_bytes() {
            *iter.next().unwrap() = byte;
        }

        // HRAM Size, constant
        let hram_size = u32::from(crate::HRAM_SIZE);

        for byte in hram_size.to_le_bytes() {
            *iter.next().unwrap() = byte;
        }

        // Offset to HRAM from file start, is stored right after OAM
        let hram_offset = oam_offset + oam_size;

        for byte in hram_offset.to_le_bytes() {
            *iter.next().unwrap() = byte;
        }

        // Background Palette size, constant
        let bg_palette_size = match gb.cgb_mode {
            CgbMode::Dmg | CgbMode::Compat => 0_u32,
            CgbMode::Cgb => 0x40_u32,
        };

        for byte in bg_palette_size.to_le_bytes() {
            *iter.next().unwrap() = byte;
        }

        // Offset to Background Palette from file start, is stored right after HRAM
        let bg_palette_offset = hram_offset + hram_size;

        for byte in bg_palette_offset.to_le_bytes() {
            *iter.next().unwrap() = byte;
        }
    }
}

fn write_end_block(iter: &mut IterMut<u8>) {
    write_block_header(iter, "END ", 0);
}
