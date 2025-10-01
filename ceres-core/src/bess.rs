// Best Effort Save State (https://github.com/LIJI32/SameBoy/blob/master/BESS.md)
// Every integer is in little-endian byte order

use alloc::vec::Vec;

use crate::{
    AudioCallback, Cartridge, CgbMode, Gb,
    error::Error,
    memory::{Hram, Wram},
    ppu::{Oam, Vram},
};

#[derive(Default)]
struct CreatedSizes {
    bg_palette: u32,
    hram: u32,
    mbc_ram: u32,
    oam: u32,
    obj_palette: u32,
    ram: u32,
    vram: u32,
}

impl CreatedSizes {
    // fn total(&self) -> u32 {
    //     self.ram_size
    //         + self.vram_size
    //         + self.mbc_ram_size
    //         + self.oam_size
    //         + self.hram_size
    //         + self.bg_palette_size
    //         + self.obj_palette_size
    // }

    const fn bg_palette_offset(&self) -> u32 {
        self.hram_offset() + self.hram
    }

    const fn hram_offset(&self) -> u32 {
        self.oam_offset() + self.oam
    }

    const fn mbc_ram_offset(&self) -> u32 {
        self.vram_offset() + self.vram
    }

    const fn oam_offset(&self) -> u32 {
        self.mbc_ram_offset() + self.mbc_ram
    }

    const fn obj_palette_offset(&self) -> u32 {
        self.bg_palette_offset() + self.bg_palette
    }

    #[expect(clippy::unused_self)]
    const fn ram_offset(&self) -> u32 {
        0
    }

    const fn vram_offset(&self) -> u32 {
        self.ram
    }
}

pub struct Reader<'a> {
    data: &'a [u8],
    position: usize,
}

impl<'a> Reader<'a> {
    pub fn load_state<A: AudioCallback>(
        &mut self,
        gb: &mut Gb<A>,
        secs_since_unix_epoch: u64,
    ) -> Result<(), Error> {
        let offset_to_first_block = self.read_footer()?;

        // Read blocks
        self.seek_from_start(offset_to_first_block as usize)?;

        let mut sizes = ReadSizes::default();

        'reading: loop {
            let (name, size) = self.read_block_header()?;

            match name.as_ref() {
                b"NAME" => self.read_name_block(size)?,
                b"INFO" => {
                    self.read_info_block(size)?;
                }
                b"CORE" => sizes = self.read_core_block(size, gb)?,
                b"RTC " => self.read_rtc_block(secs_since_unix_epoch, &mut gb.cart)?,
                b"END " => break 'reading,
                _ => {
                    return Err(Error::InvalidSaveState);
                }
            }
        }

        // Read data
        self.seek_from_start(sizes.ram_offset() as usize)?;
        self.read_exact(gb.wram.wram_mut())?;

        self.seek_from_start(sizes.vram_offset() as usize)?;
        self.read_exact(gb.ppu.vram_mut().bytes_mut())?;

        if let Some(mbc_ram) = gb.cart.mbc_ram_mut() {
            // FIXME: use crate::Error to indicate ram size is not what expected from header
            self.seek_from_start(sizes.mbc_ram_offset() as usize)?;
            self.read_exact(mbc_ram)?;
        }

        self.seek_from_start(sizes.oam_offset() as usize)?;
        self.read_exact(gb.ppu.oam_mut().bytes_mut())?;

        self.seek_from_start(sizes.hram_offset() as usize)?;
        self.read_exact(gb.hram.hram_mut())?;

        let skip_palette = if matches!(gb.cgb_mode, CgbMode::Cgb) {
            sizes.bg_palette_offset() + sizes.bg_palette_size
        } else {
            sizes.bg_palette_offset()
        };

        self.seek_from_start(skip_palette as usize)?;

        Ok(())
    }

    pub const fn new(data: &'a [u8]) -> Self {
        Self { data, position: 0 }
    }

    fn read_block_header(&mut self) -> Result<([u8; 4], u32), Error> {
        let mut header = [0; 8];
        self.read_exact(&mut header)?;

        #[expect(
            clippy::unwrap_used,
            reason = "header is 8 bytes long, so this will never panic"
        )]
        {
            let name = &header[0..4];
            let size = u32::from_le_bytes(header[4..].try_into().unwrap());

            // println!("Block: {}, size: {}", String::from_utf8_lossy(&name), size);

            Ok((name.try_into().unwrap(), size))
        }
    }

    fn read_core_block<A: AudioCallback>(
        &mut self,
        _size: u32,
        _gb: &mut Gb<A>,
    ) -> Result<ReadSizes, Error> {
        // Ignore version for now
        self.seek_from_current(4)?;

        // Read model
        let mut model = [0; 4];
        self.read_exact(&mut model)?;

        // gb.cgb_mode = match &model {
        //     b"GD  " => CgbMode::Dmg,
        //     b"GM  " => CgbMode::Compat,
        //     b"CC  " => CgbMode::Cgb,
        //     _ => {
        //         return Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid model"));
        //     }
        // };

        // Ignore CPU registers for now
        self.seek_from_current(0x90)?;

        // Read sizes into sizes struct
        let mut sizes = ReadSizes::default();

        let mut size_buf = [0; 4];

        self.read_exact(&mut size_buf)?;
        sizes.ram_size = u32::from_le_bytes(size_buf);

        self.read_exact(&mut size_buf)?;
        sizes.ram_offset = u32::from_le_bytes(size_buf);

        self.read_exact(&mut size_buf)?;
        sizes.vram_size = u32::from_le_bytes(size_buf);

        self.read_exact(&mut size_buf)?;
        sizes.vram_offset = u32::from_le_bytes(size_buf);

        self.read_exact(&mut size_buf)?;
        sizes.mbc_ram_size = u32::from_le_bytes(size_buf);

        self.read_exact(&mut size_buf)?;
        sizes.mbc_ram_offset = u32::from_le_bytes(size_buf);

        self.read_exact(&mut size_buf)?;
        sizes.oam_size = u32::from_le_bytes(size_buf);

        self.read_exact(&mut size_buf)?;
        sizes.oam_offset = u32::from_le_bytes(size_buf);

        self.read_exact(&mut size_buf)?;
        sizes.hram_size = u32::from_le_bytes(size_buf);

        self.read_exact(&mut size_buf)?;
        sizes.hram_offset = u32::from_le_bytes(size_buf);

        self.read_exact(&mut size_buf)?;
        sizes.bg_palette_size = u32::from_le_bytes(size_buf);

        self.read_exact(&mut size_buf)?;
        sizes.bg_palette_offset = u32::from_le_bytes(size_buf);

        self.read_exact(&mut size_buf)?;
        sizes.obj_palette_size = u32::from_le_bytes(size_buf);

        self.read_exact(&mut size_buf)?;
        sizes.obj_palette_offset = u32::from_le_bytes(size_buf);

        Ok(sizes)
    }

    fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), Error> {
        if self.position + buf.len() > self.data.len() {
            return Err(Error::InvalidSaveState);
        }

        buf.copy_from_slice(&self.data[self.position..self.position + buf.len()]);
        self.position += buf.len();
        Ok(())
    }

    fn read_footer(&mut self) -> Result<u32, Error> {
        let mut footer = [0; 8];
        self.seek_from_end(8)?;
        self.read_exact(&mut footer)?;
        // Check for BESS magic
        if &footer[4..] != b"BESS" {
            return Err(Error::InvalidSaveState);
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

    fn read_info_block(&mut self, size: u32) -> Result<([u8; 0x10], u16), Error> {
        if size != 0x12 {
            return Err(Error::InvalidSaveState);
        }

        let mut title = [0; 0x10];
        self.read_exact(&mut title)?;

        // Read global checksum
        let mut global_checksum = [0; 2];
        self.read_exact(&mut global_checksum)?;

        let global_checksum = u16::from_le_bytes(global_checksum);

        Ok((title, global_checksum))
    }

    fn read_name_block(&mut self, size: u32) -> Result<(), Error> {
        // Ignore for now
        self.seek_from_current(size as usize)?;
        Ok(())
    }

    fn read_rtc_block(
        &mut self,
        secs_since_unix_epoch: u64,
        cart: &mut Cartridge,
    ) -> Result<(), Error> {
        if let Some(rtc) = cart.rtc_mut() {
            let mut byte_buf = [0; 4];

            self.read_exact(&mut byte_buf)?;
            rtc.set_seconds(byte_buf[0]);

            self.read_exact(&mut byte_buf)?;
            rtc.set_minutes(byte_buf[0]);

            self.read_exact(&mut byte_buf)?;
            rtc.set_hours(byte_buf[0]);

            self.read_exact(&mut byte_buf)?;
            rtc.set_days(byte_buf[0]);

            self.read_exact(&mut byte_buf)?;
            rtc.set_control(byte_buf[0]);

            // Skip latched values
            self.read_exact(&mut byte_buf)?;
            self.read_exact(&mut byte_buf)?;
            self.read_exact(&mut byte_buf)?;
            self.read_exact(&mut byte_buf)?;
            self.read_exact(&mut byte_buf)?;

            // Seconds since saved timestamp
            {
                let mut timestamp_buf = [0; 8];
                self.read_exact(&mut timestamp_buf)?;

                let timestamp = u64::from_le_bytes(timestamp_buf);
                let elapsed = secs_since_unix_epoch - timestamp;

                rtc.add_seconds(elapsed);
            }
        }

        Ok(())
    }

    const fn seek_from_current(&mut self, n: usize) -> Result<(), Error> {
        if self.position + n > self.data.len() {
            return Err(Error::InvalidSaveState);
        }

        self.position += n;
        Ok(())
    }

    const fn seek_from_end(&mut self, n: usize) -> Result<(), Error> {
        if n > self.data.len() {
            return Err(Error::InvalidSaveState);
        }

        self.position = self.data.len() - n;
        Ok(())
    }

    const fn seek_from_start(&mut self, n: usize) -> Result<(), Error> {
        if n > self.data.len() {
            return Err(Error::InvalidSaveState);
        }

        self.position = n;
        Ok(())
    }
}

#[derive(Default)]
struct ReadSizes {
    bg_palette_offset: u32,
    bg_palette_size: u32,
    hram_offset: u32,
    hram_size: u32,
    mbc_ram_offset: u32,
    mbc_ram_size: u32,
    oam_offset: u32,
    oam_size: u32,
    obj_palette_offset: u32,
    obj_palette_size: u32,
    ram_offset: u32,
    ram_size: u32,
    vram_offset: u32,
    vram_size: u32,
}

impl ReadSizes {
    const fn bg_palette_offset(&self) -> u32 {
        self.bg_palette_offset
    }

    const fn hram_offset(&self) -> u32 {
        self.hram_offset
    }

    const fn mbc_ram_offset(&self) -> u32 {
        self.mbc_ram_offset
    }

    const fn oam_offset(&self) -> u32 {
        self.oam_offset
    }

    const fn ram_offset(&self) -> u32 {
        self.ram_offset
    }

    const fn vram_offset(&self) -> u32 {
        self.vram_offset
    }

    // fn obj_palette_offset(&self) -> u32 {
    //     self.obj_palette_offset
    // }
}

pub struct Writer<'a> {
    buf: &'a mut Vec<u8>,
    position: usize,
}

impl<'a> Writer<'a> {
    pub const fn new(buf: &'a mut Vec<u8>) -> Self {
        Self { buf, position: 0 }
    }

    pub fn save_state<A: AudioCallback>(&mut self, gb: &Gb<A>, secs_since_unix_epoch: u64) {
        let sizes = CreatedSizes {
            ram: match gb.cgb_mode {
                CgbMode::Dmg | CgbMode::Compat => u32::from(Wram::SIZE_GB),
                CgbMode::Cgb => u32::from(Wram::SIZE_CGB),
            },
            vram: match gb.cgb_mode {
                CgbMode::Dmg | CgbMode::Compat => u32::from(Vram::SIZE_GB),
                CgbMode::Cgb => u32::from(Vram::SIZE_CGB),
            },
            mbc_ram: gb.cart.ram_size_bytes(),
            oam: u32::from(Oam::SIZE),
            hram: u32::from(Hram::SIZE),
            bg_palette: match gb.cgb_mode {
                CgbMode::Dmg | CgbMode::Compat => 0,
                CgbMode::Cgb => 0x40,
            },
            obj_palette: match gb.cgb_mode {
                CgbMode::Dmg | CgbMode::Compat => 0,
                CgbMode::Cgb => 0x40,
            },
        };

        // Write RAM
        self.write_all(&gb.wram.wram()[..sizes.ram as usize]);

        // Write VRAM
        self.write_all(&gb.ppu.vram().bytes()[..sizes.vram as usize]);

        // Write MBC RAM
        if let Some(mbc_ram) = gb.cart.mbc_ram() {
            self.write_all(&mbc_ram[..sizes.mbc_ram as usize]);
        }

        // Write OAM
        self.write_all(&gb.ppu.oam().bytes()[..sizes.oam as usize]);

        // Write HRAM
        self.write_all(gb.hram.hram().as_slice());

        // Write Background Palette
        if matches!(gb.cgb_mode, CgbMode::Cgb) {
            let dummy_palette = [0; 0x80];
            self.write_all(&dummy_palette);
        }
        #[expect(clippy::cast_possible_truncation)]
        let offset_to_first_block = { self.position as u32 };

        // println!("Offset to first block: {}", offset_to_first_block);
        // println!("Total size: {}", sizes.total());

        self.write_name_block();
        self.write_info_block(&gb.cart);
        self.write_core_block(gb, &sizes);
        self.write_rtc_block(secs_since_unix_epoch, &gb.cart);
        self.write_end_block();
        self.write_footer(offset_to_first_block);
    }

    fn write_all(&mut self, buf: &[u8]) {
        self.buf.extend_from_slice(buf);
        self.position += buf.len();
    }

    fn write_block_header(&mut self, name: [u8; 4], size: u32) {
        self.write_all(&name);
        self.write_all(&size.to_le_bytes());
    }

    fn write_core_block<A: AudioCallback>(&mut self, gb: &Gb<A>, sizes: &CreatedSizes) {
        self.write_block_header(*b"CORE", 0xD0);

        // BESS Version
        {
            const MAJOR_VERSION: u16 = 1;
            const MINOR_VERSION: u16 = 1;

            self.write_all(&MAJOR_VERSION.to_le_bytes());
            self.write_all(&MINOR_VERSION.to_le_bytes());
        }

        // Model
        {
            let model = match gb.model {
                crate::Model::Dmg => "GD  ",
                crate::Model::Mgb => "GM  ",
                crate::Model::Cgb => "CC  ",
            };

            self.write_all(model.as_bytes());
        }

        // CPU Registers
        {
            self.write_all(&gb.cpu.pc().to_le_bytes()); // PC
            self.write_all(&gb.cpu.af().to_le_bytes()); // AF
            self.write_all(&gb.cpu.bc().to_le_bytes()); // BC
            self.write_all(&gb.cpu.de().to_le_bytes()); // DE
            self.write_all(&gb.cpu.hl().to_le_bytes()); // HL
            self.write_all(&gb.cpu.sp().to_le_bytes()); // SP
            self.write_all(&[u8::from(gb.ints.are_enabled())]); // IME
            self.write_all(&[gb.ints.read_ie()]); // IE

            // Execution state (TODO: stopped state)
            self.write_all(&[u8::from(gb.cpu.is_halted())]);
            // Reserved byte, must be zero according to BESS specification
            self.write_all(&[0]);

            // Every memory mapped register
            for i in 0xFF00..0xFF80 {
                self.write_all(&[gb.read_mem(i)]);
            }
        }

        // Sizes
        {
            self.write_all(&sizes.ram.to_le_bytes());
            self.write_all(&sizes.ram_offset().to_le_bytes());
            self.write_all(&sizes.vram.to_le_bytes());
            self.write_all(&sizes.vram_offset().to_le_bytes());
            self.write_all(&sizes.mbc_ram.to_le_bytes());
            self.write_all(&sizes.mbc_ram_offset().to_le_bytes());
            self.write_all(&sizes.oam.to_le_bytes());
            self.write_all(&sizes.oam_offset().to_le_bytes());
            self.write_all(&sizes.hram.to_le_bytes());
            self.write_all(&sizes.hram_offset().to_le_bytes());
            self.write_all(&sizes.bg_palette.to_le_bytes());
            self.write_all(&sizes.bg_palette_offset().to_le_bytes());
            self.write_all(&sizes.obj_palette.to_le_bytes());
            self.write_all(&sizes.obj_palette_offset().to_le_bytes());
        }
    }

    fn write_end_block(&mut self) {
        self.write_block_header(*b"END ", 0);
    }

    fn write_footer(&mut self, offset_to_first_block: u32) {
        const LITERAL: &[u8] = b"BESS";

        self.write_all(&offset_to_first_block.to_le_bytes());
        self.write_all(LITERAL);
    }

    fn write_info_block(&mut self, cart: &Cartridge) {
        const INFO_BLOCK_SIZE: u32 = 0x12;

        self.write_block_header(*b"INFO", INFO_BLOCK_SIZE);

        // pad title to 0x10 bytes
        let mut title = [0; 0x10];
        let title_bytes = cart.ascii_title();
        let title_len = title_bytes.len();
        title[0..title_len].copy_from_slice(title_bytes);

        self.write_all(&title);
        self.write_all(&cart.global_checksum().to_le_bytes());
    }

    fn write_name_block(&mut self) {
        const EMULATOR_NAME: &str = "Ceres, 0.1.0";

        #[expect(clippy::cast_possible_truncation)]
        self.write_block_header(*b"NAME", EMULATOR_NAME.len() as u32);
        self.write_all(EMULATOR_NAME.as_bytes());
    }

    fn write_rtc_block(&mut self, secs_since_unix_epoch: u64, cart: &Cartridge) {
        if let Some(rtc) = cart.rtc() {
            self.write_block_header(*b"RTC ", 0x28 + 0x8);

            // FIXME: this are "latched" values, not the actual values
            // Write seconds byte (0) and 3 bytes of padding
            self.write_all(&[rtc.seconds(), 0, 0, 0]);
            // Same for the rest
            self.write_all(&[rtc.minutes(), 0, 0, 0]);
            self.write_all(&[rtc.hours(), 0, 0, 0]);
            self.write_all(&[rtc.days(), 0, 0, 0]);
            self.write_all(&[rtc.control(), 0, 0, 0]);

            // FIXME: for now write the same values as the latched ones
            self.write_all(&[rtc.seconds(), 0, 0, 0]);
            self.write_all(&[rtc.minutes(), 0, 0, 0]);
            self.write_all(&[rtc.hours(), 0, 0, 0]);
            self.write_all(&[rtc.days(), 0, 0, 0]);
            self.write_all(&[rtc.control(), 0, 0, 0]);

            {
                let timestamp = secs_since_unix_epoch;
                self.write_all(&timestamp.to_le_bytes());
            }
        }
    }
}
