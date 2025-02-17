// Best Effort Save State (https://github.com/LIJI32/SameBoy/blob/master/BESS.md)
// Every integer is in little-endian byte order

use smallvec::SmallVec;

use crate::{
    ppu::{OAM_SIZE, VRAM_SIZE_CGB, VRAM_SIZE_GB},
    AudioCallback, Cart, CgbMode, Gb,
};
use std::io::Write;

fn write_footer<W: Write>(writer: &mut W, offset_to_first_block: u32) -> std::io::Result<()> {
    const LITERAL: &[u8] = b"BESS";

    writer.write_all(&offset_to_first_block.to_le_bytes())?;
    writer.write_all(LITERAL)?;
    Ok(())
}

fn write_block_header<W: Write>(writer: &mut W, name: &[u8; 4], size: u32) -> std::io::Result<()> {
    writer.write_all(name)?;
    writer.write_all(&size.to_le_bytes())?;
    Ok(())
}

fn write_name_block<W: Write>(writer: &mut W) -> std::io::Result<()> {
    const EMULATOR_NAME: &str = "Ceres, 0.1.0";

    write_block_header(writer, b"NAME", EMULATOR_NAME.len() as u32)?;
    writer.write_all(EMULATOR_NAME.as_bytes())?;
    Ok(())
}

fn write_info_block<W: Write>(writer: &mut W, cart: &Cart) -> std::io::Result<()> {
    const INFO_BLOCK_SIZE: u32 = 0x12;

    write_block_header(writer, b"INFO", INFO_BLOCK_SIZE)?;

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
) -> std::io::Result<()> {
    write_block_header(writer, b"CORE", 0xD0)?;

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
        writer.write_all(&gb.pc.to_le_bytes())?; // PC
        writer.write_all(&gb.af.to_le_bytes())?; // AF
        writer.write_all(&gb.bc.to_le_bytes())?; // BC
        writer.write_all(&gb.de.to_le_bytes())?; // DE
        writer.write_all(&gb.hl.to_le_bytes())?; // HL
        writer.write_all(&gb.sp.to_le_bytes())?; // SP
        writer.write_all(&[if gb.ints.are_enabled() { 1 } else { 0 }])?; // IME
        writer.write_all(&[gb.ints.read_ie()])?; // IE

        // Execution state (TODO: stopped state)
        let cpu_halted: u8 = if gb.cpu_halted { 1 } else { 0 };
        writer.write_all(&[cpu_halted])?;
        // Reserved byte, must be zero according to BESS specification
        writer.write_all(&[0])?;
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
        writer.write_all(&sizes.obj_palette_size.to_le_bytes())?;
        writer.write_all(&sizes.obj_palette_offset().to_le_bytes())?;
    }

    Ok(())
}

fn write_end_block<W: Write>(writer: &mut W) -> std::io::Result<()> {
    write_block_header(writer, b"END ", 0)
}

fn write_rtc_block<W: Write>(writer: &mut W, cart: &Cart) -> std::io::Result<()> {
    if let Some(rtc) = cart.rtc() {
        write_block_header(writer, b"RTC ", 0x28 + 0x8)?;

        // FIXME: this are "latched" values, not the actual values
        // Write seconds byte (0) and 3 bytes of padding
        writer.write_all(&[rtc.seconds(), 0, 0, 0])?;
        // Same for the rest
        writer.write_all(&[rtc.minutes(), 0, 0, 0])?;
        writer.write_all(&[rtc.hours(), 0, 0, 0])?;
        writer.write_all(&[rtc.days(), 0, 0, 0])?;
        writer.write_all(&[rtc.control(), 0, 0, 0])?;

        // FIXME: for now write the same values as the latched ones
        writer.write_all(&[rtc.seconds(), 0, 0, 0])?;
        writer.write_all(&[rtc.minutes(), 0, 0, 0])?;
        writer.write_all(&[rtc.hours(), 0, 0, 0])?;
        writer.write_all(&[rtc.days(), 0, 0, 0])?;
        writer.write_all(&[rtc.control(), 0, 0, 0])?;

        // Unix timestamp
        #[expect(clippy::unwrap_used)]
        {
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();

            writer.write_all(&timestamp.to_le_bytes())?;
        }
    }

    Ok(())
}

#[derive(Default)]
struct Sizes {
    ram_size: u32,
    vram_size: u32,
    mbc_ram_size: u32,
    oam_size: u32,
    hram_size: u32,
    bg_palette_size: u32,
    obj_palette_size: u32,
}

impl Sizes {
    // fn total(&self) -> u32 {
    //     self.ram_size
    //         + self.vram_size
    //         + self.mbc_ram_size
    //         + self.oam_size
    //         + self.hram_size
    //         + self.bg_palette_size
    //         + self.obj_palette_size
    // }

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

    fn obj_palette_offset(&self) -> u32 {
        self.bg_palette_offset() + self.bg_palette_size
    }
}

pub fn save_state<C: AudioCallback, W: Write + std::io::Seek>(
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
        obj_palette_size: match gb.cgb_mode {
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
        let dummy_palette = [0; 0x80];
        writer.write_all(&dummy_palette)?;
    }

    let offset_to_first_block = writer.stream_position()? as u32;

    // println!("Offset to first block: {}", offset_to_first_block);
    // println!("Total size: {}", sizes.total());

    write_name_block(writer)?;
    write_info_block(writer, &gb.cart)?;
    write_core_block(gb, sizes, writer)?;
    write_rtc_block(writer, &gb.cart)?;
    write_end_block(writer)?;
    write_footer(writer, offset_to_first_block)?;

    Ok(())
}

fn read_footer<R: std::io::Read + std::io::Seek>(reader: &mut R) -> std::io::Result<u32> {
    let mut footer = [0; 8];
    reader.seek(std::io::SeekFrom::End(-8))?;
    reader.read_exact(&mut footer)?;
    // Check for BESS magic
    if &footer[4..] != b"BESS" {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Invalid BESS footer",
        ));
    }

    #[expect(
        clippy::unwrap_used,
        reason = "footer is 4 bytes long, so this will never panic"
    )]
    {
        // Read offset to first block
        Ok(u32::from_le_bytes(footer[0..4].try_into().unwrap()))
    }
}

fn read_block_header<R: std::io::Read>(
    reader: &mut R,
) -> std::io::Result<(SmallVec<[u8; 4]>, u32)> {
    let mut header = [0; 8];
    reader.read_exact(&mut header)?;

    #[expect(
        clippy::unwrap_used,
        reason = "header is 8 bytes long, so this will never panic"
    )]
    {
        let name = SmallVec::from_slice(&header[0..4]);
        let size = u32::from_le_bytes(header[4..].try_into().unwrap());

        // println!("Block: {}, size: {}", String::from_utf8_lossy(&name), size);

        Ok((name, size))
    }
}

fn read_name_block<R: std::io::Read + std::io::Seek>(
    reader: &mut R,
    size: u32,
) -> std::io::Result<()> {
    // Ignore for now
    reader.seek(std::io::SeekFrom::Current(i64::from(size)))?;
    Ok(())
}

fn read_info_block<R: std::io::Read>(
    reader: &mut R,
    size: u32,
) -> std::io::Result<(SmallVec<[u8; 4]>, u16)> {
    if size != 0x12 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Invalid INFO block size",
        ));
    }

    let mut title = [0; 0x10];
    reader.read_exact(&mut title)?;

    // Read global checksum
    let mut global_checksum = [0; 2];
    reader.read_exact(&mut global_checksum)?;

    let title = SmallVec::from_slice(&title);
    let global_checksum = u16::from_le_bytes(global_checksum);

    Ok((title, global_checksum))
}

fn read_core_block<C: AudioCallback, R: std::io::Read + std::io::Seek>(
    reader: &mut R,
    _size: u32,
    gb: &mut Gb<C>,
) -> std::io::Result<Sizes> {
    // Ignore version for now
    reader.seek(std::io::SeekFrom::Current(4))?;

    // Read model
    let mut model = [0; 4];
    reader.read_exact(&mut model)?;

    gb.cgb_mode = match &model {
        b"GD  " => CgbMode::Dmg,
        b"GM  " => CgbMode::Compat,
        b"CC  " => CgbMode::Cgb,
        _ => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid model",
            ))
        }
    };

    // Ignore CPU registers for now
    // FIXME: is this offset correct?
    reader.seek(std::io::SeekFrom::Current(0x91))?;

    // Read sizes into sizes struct
    let mut sizes = Sizes::default();

    let mut size_buf = [0; 4];

    reader.read_exact(&mut size_buf)?;
    sizes.ram_size = u32::from_le_bytes(size_buf);

    // FIXME: discard offsets for now
    reader.seek(std::io::SeekFrom::Current(4))?;

    reader.read_exact(&mut size_buf)?;
    sizes.vram_size = u32::from_le_bytes(size_buf);

    // FIXME: discard offsets for now
    reader.seek(std::io::SeekFrom::Current(4))?;

    reader.read_exact(&mut size_buf)?;
    sizes.mbc_ram_size = u32::from_le_bytes(size_buf);

    // FIXME: discard offsets for now
    reader.seek(std::io::SeekFrom::Current(4))?;

    reader.read_exact(&mut size_buf)?;
    sizes.oam_size = u32::from_le_bytes(size_buf);

    // FIXME: discard offsets for now
    reader.seek(std::io::SeekFrom::Current(4))?;

    reader.read_exact(&mut size_buf)?;
    sizes.hram_size = u32::from_le_bytes(size_buf);

    // FIXME: discard offsets for now
    reader.seek(std::io::SeekFrom::Current(4))?;

    reader.read_exact(&mut size_buf)?;
    sizes.bg_palette_size = u32::from_le_bytes(size_buf);

    // FIXME: discard offsets for now
    reader.seek(std::io::SeekFrom::Current(4))?;

    reader.read_exact(&mut size_buf)?;
    sizes.obj_palette_size = u32::from_le_bytes(size_buf);

    // FIXME: discard offsets for now
    reader.seek(std::io::SeekFrom::Current(4))?;

    Ok(sizes)
}

fn read_rtc_block<R: std::io::Read + std::io::Seek>(
    reader: &mut R,
    cart: &mut Cart,
) -> std::io::Result<()> {
    if let Some(rtc) = cart.rtc_mut() {
        let mut byte_buf = [0; 4];

        reader.read_exact(&mut byte_buf)?;
        rtc.set_seconds(byte_buf[0]);

        reader.read_exact(&mut byte_buf)?;
        rtc.set_minutes(byte_buf[0]);

        reader.read_exact(&mut byte_buf)?;
        rtc.set_hours(byte_buf[0]);

        reader.read_exact(&mut byte_buf)?;
        rtc.set_days(byte_buf[0]);

        reader.read_exact(&mut byte_buf)?;
        rtc.set_control(byte_buf[0]);

        // Skip latched values
        reader.read_exact(&mut byte_buf)?;
        reader.read_exact(&mut byte_buf)?;
        reader.read_exact(&mut byte_buf)?;
        reader.read_exact(&mut byte_buf)?;
        reader.read_exact(&mut byte_buf)?;

        // Seconds since saved timestamp
        #[expect(clippy::unwrap_used)]
        {
            let mut timestamp_buf = [0; 8];
            reader.read_exact(&mut timestamp_buf)?;

            let timestamp = u64::from_le_bytes(timestamp_buf);
            let elapsed = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
                - timestamp;

            rtc.add_seconds(elapsed);
        }
    }

    Ok(())
}

pub fn load_state<C: AudioCallback, R: std::io::Read + std::io::Seek>(
    gb: &mut Gb<C>,
    reader: &mut R,
) -> std::io::Result<()> {
    let offset_to_first_block = read_footer(reader)?;

    // Read blocks
    reader.seek(std::io::SeekFrom::Start(u64::from(offset_to_first_block)))?;

    let mut sizes = Sizes::default();

    'reading: loop {
        let (name, size) = read_block_header(reader)?;

        match name.as_ref() {
            b"NAME" => read_name_block(reader, size)?,
            b"INFO" => {
                read_info_block(reader, size)?;
            }
            b"CORE" => sizes = read_core_block(reader, size, gb)?,
            b"RTC " => read_rtc_block(reader, &mut gb.cart)?,
            b"END " => break 'reading,
            _ => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Illegal block",
                ));
            }
        }
    }

    // Read data
    reader.seek(std::io::SeekFrom::Start(sizes.ram_offset() as u64))?;
    reader.read_exact(gb.wram.as_mut_slice())?;

    reader.seek(std::io::SeekFrom::Start(sizes.vram_offset() as u64))?;
    reader.read_exact(gb.ppu.vram_mut())?;

    if let Some(mbc_ram) = gb.cart.mbc_ram_mut() {
        reader.seek(std::io::SeekFrom::Start(sizes.mbc_ram_offset() as u64))?;
        reader.read_exact(mbc_ram)?;
    }

    reader.seek(std::io::SeekFrom::Start(sizes.oam_offset() as u64))?;
    reader.read_exact(gb.ppu.oam_mut())?;

    reader.seek(std::io::SeekFrom::Start(sizes.hram_offset() as u64))?;
    reader.read_exact(&mut gb.hram)?;

    let skip_palette = if let CgbMode::Cgb = gb.cgb_mode {
        sizes.bg_palette_offset() + sizes.bg_palette_size
    } else {
        sizes.bg_palette_offset()
    };

    reader.seek(std::io::SeekFrom::Start(skip_palette as u64))?;

    Ok(())
}
