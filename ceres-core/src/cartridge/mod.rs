mod mbc;
mod ram_size;
mod rom_size;

use {
    crate::Error,
    alloc::boxed::Box,
    mbc::{Mbc, Mbc3RTC},
    ram_size::RAMSize,
    rom_size::ROMSize,
};

#[derive(Debug)]
pub struct Cartridge {
    /// True for the special MBC1 1 MiB (64-bank) "multicart" wiring.
    /// Detected via the heuristic that every ROM bank carries the
    /// Nintendo logo at $0104–$0133, which real multicarts tend to do
    /// (and which standard MBC1 carts do not).
    is_mbc1_multicart: bool,
    has_battery: bool,
    mbc: Mbc,

    ram: Box<[u8]>,
    ram_bank: u8,
    ram_enabled: bool,
    ram_offset: u32,
    ram_size: RAMSize,

    rom: Box<[u8]>,
    rom_bank_hi: u8,
    rom_bank_lo: u8,
    rom_offsets: (u32, u32),
    rom_size: ROMSize,
}

impl Default for Cartridge {
    #[expect(
        clippy::similar_names,
        reason = "ROM and RAM are common names in this context"
    )]
    fn default() -> Self {
        let rom_size = ROMSize::default();
        let ram_size = RAMSize::default();
        let mbc = Mbc::default();
        let has_battery = false;

        let rom = alloc::vec![0xFF; rom_size.size_bytes() as usize].into_boxed_slice();
        let ram = alloc::vec![0xFF; ram_size.size_bytes() as usize].into_boxed_slice();

        Self {
            is_mbc1_multicart: false,
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

/// Nintendo logo bytes at $0104-$0133, used by the boot ROM to verify a
/// legitimate cartridge. Real MBC1 multicarts typically repeat this logo
/// in every ROM bank; standard MBC1 carts only have it in bank 0.
const NINTENDO_LOGO: [u8; 48] = [
    0xCE, 0xED, 0x66, 0x66, 0xCC, 0x0D, 0x00, 0x0B, 0x03, 0x73, 0x00, 0x83, 0x00, 0x0C, 0x00, 0x0D,
    0x00, 0x08, 0x11, 0x1F, 0x88, 0x89, 0x00, 0x0E, 0xDC, 0xCC, 0x6E, 0xE6, 0xDD, 0xDD, 0xD9, 0x99,
    0xBB, 0xBB, 0x67, 0x63, 0x6E, 0x0E, 0xEC, 0xCC, 0xDD, 0xDC, 0x99, 0x9F, 0xBB, 0xB9, 0x33, 0x3E,
];

/// Heuristic detection: an MBC1 1 MiB (64-bank) ROM with the Nintendo
/// logo present in *every* bank is treated as an MBC1 multicart. The
/// canonical mooneye-gb multicart test relies on this.
fn detect_mbc1_multicart(rom: &[u8], rom_size: ROMSize) -> bool {
    if !matches!(rom_size, ROMSize::Mb1) {
        return false;
    }
    let bank_size = ROMSize::BANK_SIZE as usize;
    let num_banks = rom.len() / bank_size;
    if num_banks < 4 {
        return false;
    }
    for bank in 0..num_banks {
        let start = bank * bank_size + 0x104;
        let end = start + NINTENDO_LOGO.len();
        if rom.len() < end {
            return false;
        }
        if rom[start..end] != NINTENDO_LOGO {
            return false;
        }
    }
    true
}

impl Cartridge {
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
    pub const fn global_checksum(&self) -> u16 {
        u16::from_le_bytes([self.rom[0x14F], self.rom[0x14E]])
    }

    #[must_use]
    pub const fn has_battery(&self) -> bool {
        self.has_battery
    }

    #[must_use]
    pub const fn header_checksum(&self) -> u8 {
        self.rom[0x14D]
    }

    #[must_use]
    pub const fn is_old_licensee_code(&self) -> bool {
        let code = self.rom[0x14B];
        code != 0x33
    }

    #[must_use]
    pub fn mbc_ram(&self) -> Option<&[u8]> {
        self.has_battery.then_some(&*self.ram)
    }

    #[must_use]
    pub fn mbc_ram_mut(&mut self) -> Option<&mut [u8]> {
        self.has_battery.then_some(&mut *self.ram)
    }

    #[expect(
        clippy::similar_names,
        reason = "ROM and RAM are common names in this context"
    )]
    pub fn new(rom: Box<[u8]>) -> Result<Self, Error> {
        if rom.len() < 0x150 {
            return Err(Error::InvalidRomHeaderSize);
        }

        // NOTE: Superfluous but silences clippy false positive
        assert!(rom.len() >= 0x150, "ROM is too small to be valid");

        let rom_size = ROMSize::from_len(rom.len())?;
        let ram_size = RAMSize::new(rom[0x149])?;
        let (mut mbc, has_battery) = Mbc::mbc_and_battery(rom[0x147])?;
        if let Mbc::Mbc3 {
            ref mut is_mbc30, ..
        } = mbc
        {
            *is_mbc30 = rom_size.size_bytes() > 0x20_0000 || ram_size.size_bytes() > 0x8000;
        }

        // Unused space reads as $FF.
        let rom = if rom.len() == rom_size.size_bytes() as usize {
            rom
        } else {
            let mut padded = alloc::vec![0xFF; rom_size.size_bytes() as usize];
            padded[..rom.len()].copy_from_slice(&rom);
            padded.into_boxed_slice()
        };

        // MBC2 has built-in 512 bytes of 4-bit RAM regardless of header
        let actual_ram_size = if matches!(mbc, Mbc::Mbc2) {
            0x200 // 512 bytes
        } else {
            ram_size.size_bytes() as usize
        };
        let ram = alloc::vec![0xFF; actual_ram_size].into_boxed_slice();

        Ok(Self {
            is_mbc1_multicart: detect_mbc1_multicart(&rom, rom_size),
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
    const fn ram_addr(&self, addr: u16) -> u32 {
        // Mask the final address with RAM size to handle wrapping
        let raw_addr = self.ram_offset | (addr & 0x1FFF) as u32;
        if self.ram_size.size_bytes() > 0 {
            raw_addr & (self.ram_size.size_bytes() - 1)
        } else {
            raw_addr
        }
    }

    #[must_use]
    pub const fn ram_size_bytes(&self) -> u32 {
        self.ram_size.size_bytes()
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

        match self.mbc {
            Mbc::Mbc0 => 0xFF,
            Mbc::Mbc1 { .. } | Mbc::Mbc5 => mbc_read_ram(self, self.ram_enabled, addr),
            Mbc::Mbc2 => {
                // MBC2 has built-in 512 bytes of 4-bit RAM
                // Upper 4 bits always read as 0xF
                if self.ram_enabled {
                    let ram_addr = (addr & 0x1FF) as usize; // 512 bytes
                    (self.ram[ram_addr] & 0xF) | 0xF0
                } else {
                    0xFF
                }
            }
            Mbc::Mbc3 { ref rtc, .. } => rtc
                .as_ref()
                .and_then(|r| r.read(self.ram_enabled))
                .unwrap_or_else(|| mbc_read_ram(self, self.ram_enabled, addr)),
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
    pub const fn rtc(&self) -> Option<&Mbc3RTC> {
        if let Mbc::Mbc3 {
            rtc: Some(ref rtc), ..
        } = self.mbc
        {
            Some(rtc)
        } else {
            None
        }
    }

    #[must_use]
    pub const fn rtc_mut(&mut self) -> Option<&mut Mbc3RTC> {
        if let Mbc::Mbc3 { ref mut rtc, .. } = self.mbc {
            rtc.as_mut()
        } else {
            None
        }
    }

    pub const fn run_rtc(&mut self, units: u32) {
        if let Mbc::Mbc3 {
            rtc: Some(ref mut rtc),
            ..
        } = self.mbc
        {
            rtc.run(units);
        }
    }

    #[must_use]
    pub const fn version(&self) -> u8 {
        self.rom[0x14C]
    }

    pub fn write_ram(&mut self, addr: u16, val: u8) {
        fn mbc_write_ram(cart: &mut Cartridge, ram_enabled: bool, addr: u16, val: u8) {
            if cart.ram_size.has_ram() && ram_enabled {
                let addr = cart.ram_addr(addr);
                cart.ram[addr as usize] = val;
            }
        }

        match self.mbc {
            Mbc::Mbc0 => (),
            Mbc::Mbc1 { .. } | Mbc::Mbc5 => {
                mbc_write_ram(self, self.ram_enabled, addr, val);
            }
            Mbc::Mbc2 => {
                // MBC2 has built-in 512 bytes of 4-bit RAM
                // Only lower 4 bits are stored
                if self.ram_enabled {
                    let ram_addr = (addr & 0x1FF) as usize; // 512 bytes
                    self.ram[ram_addr] = val & 0xF;
                }
            }
            Mbc::Mbc3 { ref mut rtc, .. } => rtc
                .as_mut()
                .and_then(|r| r.write(self.ram_enabled, val))
                .unwrap_or_else(|| {
                    mbc_write_ram(self, self.ram_enabled, addr, val);
                }),
        }
    }

    #[expect(clippy::too_many_lines)]
    pub fn write_rom(&mut self, addr: u16, val: u8) {
        match self.mbc {
            Mbc::Mbc0 => (),
            Mbc::Mbc1 { ref mut bank_mode } => {
                const fn mbc1_rom_offsets(c: &Cartridge, bank_mode: bool) -> (u32, u32) {
                    // Multicart is a special MBC1 wiring found on some 8 Mbit
                    // (1 MiB / 64-bank) cartridges. It uses a 4-quadrant
                    // layout (4 × 16 banks) instead of the standard 8 × 8
                    // layout MBC1 normally supports at this ROM size. The
                    // header is identical to a normal 1 MiB MBC1 cart, so
                    // we detect it via the Nintendo-logo-in-every-bank
                    // heuristic (see `detect_mbc1_multicart`).
                    let is_multicart = c.is_mbc1_multicart;

                    if is_multicart {
                        // bank_lo: low 4 bits index within the quadrant.
                        // Bit 4 selects between two "sub-quadrants" that
                        // both map to the same 16 physical banks; the 0→1
                        // correction only applies when bit 4 is clear.
                        let lo = c.rom_bank_lo & 0x1F;
                        let low4 = lo & 0x0F;
                        let mut lo_bank_idx = low4;
                        if lo & 0x10 == 0 && lo_bank_idx == 0 {
                            lo_bank_idx = 1;
                        }
                        // Quadrant = bank_hi (0..3), selects which 16-bank
                        // group of the 64-bank ROM.
                        let quadrant = (c.rom_bank_hi & 0x3) as u32;
                        let lo_bank_idx_u32 = lo_bank_idx as u32;
                        let hi_bank_idx = (quadrant * 16) + lo_bank_idx_u32;

                        // In mode 1, $0000-$3FFF maps to the first bank
                        // of the active quadrant (e.g. bank_hi=2 → bank 32).
                        let lo_bank = if bank_mode { quadrant * 16 } else { 0 };

                        return (
                            ROMSize::BANK_SIZE as u32 * lo_bank,
                            ROMSize::BANK_SIZE as u32 * hi_bank_idx,
                        );
                    }

                    // MBC1 bank_lo only uses lower 5 bits
                    let lo = c.rom_bank_lo & 0x1F;
                    let hi = c.rom_bank_hi << 5;

                    let lo_bank = if bank_mode {
                        hi as u16 & c.rom_size.mask()
                    } else {
                        0
                    };

                    // Calculate combined bank BEFORE ROM size mask
                    let mut combined = (hi | lo) as u16;

                    // The 0→1 correction happens on the UNMASKED lower 5 bits:
                    // if lower 5 bits of combined bank are 0, increment
                    // This check must happen BEFORE applying the ROM size mask
                    if combined.trailing_zeros() >= 5 {
                        combined += 1;
                    }

                    // Now apply ROM size mask to get the actual bank
                    let hi_bank = combined & c.rom_size.mask();

                    (
                        ROMSize::BANK_SIZE as u32 * lo_bank as u32,
                        ROMSize::BANK_SIZE as u32 * hi_bank as u32,
                    )
                }

                const fn mbc1_ram_offset(cart: &Cartridge, bank_mode: bool) -> u32 {
                    // In multicart mode the upper 2 bits of `rom_bank_hi`
                    // select the RAM bank (instead of the upper 2 ROM bank
                    // bits, which now select a quadrant).
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

                        // Store raw value, masking and 0→1 correction happen in mbc1_rom_offsets
                        self.rom_bank_lo = val;
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
                    // MBC2 uses bit 8 to distinguish RAM enable from ROM bank
                    if (addr >> 8) & 1 == 0 {
                        self.ram_enabled = (val & 0xF) == 0xA;
                    } else {
                        // MBC2 only uses lower 4 bits for bank number
                        // Store raw value, apply 0→1 correction based on lower 4 bits
                        let bank = val & 0xF;
                        let bank = if bank == 0 { 1 } else { bank };
                        // Apply ROM size mask and calculate offset
                        let masked_bank = u16::from(bank) & self.rom_size.mask();
                        self.rom_bank_lo = bank;
                        self.rom_offsets =
                            (0, u32::from(ROMSize::BANK_SIZE) * u32::from(masked_bank));
                    }
                }
            }
            Mbc::Mbc3 {
                ref mut rtc,
                ref mut is_mbc30,
            } => match addr {
                0x0000..=0x1FFF => {
                    self.ram_enabled = (val & 0x0F) == 0x0A;
                }
                0x2000..=0x3FFF => {
                    let mask = if *is_mbc30 { 0xFF } else { 0x7F };
                    #[expect(clippy::cast_possible_truncation)]
                    {
                        self.rom_bank_lo = val & (self.rom_size.mask() & mask) as u8;
                    }

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
                        if let Some(rtc) = rtc.as_mut() {
                            #[expect(
                                clippy::unwrap_used,
                                reason = "val can only be 0x8..=0xC it will panic only when passed 0"
                            )]
                            rtc.map_reg(val).unwrap();
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
                    // Any write latches the clock into the registers the CPU reads.
                    if let Some(rtc) = rtc.as_mut() {
                        rtc.latch();
                    }
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
}
