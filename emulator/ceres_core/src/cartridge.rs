use alloc::vec;
use {
    alloc::boxed::Box,
    Mbc::{Mbc1, Mbc2, Mbc3, Mbc5, NoMbc},
};

const ROM_BANK_SIZE: usize = 0x4000;
const RAM_BANK_SIZE: usize = 0x2000;

#[allow(clippy::enum_variant_names)]
enum Mbc {
    NoMbc,
    Mbc1 {
        // 1 MiB Multi-Game Compilation Carts
        multicart: bool,
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
    fn mbc_and_battery(mbc_byte: u8) -> Result<(Self, bool), InitializationError> {
        let res = match mbc_byte {
            0x00 => (NoMbc, false),
            0x01 | 0x02 => (
                Mbc1 {
                    multicart: false,
                    bank_mode: false,
                },
                false,
            ),
            0x03 => (
                Mbc1 {
                    multicart: false,
                    bank_mode: false,
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
            _ => return Err(InitializationError::UnsupportedMBC),
        };

        Ok(res)
    }
}

/// Represents a cartridge initialization error.
#[derive(Debug)]
pub enum InitializationError {
    InvalidRomSize,
    InvalidRamSize,
    NonAsciiTitleString,
    UnsupportedMBC,
}

pub struct Cartridge {
    mbc: Mbc,

    rom: Box<[u8]>,
    ram: Box<[u8]>,

    rom_bank_lo: u8,
    rom_bank_hi: u8,
    rom_offsets: (usize, usize),
    // bit mask of rom bank, anded with the rom bank selected gets
    // the actual rom bank depending on the ROM size
    rom_bank_mask: usize,

    ram_enabled: bool,
    ram_bank: u8,
    ram_offset: usize,

    has_battery: bool,
    has_ram: bool,
}

impl Cartridge {
    /// # Errors
    pub fn new(rom: Box<[u8]>, save_data: Option<Box<[u8]>>) -> Result<Self, InitializationError> {
        let rom_size = ROMSize::new(&rom)?;
        let ram_size = RAMSize::new(&rom)?;
        let rom_bank_mask = rom_size.bank_bit_mask();
        let has_ram = ram_size != RAMSize::NoRAM;
        let (mut mbc, has_battery) = Mbc::mbc_and_battery(rom[0x147])?;

        let ram = if let Some(save_data) = save_data {
            if let Mbc3 { rtc: Some(rtc) } = &mut mbc {
                rtc.deserialize(&save_data[save_data.len() - Mbc3RTC::serialize_len()..]);
            }
            save_data
        } else {
            let mut ram_len = ram_size.size_bytes();

            if let Mbc3 { rtc: Some(_) } = &mut mbc {
                ram_len += Mbc3RTC::serialize_len();
            }

            vec![0; ram_len].into_boxed_slice()
        };

        Ok(Self {
            mbc,
            rom,
            ram,
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
        self.has_battery.then(|| {
            if let Mbc3 { rtc: Some(rtc) } = &self.mbc {
                let len = self.ram.len();
                rtc.serialize(&mut self.ram[len - Mbc3RTC::serialize_len()..]);
            }

            self.ram.as_ref()
        })
    }

    #[must_use]
    pub fn clock(&self) -> Option<&[u8]> {
        if let Mbc3 { rtc: Some(rtc) } = &self.mbc {
            Some(&rtc.timer)
        } else {
            None
        }
    }

    #[must_use]
    pub fn has_battery(&self) -> bool {
        self.has_battery
    }

    pub(crate) fn run_cycles(&mut self, cycles: i32) {
        if let Mbc3 { rtc: Some(rtc) } = &mut self.mbc {
            rtc.run_cycles(cycles);
        }
    }

    #[must_use]
    pub(crate) fn read_rom(&self, addr: u16) -> u8 {
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
        fn mbc_read_ram(cart: &Cartridge, ram_enabled: bool, addr: u16) -> u8 {
            if cart.has_ram && ram_enabled {
                let addr = cart.ram_addr(addr);
                cart.ram[addr]
            } else {
                0xFF
            }
        }

        match &self.mbc {
            NoMbc => 0xFF,
            Mbc1 { .. } | Mbc5 => mbc_read_ram(self, self.ram_enabled, addr),
            Mbc2 => (mbc_read_ram(self, self.ram_enabled, addr) & 0xF) | 0xF0,
            Mbc3 { rtc } => rtc
                .as_ref()
                .and_then(|r| r.read(self.ram_enabled))
                .unwrap_or_else(|| match self.ram_bank {
                    0x00..=0x07 => mbc_read_ram(self, self.ram_enabled, addr),
                    _ => 0xFF,
                }),
        }
    }

    #[allow(clippy::too_many_lines)]
    pub(crate) fn write_rom(&mut self, addr: u16, val: u8) {
        // TODO: &mut
        match &mut self.mbc {
            NoMbc => (),
            Mbc1 {
                multicart,
                bank_mode,
            } => {
                fn mbc1_rom_offsets(
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

                fn mbc1_ram_offset(cart: &Cartridge, bank_mode: bool) -> usize {
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
                    if val > 0x7 && val <= 0xC {
                        if let Some(r) = rtc.as_mut() {
                            r.mapped_reg = Some(val);
                        }
                    }

                    self.ram_bank = val & 0x7;
                    self.ram_offset = RAM_BANK_SIZE * self.ram_bank as usize;

                    if let Some(r) = rtc.as_mut() {
                        r.mapped_reg = None;
                    }
                }
                0x6000..=0x7FFF => {
                    if let Some(r) = rtc.as_mut() {
                        r.write_latch(val);
                    }
                }
                _ => (),
            },
            Mbc5 => {
                fn mbc5_rom_offsets(cart: &Cartridge) -> (usize, usize) {
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
            NoMbc => (),
            Mbc1 { .. } | Mbc2 | Mbc5 => {
                mbc_write_ram(self, self.ram_enabled, addr, val);
            }
            Mbc3 { rtc } => {
                if rtc
                    .as_mut()
                    .and_then(|r| r.write(self.ram_enabled, val))
                    .is_none()
                {
                    if let 0x00..=0x07 = self.ram_bank {
                        mbc_write_ram(self, self.ram_enabled, addr, val);
                    }
                }
            }
        }
    }

    #[must_use]
    fn ram_addr(&self, addr: u16) -> usize {
        self.ram_offset | (addr as usize & 0x1FFF)
    }
}

#[derive(PartialEq, Eq)]
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
    fn new(rom: &[u8]) -> Result<Self, InitializationError> {
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
            _ => return Err(InitializationError::InvalidRomSize),
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum RAMSize {
    NoRAM,
    Kb2,
    Kb8,
    Kb32,
    Kb128,
    Kb64,
}

impl RAMSize {
    const fn new(rom: &[u8]) -> Result<Self, InitializationError> {
        use RAMSize::{Kb128, Kb2, Kb32, Kb64, Kb8, NoRAM};
        let ram_size_byte = rom[0x149];
        let ram_size = match ram_size_byte {
            0 => NoRAM,
            1 => Kb2,
            2 => Kb8,
            3 => Kb32,
            4 => Kb128,
            5 => Kb64,
            _ => return Err(InitializationError::InvalidRamSize),
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
    cycle_timer: i32,
    timer: [u8; 5],
    latched: [u8; 5],
    last_written: u8,
    mapped_reg: Option<u8>,
    halt: bool,
    carry: bool,
}

impl Mbc3RTC {
    fn run_cycles(&mut self, cycles: i32) {
        for _ in 0..cycles {
            self.update_t_cycle();
        }
    }

    fn update_t_cycle(&mut self) {
        if self.halt {
            return;
        }

        if self.cycle_timer == crate::TC_SEC {
            self.cycle_timer = 0;
            self.update_secs();
        } else {
            self.cycle_timer += 1;
        }
    }

    fn update_secs(&mut self) {
        #[allow(clippy::if_not_else)]
        if self.timer[0] > 60 {
            self.timer[0] = 0;
            if self.timer[1] > 60 {
                self.timer[1] = 0;
                if self.timer[2] > 24 {
                    self.timer[2] = 0;
                    if self.timer[3] == 255 {
                        self.timer[3] = 0;
                        if self.timer[4] != 0 {
                            self.timer[4] = 0;
                            self.carry = true;
                        } else {
                            self.timer[4] += 1;
                        }
                    } else {
                        self.timer[3] += 1;
                    }
                } else {
                    self.timer[2] += 1;
                }
            } else {
                self.timer[1] += 1;
            }
        } else {
            self.timer[0] += 1;
        }
    }

    fn write_latch(&mut self, val: u8) {
        if self.last_written == 0 && val == 1 {
            self.latched.copy_from_slice(&self.timer);
        }

        self.last_written = val;
    }

    fn read(&self, ram_enabled: bool) -> Option<u8> {
        if !ram_enabled {
            return None;
        }

        self.mapped_reg.map(|m| match m {
            0x8 => self.latched[0],
            0x9 => self.latched[1],
            0xa => self.latched[2],
            0xb => self.latched[3],
            0xc => self.latched[4] | (u8::from(self.halt) << 6) | (u8::from(self.halt) << 7),
            _ => unreachable!("Not a valid RTC register"),
        })
    }

    fn write(&mut self, ram_enabled: bool, val: u8) -> Option<()> {
        if !ram_enabled {
            return None;
        }

        if let Some(mapped) = self.mapped_reg {
            match mapped {
                0x8 => {
                    self.timer[0] = val;
                    self.latched[0] = val;
                }
                0x9 => {
                    self.timer[1] = val;
                    self.latched[1] = val;
                }
                0xa => {
                    self.timer[2] = val;
                    self.latched[2] = val;
                }
                0xb => {
                    self.timer[3] = val;
                    self.latched[3] = val;
                }
                0xc => {
                    self.timer[4] = val;
                    self.latched[4] = val;
                    self.carry = val & 0x80 != 0;
                    self.halt = val & 0x40 != 0;
                }
                _ => unreachable!("Not a valid RTC register"),
            }
            Option::Some(())
        } else {
            Option::None
        }
    }

    const fn serialize_len() -> usize {
        5 + 8
    }

    fn serialize(&self, buf: &mut [u8]) {
        // let start = std::time::SystemTime::now();
        // let now: [u8; 8] = start
        //     .duration_since(std::time::UNIX_EPOCH)
        //     .expect("Time went backwards")
        //     .as_secs()
        //     .to_be_bytes();

        // copy into buffer
        buf[0..5].copy_from_slice(&self.timer);
        // buf[5..(5 + 8)].copy_from_slice(&now);
    }

    fn deserialize(&mut self, buf: &[u8]) {
        self.timer.copy_from_slice(&buf[0..5]);

        // let mut saved_time: [u8; 8] = [0; 8];
        // saved_time.copy_from_slice(&buf[5..(5 + 8)]);

        // let start = std::time::SystemTime::now();
        // let now = start
        //     .duration_since(std::time::UNIX_EPOCH)
        //     .expect("Time went backwards");

        // let saved_time = std::time::Duration::from_secs(u64::from_be_bytes(saved_time));

        // let secs = (now - saved_time).as_secs();

        // for _ in 0..secs {
        //     self.update_secs();
        // }
    }
}
