use {
    alloc::boxed::Box,
    core::{fmt::Display, num::NonZeroU8},
    Mbc::{Mbc0, Mbc1, Mbc2, Mbc3, Mbc5},
};

#[derive(Debug)]
enum Mbc {
    Mbc0,
    Mbc1 {
        // Alternative MBC1 wiring allows to address up to 2MB of ROM
        bank_mode: bool,
    },
    Mbc2,
    Mbc3 {
        // Real time clock
        rtc: Option<Mbc3RTC>,
    },
    Mbc5,
}

impl Mbc {
    fn mbc_and_battery(mbc_byte: u8, rom_size: ROMSize) -> Result<(Self, bool), Error> {
        let bank_mode = matches!(
            rom_size,
            ROMSize::Mb1 | ROMSize::Mb2 | ROMSize::Mb4 | ROMSize::Mb8
        );

        let res = match mbc_byte {
            0x00 => (Mbc0, false),
            0x01 | 0x02 => (Mbc1 { bank_mode }, false),
            0x03 => (Mbc1 { bank_mode }, true),
            0x05 => (Mbc2, false),
            0x06 => (Mbc2, true),
            0x0F | 0x10 => (
                Mbc3 {
                    rtc: Some(Mbc3RTC::default()),
                },
                true,
            ),
            0x11 | 0x12 => (Mbc3 { rtc: None }, false),
            0x13 => (Mbc3 { rtc: None }, true),
            0x19 | 0x1A => (Mbc5, false),
            // rumble
            // 0x1C | 0x1D => (Mbc5, false),
            // 0x1E => (Mbc5, true),
            0x1B => (Mbc5, true),
            _ => return Err(Error::UnsupportedMBC(mbc_byte)),
        };

        Ok(res)
    }
}

#[derive(Debug)]
pub enum Error {
    InvalidRomSize,
    InvalidRamSize,
    NonAsciiTitleString,
    UnsupportedMBC(u8),
    RomSizeDifferentThanActual,
    RamSizeDifferentThanActual,
}

impl Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidRomSize => {
                write!(f, "invalid ROM size in cartridge header")
            }
            Self::InvalidRamSize => {
                write!(f, "invalid RAM size in cartridge header")
            }
            Self::NonAsciiTitleString => write!(
                f,
                "invalid title string in cartridge header, contains non ASCII \
         characters"
            ),
            Self::UnsupportedMBC(byte) => write!(f, "unsupported MBC: {byte:#0x}"),
            Self::RomSizeDifferentThanActual => write!(
                f,
                "header ROM size is different from the size of the supplied file"
            ),
            Self::RamSizeDifferentThanActual => write!(
                f,
                "header RAM size is different from the size of the supplied file"
            ),
        }
    }
}

impl core::error::Error for Error {}

#[derive(Debug)]
pub struct Cart {
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

impl Default for Cart {
    #[expect(clippy::unwrap_used)]
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

impl Cart {
    pub fn new(rom: Box<[u8]>) -> Result<Self, Error> {
        let rom_size = ROMSize::new(rom[0x148])?;
        let ram_size = RAMSize::new(rom[0x149])?;
        let (mbc, has_battery) = Mbc::mbc_and_battery(rom[0x147], rom_size)?;

        if rom_size.size_bytes() as usize != rom.len() {
            return Err(Error::RomSizeDifferentThanActual);
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

    pub fn set_ram(&mut self, ram: Box<[u8]>) -> Result<(), Error> {
        let ram_size = RAMSize::new(self.rom[0x149])?;

        if ram_size.size_bytes() as usize != ram.len() {
            return Err(Error::RamSizeDifferentThanActual);
        }

        self.ram = ram;

        Ok(())
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
        u16::from_be_bytes([self.rom[0x14E], self.rom[0x14F]])
    }

    #[must_use]
    pub const fn version(&self) -> u8 {
        self.rom[0x14C]
    }

    #[must_use]
    #[inline]
    pub fn save_data(&self) -> Option<&[u8]> {
        self.has_battery.then_some(&*self.ram)
    }

    #[must_use]
    #[inline]
    pub const fn clock(&self) -> Option<&[u8]> {
        if let Mbc3 { rtc: Some(rtc) } = &self.mbc {
            Some(&rtc.regs)
        } else {
            None
        }
    }

    #[must_use]
    #[inline]
    pub const fn has_battery(&self) -> bool {
        self.has_battery
    }

    pub const fn ram_size_bytes(&self) -> u32 {
        self.ram_size.size_bytes()
    }

    pub(crate) fn run_rtc(&mut self, cycles: i32) {
        if let Mbc3 { rtc: Some(rtc) } = &mut self.mbc {
            rtc.run_cycles(cycles);
        }
    }

    #[must_use]
    pub(crate) const fn read_rom(&self, addr: u16) -> u8 {
        let (lo, hi) = self.rom_offsets;

        let bank_addr = match addr {
            0x0000..=0x3FFF => lo | (addr & 0x3FFF) as u32,
            0x4000..=0x7FFF => hi | (addr & 0x3FFF) as u32,
            _ => unreachable!(),
        };

        self.rom[bank_addr as usize]
    }

    #[must_use]
    #[inline]
    pub(crate) fn read_ram(&self, addr: u16) -> u8 {
        const fn mbc_read_ram(cart: &Cart, ram_enabled: bool, addr: u16) -> u8 {
            if cart.ram_size.has_ram() && ram_enabled {
                let addr = cart.ram_addr(addr);
                cart.ram[addr as usize]
            } else {
                0xFF
            }
        }

        match &self.mbc {
            Mbc0 => 0xFF,
            Mbc1 { .. } | Mbc5 => mbc_read_ram(self, self.ram_enabled, addr),
            Mbc2 => (mbc_read_ram(self, self.ram_enabled, addr) & 0xF) | 0xF0,
            Mbc3 { rtc } => rtc
                .as_ref()
                .and_then(|r| r.read(self.ram_enabled))
                .unwrap_or_else(|| mbc_read_ram(self, self.ram_enabled, addr)),
        }
    }

    #[expect(clippy::too_many_lines)]
    #[inline]
    pub(crate) fn write_rom(&mut self, addr: u16, val: u8) {
        match &mut self.mbc {
            Mbc0 => (),
            Mbc1 { bank_mode } => {
                const fn mbc1_rom_offsets(c: &Cart, bank_mode: bool) -> (u32, u32) {
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

                const fn mbc1_ram_offset(cart: &Cart, bank_mode: bool) -> u32 {
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
            Mbc2 => {
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
            Mbc3 { rtc } => match addr {
                0x0000..=0x1FFF => {
                    self.ram_enabled = (val & 0x0F) == 0x0A;
                }
                0x2000..=0x3FFF => {
                    self.rom_bank_lo = val & (self.rom_size.mask() & 0x7F) as u8;

                    if self.rom_bank_lo == 0 {
                        self.rom_bank_lo = 1;
                    };

                    self.rom_offsets = (
                        0,
                        u32::from(ROMSize::BANK_SIZE) * u32::from(self.rom_bank_lo),
                    );
                }
                0x4000..=0x5FFF => {
                    if (0x8..=0xC).contains(&val) {
                        // Write to RTC registers
                        if let Some(r) = rtc.as_mut() {
                            r.map_reg(val);
                        }
                    } else {
                        // Choose RAM bank
                        self.ram_bank = val & 0x7 & self.ram_size.mask();
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
            Mbc5 => {
                const fn mbc5_rom_offsets(cart: &Cart) -> (u32, u32) {
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

    #[inline]
    pub(crate) fn write_ram(&mut self, addr: u16, val: u8) {
        fn mbc_write_ram(cart: &mut Cart, ram_enabled: bool, addr: u16, val: u8) {
            if cart.ram_size.has_ram() && ram_enabled {
                let addr = cart.ram_addr(addr);
                cart.ram[addr as usize] = val;
            }
        }

        match &mut self.mbc {
            Mbc0 => (),
            Mbc1 { .. } | Mbc2 | Mbc5 => {
                mbc_write_ram(self, self.ram_enabled, addr, val);
            }
            Mbc3 { rtc } => rtc
                .as_mut()
                .and_then(|r| r.write(self.ram_enabled, val))
                .unwrap_or_else(|| {
                    mbc_write_ram(self, self.ram_enabled, addr, val);
                }),
        }
    }

    #[must_use]
    #[inline]
    const fn ram_addr(&self, addr: u16) -> u32 {
        self.ram_offset | (addr & 0x1FFF) as u32
    }
}

#[derive(Clone, Copy, Debug)]
enum ROMSize {
    Kb32 = 0,
    Kb64 = 1,
    Kb128 = 2,
    Kb256 = 3,
    Kb512 = 4,
    Mb1 = 5,
    Mb2 = 6,
    Mb4 = 7,
    Mb8 = 8,
}

impl ROMSize {
    const BANK_SIZE: u16 = 0x4000;

    #[inline]
    const fn new(byte: u8) -> Result<Self, Error> {
        use ROMSize::{Kb128, Kb256, Kb32, Kb512, Kb64, Mb1, Mb2, Mb4, Mb8};
        let rom_size = match byte {
            0 => Kb32,
            1 => Kb64,
            2 => Kb128,
            3 => Kb256,
            4 => Kb512,
            5 => Mb1,
            6 => Mb2,
            7 => Mb4,
            8 => Mb8,
            _ => return Err(Error::InvalidRomSize),
        };

        Ok(rom_size)
    }

    #[must_use]
    #[inline]
    const fn size_bytes(self) -> u32 {
        // maximum is 0x8000 << 8 = 0x80_0000
        (Self::BANK_SIZE as u32 * 2) << (self as u8)
    }

    #[must_use]
    #[inline]
    const fn mask(self) -> u16 {
        // maximum is 2 << 8 - 1 = 1FF
        (2_u16 << (self as u8)) - 1
    }
}

#[derive(Clone, Copy, Debug)]
enum RAMSize {
    NoRAM,
    Kb8,
    Kb32,
    Kb128,
    Kb64,
}

impl RAMSize {
    const BANK_SIZE: u16 = 0x2000;

    #[inline]
    const fn new(byte: u8) -> Result<Self, Error> {
        use RAMSize::{Kb128, Kb32, Kb64, Kb8, NoRAM};
        let ram_size = match byte {
            0 => NoRAM,
            2 => Kb8,
            3 => Kb32,
            4 => Kb128,
            5 => Kb64,
            _ => return Err(Error::InvalidRamSize),
        };

        Ok(ram_size)
    }

    #[must_use]
    #[inline]
    const fn has_ram(self) -> bool {
        !matches!(self, Self::NoRAM)
    }

    #[must_use]
    #[inline]
    const fn size_bytes(self) -> u32 {
        // Max size is 0x2000 * 0x10 = 0x20000 so it fits in a u32
        self.num_banks() as u32 * Self::BANK_SIZE as u32
    }

    #[must_use]
    #[inline]
    const fn num_banks(self) -> u8 {
        match self {
            Self::NoRAM => 0x0,
            Self::Kb8 => 0x1,
            Self::Kb32 => 0x4,
            Self::Kb128 => 0x10,
            Self::Kb64 => 0x8,
        }
    }

    #[must_use]
    #[inline]
    const fn mask(self) -> u8 {
        match self {
            Self::NoRAM | Self::Kb8 => 0x0,
            Self::Kb32 => 0x3,
            Self::Kb128 => 0xF,
            Self::Kb64 => 0x7,
        }
    }
}

#[derive(Default, Debug)]
struct Mbc3RTC {
    t_cycles: i32,
    regs: [u8; 5],
    mapped: Option<NonZeroU8>,
    halt: bool,
    carry: bool,
}

impl Mbc3RTC {
    #[inline]
    #[expect(clippy::unwrap_used)]
    fn map_reg(&mut self, val: u8) {
        self.mapped = Some(NonZeroU8::new(val).unwrap());
    }

    #[inline]
    fn unmap_reg(&mut self) {
        self.mapped = None;
    }

    fn run_cycles(&mut self, cycles: i32) {
        if self.halt {
            return;
        }

        self.t_cycles += cycles;
        // TODO: this while is not at all necessary
        while self.t_cycles > crate::TC_SEC {
            self.t_cycles -= crate::TC_SEC + 1;
            self.update_secs();
        }
    }

    #[inline]
    fn update_secs(&mut self) {
        self.regs[0] = (self.regs[0] + 1) & 0x3F;
        if self.regs[0] == 60 {
            self.regs[0] = 0;

            self.regs[1] = (self.regs[1] + 1) & 0x3F;
            if self.regs[1] == 60 {
                self.regs[1] = 0;

                self.regs[2] = (self.regs[2] + 1) & 0x1F;
                if self.regs[2] == 24 {
                    self.regs[2] = 0;

                    self.regs[3] = self.regs[3].wrapping_add(1);
                    if self.regs[3] == 0 {
                        self.regs[4] = (self.regs[4] + 1) & 1;
                        if self.regs[4] == 0 {
                            self.carry = true;
                        }
                    }
                }
            }
        }
    }

    #[inline]
    fn read(&self, ram_enabled: bool) -> Option<u8> {
        ram_enabled
            .then(|| {
                self.mapped.map(|m| match m.get() {
                    0x8 => self.regs[0],
                    0x9 => self.regs[1],
                    0xA => self.regs[2],
                    0xB => self.regs[3],
                    0xC => self.regs[4] | (u8::from(self.halt) << 6) | (u8::from(self.carry) << 7),
                    _ => unreachable!("Not a valid RTC register"),
                })
            })
            .flatten()
    }

    #[must_use]
    fn write(&mut self, ram_enabled: bool, val: u8) -> Option<()> {
        ram_enabled
            .then(|| {
                self.mapped.map(|m| match m.get() {
                    0x8 => self.regs[0] = val & 0x3F,
                    0x9 => self.regs[1] = val & 0x3F,
                    0xA => self.regs[2] = val & 0x1F,
                    0xB => self.regs[3] = val,
                    0xC => {
                        let val = val & 0xC1;
                        self.regs[4] = val;
                        self.carry = val & 0x80 != 0;
                        self.halt = val & 0x40 != 0;
                    }
                    _ => unreachable!("Not a valid RTC register"),
                })
            })
            .flatten()
    }
}
