use {
    super::{CORE_REGISTERS_SIZE, FOOTER_SIZE, INFO_BLOCK_SIZE},
    crate::{AudioCallback, Cartridge, CgbMode, Gb, error::Error},
};

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

            match &name {
                // Ignore the emulator name for now
                b"NAME" => self.seek_from_current(size as usize)?,
                b"INFO" => self.read_info_block(size)?,
                b"CORE" => sizes = self.read_core_block()?,
                b"RTC " => self.read_rtc_block(size, secs_since_unix_epoch, &mut gb.cart)?,
                b"END " => break 'reading,
                _ => return Err(Error::InvalidSaveState),
            }
        }

        // Read data
        self.seek_from_start(sizes.ram.offset as usize)?;
        self.read_exact(gb.wram.wram_mut())?;

        self.seek_from_start(sizes.vram.offset as usize)?;
        self.read_exact(gb.ppu.vram_mut().bytes_mut())?;

        if let Some(mbc_ram) = gb.cart.mbc_ram_mut() {
            // FIXME: use crate::Error to indicate ram size is not what expected from header
            self.seek_from_start(sizes.mbc_ram.offset as usize)?;
            self.read_exact(mbc_ram)?;
        }

        self.seek_from_start(sizes.oam.offset as usize)?;
        self.read_exact(gb.ppu.oam_mut().bytes_mut())?;

        self.seek_from_start(sizes.hram.offset as usize)?;
        self.read_exact(gb.hram.hram_mut())?;

        let skip_palette = if matches!(gb.cgb_mode, CgbMode::Cgb) {
            sizes.bg_palette.offset + sizes.bg_palette.size
        } else {
            sizes.bg_palette.offset
        };

        self.seek_from_start(skip_palette as usize)?;

        Ok(())
    }

    pub const fn new(data: &'a [u8]) -> Self {
        Self { data, position: 0 }
    }

    fn read_block_header(&mut self) -> Result<([u8; 4], u32), Error> {
        let mut name = [0; 4];
        self.read_exact(&mut name)?;
        Ok((name, self.read_u32()?))
    }

    fn read_core_block(&mut self) -> Result<ReadSizes, Error> {
        // Ignore the version, the model and the CPU registers for now
        self.seek_from_current(4 + 4 + CORE_REGISTERS_SIZE)?;

        let sizes = ReadSizes {
            ram: self.read_buffer()?,
            vram: self.read_buffer()?,
            mbc_ram: self.read_buffer()?,
            oam: self.read_buffer()?,
            hram: self.read_buffer()?,
            bg_palette: self.read_buffer()?,
        };
        // Ignore the object palettes for now
        self.read_buffer()?;

        Ok(sizes)
    }

    fn read_buffer(&mut self) -> Result<Buffer, Error> {
        Ok(Buffer {
            size: self.read_u32()?,
            offset: self.read_u32()?,
        })
    }

    fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), Error> {
        let end = self.position + buf.len();
        let src = self
            .data
            .get(self.position..end)
            .ok_or(Error::InvalidSaveState)?;
        buf.copy_from_slice(src);
        self.position = end;
        Ok(())
    }

    fn read_u32(&mut self) -> Result<u32, Error> {
        let mut buf = [0; 4];
        self.read_exact(&mut buf)?;
        Ok(u32::from_le_bytes(buf))
    }

    fn read_footer(&mut self) -> Result<u32, Error> {
        self.seek_from_end(FOOTER_SIZE)?;
        let offset_to_first_block = self.read_u32()?;
        let mut magic = [0; 4];
        self.read_exact(&mut magic)?;
        if &magic != b"BESS" {
            return Err(Error::InvalidSaveState);
        }
        Ok(offset_to_first_block)
    }

    const fn read_info_block(&mut self, size: u32) -> Result<(), Error> {
        if size != INFO_BLOCK_SIZE {
            return Err(Error::InvalidSaveState);
        }
        // Ignore the title and the global checksum for now
        self.seek_from_current(INFO_BLOCK_SIZE as usize)
    }

    fn read_rtc_block(
        &mut self,
        size: u32,
        secs_since_unix_epoch: u64,
        cart: &mut Cartridge,
    ) -> Result<(), Error> {
        let Some(rtc) = cart.rtc_mut() else {
            // A cartridge without a clock: skip the block.
            return self.seek_from_current(size as usize);
        };

        // Each register is a byte and 3 bytes of padding: the real
        // registers, then the latched ones.
        let mut regs = [[0; 5]; 2];
        for reg in regs.as_flattened_mut() {
            *reg = self.read_u32()?.to_le_bytes()[0];
        }
        let [real, latched] = regs;
        rtc.set_real(real);
        rtc.set_latched(latched);

        let mut timestamp = [0; 8];
        self.read_exact(&mut timestamp)?;
        // A timestamp from the future (a clock that went back) counts as none.
        let elapsed = secs_since_unix_epoch.saturating_sub(u64::from_le_bytes(timestamp));
        rtc.add_seconds(elapsed);

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

/// A memory in the save state.
#[derive(Default)]
struct Buffer {
    size: u32,
    offset: u32,
}

/// Where the CORE block says the memories are.
#[derive(Default)]
struct ReadSizes {
    ram: Buffer,
    vram: Buffer,
    mbc_ram: Buffer,
    oam: Buffer,
    hram: Buffer,
    bg_palette: Buffer,
}
