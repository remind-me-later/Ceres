// Best Effort Save State (https://github.com/LIJI32/SameBoy/blob/master/BESS.md)
// Every integer is in little-endian byte order

use crate::{
    ppu::{OAM_SIZE, VRAM_SIZE_CGB, VRAM_SIZE_GB},
    AudioCallback, Cart, CgbMode, Gb,
};
use std::io::{Result, Write};

fn write_footer<W: Write>(writer: &mut W, offset_to_first_block: u32) -> Result<()> {
    const LITERAL: &str = "BESS";

    writer.write_all(&offset_to_first_block.to_le_bytes())?;
    writer.write_all(LITERAL.as_bytes())?;
    Ok(())
}

fn write_block_header<W: Write>(writer: &mut W, name: &str, size: u32) -> Result<()> {
    writer.write_all(name.as_bytes())?;
    writer.write_all(&size.to_le_bytes())?;
    Ok(())
}

fn write_name_block<W: Write>(writer: &mut W) -> Result<()> {
    const EMULATOR_NAME: &str = "Ceres, 0.1.0";

    write_block_header(writer, "NAME", EMULATOR_NAME.len() as u32)?;
    writer.write_all(EMULATOR_NAME.as_bytes())?;
    Ok(())
}

fn write_info_block<W: Write>(writer: &mut W, cart: &Cart) -> Result<()> {
    const INFO_BLOCK_SIZE: u32 = 0x12;

    write_block_header(writer, "INFO", INFO_BLOCK_SIZE)?;

    // pad title to 0x10 bytes
    let mut title = [0; 0x10];
    let title_bytes = cart.ascii_title();
    let title_len = title_bytes.len();
    title[0..title_len].copy_from_slice(title_bytes);

    writer.write_all(&title)?;
    writer.write_all(&cart.global_checksum().to_le_bytes())?;
    Ok(())
}

fn write_core_block<C: AudioCallback, W: Write>(
    gb: &Gb<C>,
    sizes: Sizes,
    writer: &mut W,
) -> Result<()> {
    write_block_header(writer, "CORE", 0xD0)?;

    // BESS Version
    {
        const MAJOR_VERSION: u16 = 1;
        const MINOR_VERSION: u16 = 1;

        writer.write_all(&MAJOR_VERSION.to_le_bytes())?;
        writer.write_all(&MINOR_VERSION.to_le_bytes())?;
    }

    // Model
    {
        let model = match gb.model {
            crate::Model::Dmg => "GD  ",
            crate::Model::Mgb => "GM  ",
            crate::Model::Cgb => "CC  ",
        };

        writer.write_all(model.as_bytes())?;
    }

    // CPU Registers
    {
        // PC
        writer.write_all(&gb.pc.to_le_bytes())?;

        // AF
        writer.write_all(&gb.af.to_le_bytes())?;

        // BC
        writer.write_all(&gb.bc.to_le_bytes())?;

        // DE
        writer.write_all(&gb.de.to_le_bytes())?;

        // HL
        writer.write_all(&gb.hl.to_le_bytes())?;

        // SP
        writer.write_all(&gb.sp.to_le_bytes())?;

        // IME
        writer.write_all(&[gb.ints.are_enabled() as u8])?;

        // IE
        writer.write_all(&[gb.ints.read_ie()])?;

        // Execution state (stopped TODO)
        writer.write_all(&[if gb.cpu_halted { 1 } else { 0 }])?;

        // Reserved 0
        writer.write_all(&[0])?;

        // Every memory mapped register
        for i in 0xFF00..0xFF80 {
            writer.write_all(&[gb.read_mem(i)])?;
        }
    }

    // Sizes
    {
        writer.write_all(&sizes.ram_size.to_le_bytes())?;
        writer.write_all(&sizes.ram_offset().to_le_bytes())?;
        writer.write_all(&sizes.vram_size.to_le_bytes())?;
        writer.write_all(&sizes.vram_offset().to_le_bytes())?;
        writer.write_all(&sizes.mbc_ram_size.to_le_bytes())?;
        writer.write_all(&sizes.mbc_ram_offset().to_le_bytes())?;
        writer.write_all(&sizes.oam_size.to_le_bytes())?;
        writer.write_all(&sizes.oam_offset().to_le_bytes())?;
        writer.write_all(&sizes.hram_size.to_le_bytes())?;
        writer.write_all(&sizes.hram_offset().to_le_bytes())?;
        writer.write_all(&sizes.bg_palette_size.to_le_bytes())?;
        writer.write_all(&sizes.bg_palette_offset().to_le_bytes())?;
    }

    Ok(())
}

fn write_end_block<W: Write>(writer: &mut W) -> Result<()> {
    write_block_header(writer, "END ", 0)
}

struct Sizes {
    ram_size: u32,
    vram_size: u32,
    mbc_ram_size: u32,
    oam_size: u32,
    hram_size: u32,
    bg_palette_size: u32,
}

impl Sizes {
    fn total(&self) -> u32 {
        self.ram_size
            + self.vram_size
            + self.mbc_ram_size
            + self.oam_size
            + self.hram_size
            + self.bg_palette_size
    }

    fn ram_offset(&self) -> u32 {
        0
    }

    fn vram_offset(&self) -> u32 {
        self.ram_size
    }

    fn mbc_ram_offset(&self) -> u32 {
        self.vram_offset() + self.vram_size
    }

    fn oam_offset(&self) -> u32 {
        self.mbc_ram_offset() + self.mbc_ram_size
    }

    fn hram_offset(&self) -> u32 {
        self.oam_offset() + self.oam_size
    }

    fn bg_palette_offset(&self) -> u32 {
        self.hram_offset() + self.hram_size
    }
}

pub fn save_state<C: AudioCallback, W: std::io::Write>(
    gb: &Gb<C>,
    writer: &mut W,
) -> std::io::Result<()> {
    let sizes = Sizes {
        ram_size: match gb.cgb_mode {
            CgbMode::Dmg | CgbMode::Compat => u32::from(crate::WRAM_SIZE_GB),
            CgbMode::Cgb => u32::from(crate::WRAM_SIZE_CGB),
        },
        vram_size: match gb.cgb_mode {
            CgbMode::Dmg | CgbMode::Compat => u32::from(VRAM_SIZE_GB),
            CgbMode::Cgb => u32::from(VRAM_SIZE_CGB),
        },
        mbc_ram_size: gb.cart.ram_size_bytes(),
        oam_size: OAM_SIZE as u32,
        hram_size: crate::HRAM_SIZE as u32,
        bg_palette_size: match gb.cgb_mode {
            CgbMode::Dmg | CgbMode::Compat => 0,
            CgbMode::Cgb => 0x40,
        },
    };

    // Write RAM
    writer.write_all(&gb.wram[..sizes.ram_size as usize])?;

    // Write VRAM
    writer.write_all(&gb.ppu.vram()[..sizes.vram_size as usize])?;

    // Write MBC RAM
    if let Some(mbc_ram) = gb.cart.mbc_ram() {
        writer.write_all(&mbc_ram[..sizes.mbc_ram_size as usize])?;
    }

    // Write OAM
    writer.write_all(&gb.ppu.oam()[..sizes.oam_size as usize])?;

    // Write HRAM
    writer.write_all(&gb.hram)?;

    // Write Background Palette
    if let CgbMode::Cgb = gb.cgb_mode {
        let (bcp, ocp) = gb.ppu.color_palette_buffers_bcp_ocp();
        writer.write_all(bcp)?;
        writer.write_all(ocp)?;
    }

    let offset_to_first_block = sizes.total();

    write_name_block(writer)?;
    write_info_block(writer, &gb.cart)?;
    write_core_block(gb, sizes, writer)?;
    write_end_block(writer)?;
    write_footer(writer, offset_to_first_block)?;

    Ok(())
}
