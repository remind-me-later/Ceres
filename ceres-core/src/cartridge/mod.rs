use {
    crate::Error,
    alloc::boxed::Box,
    mbc::{Mbc, Mbc3RTC},
    ram_size::RAMSize,
    rom_size::ROMSize,
};

mod mbc;
mod ram_size;
mod rom_size;

#[derive(Debug)]
pub struct Cartridge {
    mbc: Mbc,

    rom: Box<[u8]>,
    ram: Box<[u8]>,

    rom_bank_lo: u8,
    rom_bank_hi: u8,
    rom_offsets: (u32, u32),

    ram_enabled: bool,
    ram_bank: u8,
    ram_offset: u32,

    has_battery: bool,

    ram_size: RAMSize,
    rom_size: ROMSize,
}

impl Default for Cartridge {
    #[expect(
        clippy::unwrap_used,
        reason = "ROMSize::new and RAMSize::new are safe to unwrap"
    )]
    fn default() -> Self {
        let rom_size = ROMSize::new(0).unwrap();
        let ram_size = RAMSize::new(0).unwrap();
        let (mbc, has_battery) = Mbc::mbc_and_battery(0, rom_size).unwrap();

        let rom = alloc::vec![0xFF; rom_size.size_bytes() as usize].into_boxed_slice();
        let ram = alloc::vec![0xFF; ram_size.size_bytes() as usize].into_boxed_slice();

        Self {
            mbc,
            rom,
            ram,
            rom_bank_lo: 1,
            rom_bank_hi: 0,
            rom_offsets: (0, u32::from(ROMSize::BANK_SIZE)),
            ram_size,
            rom_size,
            ram_enabled: false,
            ram_bank: 0,
            ram_offset: 0,
            has_battery,
        }
    }
}

impl Cartridge {
    pub fn new(rom: Box<[u8]>) -> Result<Self, Error> {
        let rom_size = ROMSize::new(rom[0x148])?;
        let ram_size = RAMSize::new(rom[0x149])?;
        let (mbc, has_battery) = Mbc::mbc_and_battery(rom[0x147], rom_size)?;

        if rom_size.size_bytes() as usize != rom.len() {
            return Err(Error::RomSizeDifferentThanActual {
                expected: rom_size.size_bytes() as usize,
                actual: rom.len(),
            });
        }

        let ram = alloc::vec![0xFF; ram_size.size_bytes() as usize].into_boxed_slice();

        Ok(Self {
            mbc,
            rom,
            ram,
            rom_bank_lo: 1,
            rom_bank_hi: 0,
            rom_offsets: (0, u32::from(ROMSize::BANK_SIZE)),
            ram_size,
            rom_size,
            ram_enabled: false,
            ram_bank: 0,
            ram_offset: 0,
            has_battery,
        })
    }

    #[must_use]
    pub const fn is_old_licensee_code(&self) -> bool {
        let code = self.rom[0x14B];
        code != 0x33
    }

    #[must_use]
    pub fn ascii_title(&self) -> &[u8] {
        let range = if self.is_old_licensee_code() {
            0x134..0x144
        } else {
            0x134..0x13F
        };

        let title = &self.rom[range];
        let mut i = 0;
        while i < title.len() && title[i] != 0 {
            i += 1;
        }
        &title[..i]
    }

    #[must_use]
    pub const fn header_checksum(&self) -> u8 {
        self.rom[0x14D]
    }

    #[must_use]
    pub const fn global_checksum(&self) -> u16 {
        u16::from_le_bytes([self.rom[0x14F], self.rom[0x14E]])
    }

    #[must_use]
    pub const fn version(&self) -> u8 {
        self.rom[0x14C]
    }

    #[must_use]
    pub fn mbc_ram(&self) -> Option<&[u8]> {
        self.has_battery.then_some(&*self.ram)
    }

    #[must_use]
    pub fn mbc_ram_mut(&mut self) -> Option<&mut [u8]> {
        self.has_battery.then_some(&mut *self.ram)
    }

    #[must_use]
    pub const fn rtc(&self) -> Option<&Mbc3RTC> {
        if let Mbc::Mbc3 { rtc: Some(rtc), .. } = &self.mbc {
            Some(rtc)
        } else {
            None
        }
    }

    #[must_use]
    pub const fn rtc_mut(&mut self) -> Option<&mut Mbc3RTC> {
        if let Mbc::Mbc3 { rtc, .. } = &mut self.mbc {
            rtc.as_mut()
        } else {
            None
        }
    }

    #[must_use]
    pub const fn has_battery(&self) -> bool {
        self.has_battery
    }

    #[must_use]
    pub const fn ram_size_bytes(&self) -> u32 {
        self.ram_size.size_bytes()
    }

    pub const fn run_rtc(&mut self, cycles: i32) {
        if let Mbc::Mbc3 { rtc: Some(rtc), .. } = &mut self.mbc {
            rtc.run_cycles(cycles);
        }
    }

    #[must_use]
    pub const fn read_rom(&self, addr: u16) -> u8 {
        let (lo, hi) = self.rom_offsets;

        let bank_addr = match addr {
            0x0000..=0x3FFF => lo | (addr & 0x3FFF) as u32,
            0x4000..=0x7FFF => hi | (addr & 0x3FFF) as u32,
            _ => unreachable!(),
        };

        self.rom[bank_addr as usize]
    }

    #[must_use]
    pub fn read_ram(&self, addr: u16) -> u8 {
        const fn mbc_read_ram(cart: &Cartridge, ram_enabled: bool, addr: u16) -> u8 {
            if cart.ram_size.has_ram() && ram_enabled {
                let addr = cart.ram_addr(addr);
                cart.ram[addr as usize]
            } else {
                0xFF
            }
        }

        match &self.mbc {
            Mbc::Mbc0 => 0xFF,
            Mbc::Mbc1 { .. } | Mbc::Mbc5 => mbc_read_ram(self, self.ram_enabled, addr),
            Mbc::Mbc2 => (mbc_read_ram(self, self.ram_enabled, addr) & 0xF) | 0xF0,
            Mbc::Mbc3 { rtc, .. } => rtc
                .as_ref()
                .and_then(|r| r.read(self.ram_enabled))
                .unwrap_or_else(|| mbc_read_ram(self, self.ram_enabled, addr)),
        }
    }

    #[expect(clippy::too_many_lines)]
    pub fn write_rom(&mut self, addr: u16, val: u8) {
        match &mut self.mbc {
            Mbc::Mbc0 => (),
            Mbc::Mbc1 { bank_mode } => {
                const fn mbc1_rom_offsets(c: &Cartridge, bank_mode: bool) -> (u32, u32) {
                    let (lo, hi) = (c.rom_bank_lo, c.rom_bank_hi << 5);

                    let lo_bank = if bank_mode {
                        hi as u16 & c.rom_size.mask()
                    } else {
                        0
                    };
                    let hi_bank = (hi | lo) as u16 & c.rom_size.mask();

                    (
                        ROMSize::BANK_SIZE as u32 * lo_bank as u32,
                        ROMSize::BANK_SIZE as u32 * hi_bank as u32,
                    )
                }

                const fn mbc1_ram_offset(cart: &Cartridge, bank_mode: bool) -> u32 {
                    let bank = if bank_mode {
                        cart.rom_bank_hi as u32
                    } else {
                        0
                    };
                    RAMSize::BANK_SIZE as u32 * bank
                }

                match addr {
                    0x0000..=0x1FFF => {
                        self.ram_enabled = (val & 0xF) == 0xA;
                    }
                    0x2000..=0x3FFF => {
                        let bank_mode = *bank_mode;

                        self.rom_bank_lo = if val == 0 { 1 } else { val };
                        self.rom_offsets = mbc1_rom_offsets(self, bank_mode);
                    }
                    0x4000..=0x5FFF => {
                        let bank_mode = *bank_mode;

                        self.rom_bank_hi = val & 3;
                        self.rom_offsets = mbc1_rom_offsets(self, bank_mode);
                        self.ram_offset = mbc1_ram_offset(self, bank_mode);
                    }
                    0x6000..=0x7FFF => {
                        *bank_mode = val & 1 != 0;
                        let bank_mode = *bank_mode;

                        self.rom_offsets = mbc1_rom_offsets(self, bank_mode);
                        self.ram_offset = mbc1_ram_offset(self, bank_mode);
                    }
                    _ => (),
                }
            }
            Mbc::Mbc2 => {
                if addr <= 0x3FFF {
                    if (addr >> 8) & 1 == 0 {
                        self.ram_enabled = (val & 0xF) == 0xA;
                    } else {
                        let val = val & 0xF;
                        self.rom_bank_lo = if val == 0 { 1 } else { val };
                        self.rom_offsets = (
                            0,
                            u32::from(ROMSize::BANK_SIZE) * u32::from(self.rom_bank_lo),
                        );
                    }
                }
            }
            Mbc::Mbc3 { rtc, is_mbc30 } => match addr {
                0x0000..=0x1FFF => {
                    self.ram_enabled = (val & 0x0F) == 0x0A;
                }
                0x2000..=0x3FFF => {
                    let mask = if *is_mbc30 { 0xFF } else { 0x7F };
                    self.rom_bank_lo = val & (self.rom_size.mask() & mask) as u8;

                    if self.rom_bank_lo == 0 {
                        self.rom_bank_lo = 1;
                    }

                    self.rom_offsets = (
                        0,
                        u32::from(ROMSize::BANK_SIZE) * u32::from(self.rom_bank_lo),
                    );
                }
                0x4000..=0x5FFF => {
                    if (0x8..=0xC).contains(&val) {
                        // Write to RTC registers
                        if let Some(r) = rtc.as_mut() {
                            #[expect(
                                clippy::unwrap_used,
                                reason = "val can only be 0x8..=0xC it will panic only when passed 0"
                            )]
                            r.map_reg(val).unwrap();
                        }
                    } else {
                        // Choose RAM bank
                        let mask = if *is_mbc30 { 0xF } else { 0x7 };
                        self.ram_bank = val & mask & self.ram_size.mask();
                        self.ram_offset = u32::from(RAMSize::BANK_SIZE) * u32::from(self.ram_bank);

                        if let Some(r) = rtc.as_mut() {
                            r.unmap_reg();
                        }
                    }
                }
                0x6000..=0x7FFF => {
                    // TODO: no need to latch?
                }
                _ => (),
            },
            Mbc::Mbc5 => {
                const fn mbc5_rom_offsets(cart: &Cartridge) -> (u32, u32) {
                    let lo = cart.rom_bank_lo as u16;
                    let hi = (cart.rom_bank_hi as u16) << 8;
                    let rom_bank = (hi | lo) & cart.rom_size.mask();
                    (0, ROMSize::BANK_SIZE as u32 * rom_bank as u32)
                }

                match addr {
                    0x0000..=0x1FFF => {
                        self.ram_enabled = val & 0xF == 0xA;
                    }
                    0x2000..=0x2FFF => {
                        self.rom_bank_lo = val;
                        self.rom_offsets = mbc5_rom_offsets(self);
                    }
                    0x3000..=0x3FFF => {
                        self.rom_bank_hi = val;
                        self.rom_offsets = mbc5_rom_offsets(self);
                    }
                    0x4000..=0x5FFF => {
                        self.ram_bank = val & self.ram_size.mask();
                        self.ram_offset = u32::from(RAMSize::BANK_SIZE) * u32::from(self.ram_bank);
                    }
                    _ => (),
                }
            }
        }
    }

    pub fn write_ram(&mut self, addr: u16, val: u8) {
        fn mbc_write_ram(cart: &mut Cartridge, ram_enabled: bool, addr: u16, val: u8) {
            if cart.ram_size.has_ram() && ram_enabled {
                let addr = cart.ram_addr(addr);
                cart.ram[addr as usize] = val;
            }
        }

        match &mut self.mbc {
            Mbc::Mbc0 => (),
            Mbc::Mbc1 { .. } | Mbc::Mbc2 | Mbc::Mbc5 => {
                mbc_write_ram(self, self.ram_enabled, addr, val);
            }
            Mbc::Mbc3 { rtc, .. } => rtc
                .as_mut()
                .and_then(|r| r.write(self.ram_enabled, val))
                .unwrap_or_else(|| {
                    mbc_write_ram(self, self.ram_enabled, addr, val);
                }),
        }
    }

    #[must_use]
    const fn ram_addr(&self, addr: u16) -> u32 {
        self.ram_offset | (addr & 0x1FFF) as u32
    }
}
