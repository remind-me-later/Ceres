use crate::{AudioCallback, Cartridge, CgbMode, Gb, error::Error};

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

        // Read model and ignore for now
        let mut model = [0; 4];
        self.read_exact(&mut model)?;

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

            // FIXME: we don't emulate latched values on MBC3 RTC
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
