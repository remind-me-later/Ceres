use {
    alloc::{boxed::Box, vec},
    core::{fmt::Display, num::NonZeroU8},
    Mbc::{Mbc0, Mbc1, Mbc2, Mbc3, Mbc5},
};

const ROM_BANK_SIZE: usize = 0x4000;
const RAM_BANK_SIZE: usize = 0x2000;

enum Mbc {
    Mbc0,
    Mbc1 {
        // 1 MiB Multi-Game Compilation Carts
        multicart: bool,
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
        let bank_mode = rom_size >= ROMSize::Mb1;
        let multicart = false;

        let res = match mbc_byte {
            0x00 => (Mbc0, false),
            0x01 | 0x02 => (
                Mbc1 {
                    multicart,
                    bank_mode,
                },
                false,
            ),
            0x03 => (
                Mbc1 {
                    multicart,
                    bank_mode,
                },
                true,
            ),
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
            0x19 | 0x1A | 0x1C | 0x1D => (Mbc5, false),
            0x1B | 0x1E => (Mbc5, true),
            _ => return Err(Error::UnsupportedMBC),
        };

        Ok(res)
    }
}

/// Represents a cartridge initialization error.
#[derive(Debug)]
pub enum Error {
    InvalidRomSize,
    InvalidRamSize,
    NonAsciiTitleString,
    UnsupportedMBC,
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
                "invalid title string in cartridge header, contains non ASCII characters"
            ),
            Self::UnsupportedMBC => write!(f, "unsupported MBC"),
        }
    }
}

impl core::error::Error for Error {}

pub struct Cartridge {
    mbc: Mbc,

    rom: Box<[u8]>,
    ram: Box<[u8]>,

    rom_bank_lo:   u8,
    rom_bank_hi:   u8,
    rom_offsets:   (usize, usize),
    // bit mask of rom bank, anded with the rom bank selected gets
    // the actual rom bank depending on the ROM size
    rom_bank_mask: usize,

    ram_enabled: bool,
    ram_bank:    u8,
    ram_offset:  usize,

    has_battery: bool,
    has_ram:     bool,
}

impl Cartridge {
    /// # Errors
    pub fn new(rom: Box<[u8]>, save_data: Option<Box<[u8]>>) -> Result<Self, Error> {
        let rom_size = ROMSize::new(&rom)?;
        let mem_size = RAMSize::new(&rom)?;
        let rom_bank_mask = rom_size.bank_bit_mask();
        let has_ram = mem_size != RAMSize::NoRAM;
        let (mbc, has_battery) = Mbc::mbc_and_battery(rom[0x147], rom_size)?;

        let mem = save_data.unwrap_or_else(|| vec![0; mem_size.size_bytes()].into_boxed_slice());

        Ok(Self {
            mbc,
            rom,
            ram: mem,
            rom_bank_lo: 1,
            rom_bank_hi: 0,
            rom_offsets: (0x0000, 0x4000),
            rom_bank_mask,
            ram_enabled: false,
            ram_bank: 0,
            ram_offset: 0,
            has_battery,
            has_ram,
        })
    }

    #[must_use]
    pub fn save_data(&mut self) -> Option<&[u8]> {
        self.has_battery.then(|| self.ram.as_ref())
    }

    #[must_use]
    pub const fn clock(&self) -> Option<&[u8]> {
        if let Mbc3 { rtc: Some(rtc) } = &self.mbc {
            Some(&rtc.regs)
        } else {
            None
        }
    }

    #[must_use]
    pub const fn has_battery(&self) -> bool {
        self.has_battery
    }

    pub(crate) fn run_cycles(&mut self, cycles: i32) {
        if let Mbc3 { rtc: Some(rtc) } = &mut self.mbc {
            rtc.run_cycles(cycles);
        }
    }

    #[must_use]
    pub(crate) const fn read_rom(&self, addr: u16) -> u8 {
        let bank_addr = match addr {
            0x0000..=0x3FFF => {
                let (rom_lower, _) = self.rom_offsets;
                rom_lower | (addr as usize & 0x3FFF)
            }
            0x4000..=0x7FFF => {
                let (_, rom_upper) = self.rom_offsets;
                rom_upper | (addr as usize & 0x3FFF)
            }
            _ => 0,
        };

        self.rom[bank_addr]
    }

    #[must_use]
    pub(crate) fn read_ram(&self, addr: u16) -> u8 {
        const fn mbc_read_ram(cart: &Cartridge, ram_enabled: bool, addr: u16) -> u8 {
            if cart.has_ram && ram_enabled {
                let addr = cart.ram_addr(addr);
                cart.ram[addr]
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

    #[allow(clippy::too_many_lines)]
    pub(crate) fn write_rom(&mut self, addr: u16, val: u8) {
        match &mut self.mbc {
            Mbc0 => (),
            Mbc1 {
                multicart,
                bank_mode,
            } => {
                const fn mbc1_rom_offsets(
                    cart: &Cartridge,
                    multicart: bool,
                    bank_mode: bool,
                ) -> (usize, usize) {
                    let (upper_bits, lower_bits) = if multicart {
                        (cart.rom_bank_hi << 4, cart.rom_bank_lo & 0xF)
                    } else {
                        (cart.rom_bank_hi << 5, cart.rom_bank_lo)
                    };

                    let lower_bank = if bank_mode { upper_bits as usize } else { 0 };
                    let upper_bank = (upper_bits | lower_bits) as usize;

                    (ROM_BANK_SIZE * lower_bank, ROM_BANK_SIZE * upper_bank)
                }

                const fn mbc1_ram_offset(cart: &Cartridge, bank_mode: bool) -> usize {
                    let bank = if bank_mode {
                        cart.rom_bank_hi as usize
                    } else {
                        0
                    };

                    RAM_BANK_SIZE * bank
                }

                let multicart = *multicart;

                match addr {
                    0x0000..=0x1FFF => self.ram_enabled = (val & 0xF) == 0xA,
                    0x2000..=0x3FFF => {
                        let bank_mode = *bank_mode;

                        let val = val & 0x1F;
                        self.rom_bank_lo = if val == 0 { 1 } else { val };
                        self.rom_offsets = mbc1_rom_offsets(self, multicart, bank_mode);
                    }
                    0x4000..=0x5FFF => {
                        let bank_mode = *bank_mode;

                        self.rom_bank_hi = val & 3;
                        self.rom_offsets = mbc1_rom_offsets(self, multicart, bank_mode);
                        self.ram_offset = mbc1_ram_offset(self, bank_mode);
                    }
                    0x6000..=0x7FFF => {
                        *bank_mode = val & 1 != 0;
                        let bank_mode = *bank_mode;

                        self.rom_offsets = mbc1_rom_offsets(self, multicart, bank_mode);
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
                        self.rom_offsets = (0x0000, ROM_BANK_SIZE * self.rom_bank_lo as usize);
                    }
                }
            }
            Mbc3 { rtc } => match addr {
                0x0000..=0x1FFF => self.ram_enabled = (val & 0x0F) == 0x0A,
                0x2000..=0x3FFF => {
                    self.rom_bank_lo = if val == 0 { 1 } else { val & 0x7F };
                    self.rom_offsets = (0x0000, ROM_BANK_SIZE * self.rom_bank_lo as usize);
                }
                0x4000..=0x5FFF => {
                    if (0x8..=0xC).contains(&val) {
                        // Write to RTC registers
                        if let Some(r) = rtc.as_mut() {
                            r.map_reg(val);
                        }
                    } else {
                        // Choose RAM bank
                        self.ram_bank = val & 0x7;
                        self.ram_offset = RAM_BANK_SIZE * self.ram_bank as usize;

                        if let Some(r) = rtc.as_mut() {
                            r.unmap_reg();
                        }
                    }
                }
                0x6000..=0x7FFF => {
                    // No need to latch
                }
                _ => (),
            },
            Mbc5 => {
                const fn mbc5_rom_offsets(cart: &Cartridge) -> (usize, usize) {
                    let lower_bits = cart.rom_bank_lo as usize;
                    let upper_bits = (cart.rom_bank_hi as usize) << 8;
                    let rom_bank = (upper_bits | lower_bits) & cart.rom_bank_mask;
                    // let rom_bank = if rom_bank == 0 { 1 } else { rom_bank };
                    (0x0000, ROM_BANK_SIZE * rom_bank)
                }

                match addr {
                    0x0000..=0x1FFF => self.ram_enabled = val & 0xF == 0xA,
                    0x2000..=0x2FFF => {
                        self.rom_bank_lo = val;
                        self.rom_offsets = mbc5_rom_offsets(self);
                    }
                    0x3000..=0x3FFF => {
                        self.rom_bank_hi = val & 1;
                        self.rom_offsets = mbc5_rom_offsets(self);
                    }
                    0x4000..=0x5FFF => {
                        self.ram_bank = val & 0xF;
                        self.ram_offset = RAM_BANK_SIZE * self.ram_bank as usize;
                    }
                    _ => (),
                }
            }
        }
    }

    pub(crate) fn write_ram(&mut self, addr: u16, val: u8) {
        fn mbc_write_ram(cart: &mut Cartridge, ram_enabled: bool, addr: u16, val: u8) {
            if cart.has_ram && ram_enabled {
                let addr = cart.ram_addr(addr);
                cart.ram[addr] = val;
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
                .unwrap_or_else(|| mbc_write_ram(self, self.ram_enabled, addr, val)),
        }
    }

    #[must_use]
    const fn ram_addr(&self, addr: u16) -> usize {
        self.ram_offset | (addr as usize & 0x1FFF)
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
enum ROMSize {
    Kb32  = 0,
    Kb64  = 1,
    Kb128 = 2,
    Kb256 = 3,
    Kb512 = 4,
    Mb1   = 5,
    Mb2   = 6,
    Mb4   = 7,
    Mb8   = 8,
}

impl ROMSize {
    const fn new(rom: &[u8]) -> Result<Self, Error> {
        use ROMSize::{Kb128, Kb256, Kb32, Kb512, Kb64, Mb1, Mb2, Mb4, Mb8};
        let rom_size_byte = rom[0x148];
        let rom_size = match rom_size_byte {
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

    #[allow(dead_code)]
    // total size in  bytes
    const fn size_bytes(self) -> usize {
        let kib_32 = 1 << 15;
        kib_32 << (self as usize)
    }

    const fn bank_bit_mask(self) -> usize {
        (2 << (self as usize)) - 1
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum RAMSize {
    NoRAM,
    Kb2,
    Kb8,
    Kb32,
    Kb128,
    Kb64,
}

impl RAMSize {
    const fn new(rom: &[u8]) -> Result<Self, Error> {
        use RAMSize::{Kb128, Kb2, Kb32, Kb64, Kb8, NoRAM};
        let ram_size_byte = rom[0x149];
        let ram_size = match ram_size_byte {
            0 => NoRAM,
            1 => Kb2,
            2 => Kb8,
            3 => Kb32,
            4 => Kb128,
            5 => Kb64,
            _ => return Err(Error::InvalidRamSize),
        };

        Ok(ram_size)
    }

    const fn size_bytes(self) -> usize {
        self.num_banks() * self.bank_size_in_bytes()
    }

    const fn num_banks(self) -> usize {
        match self {
            Self::NoRAM => 0,
            Self::Kb2 | Self::Kb8 => 1,
            Self::Kb32 => 4,
            Self::Kb128 => 16,
            Self::Kb64 => 8,
        }
    }

    const fn bank_size_in_bytes(self) -> usize {
        use RAMSize::{Kb128, Kb2, Kb32, Kb64, Kb8, NoRAM};
        match self {
            NoRAM => 0,
            Kb2 => 0x800,
            Kb8 | Kb32 | Kb128 | Kb64 => 0x2000,
        }
    }
}

#[derive(Default)]
struct Mbc3RTC {
    t_cycles: i32,
    regs:     [u8; 5],
    mapped:   Option<NonZeroU8>,
    halt:     bool,
    carry:    bool,
}

impl Mbc3RTC {
    fn map_reg(&mut self, val: u8) {
        self.mapped = Some(NonZeroU8::new(val).unwrap());
    }

    fn unmap_reg(&mut self) {
        self.mapped = None;
    }

    fn run_cycles(&mut self, cycles: i32) {
        for _ in 0..cycles {
            self.update_t_cycle();
        }
    }

    fn update_t_cycle(&mut self) {
        if self.halt {
            return;
        }

        if self.t_cycles == crate::TC_SEC {
            self.t_cycles = 0;
            self.update_secs();
        } else {
            self.t_cycles += 1;
        }
    }

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
