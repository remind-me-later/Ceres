use std::num::NonZeroU8;

use crate::{Error, timing::TC_SEC};

use super::rom_size::ROMSize;

#[derive(Debug)]
pub enum Mbc {
    Mbc0,
    Mbc1 {
        // Alternative MBC1 wiring allows to address up to 2MB of ROM
        bank_mode: bool,
    },
    Mbc2,
    Mbc3 {
        // Real time clock
        rtc: Option<Mbc3RTC>,
        // Mbc30 is a variant of Mbc3 used in Pokémon Crystal that allows up to 4Mb of ROM and 64Kb of RAM
        is_mbc30: bool,
    },
    Mbc5,
}

impl Mbc {
    pub fn mbc_and_battery(mbc_byte: u8, rom_size: ROMSize) -> Result<(Self, bool), Error> {
        let bank_mode = matches!(
            rom_size,
            ROMSize::Mb1 | ROMSize::Mb2 | ROMSize::Mb4 | ROMSize::Mb8
        );

        let res = match mbc_byte {
            0x00 => (Self::Mbc0, false),
            0x01 | 0x02 => (Self::Mbc1 { bank_mode }, false),
            0x03 => (Self::Mbc1 { bank_mode }, true),
            0x05 => (Self::Mbc2, false),
            0x06 => (Self::Mbc2, true),
            0x0F => (
                Self::Mbc3 {
                    rtc: Some(Mbc3RTC::default()),
                    is_mbc30: false,
                },
                true,
            ),
            0x10 => (
                Self::Mbc3 {
                    rtc: Some(Mbc3RTC::default()),
                    is_mbc30: true,
                },
                true,
            ),
            0x11 => (
                Self::Mbc3 {
                    rtc: None,
                    is_mbc30: false,
                },
                false,
            ),
            0x12 => (
                Self::Mbc3 {
                    rtc: None,
                    is_mbc30: true,
                },
                false,
            ),
            0x13 => (
                Self::Mbc3 {
                    rtc: None,
                    is_mbc30: true,
                },
                true,
            ),
            0x19 | 0x1A => (Self::Mbc5, false),
            // rumble
            // 0x1C | 0x1D => (Mbc5, false),
            // 0x1E => (Mbc5, true),
            0x1B => (Self::Mbc5, true),
            _ => {
                return Err(Error::UnsupportedMBC {
                    mbc_hex_code: mbc_byte,
                });
            }
        };

        Ok(res)
    }
}

#[derive(Default, Debug)]
pub struct Mbc3RTC {
    carry: bool,
    halt: bool,
    mapped: Option<NonZeroU8>,
    regs: [u8; 5],
    t_cycles: i32,
}

impl Mbc3RTC {
    // Mapped register must be in 0x8..=0xC
    pub fn map_reg(&mut self, val: u8) -> Result<(), ()> {
        debug_assert!(
            (0x8..=0xC).contains(&val),
            "Mapped RTC register must be in 0x8..=0xC, got {val:#X}",
        );
        self.mapped = Some(NonZeroU8::new(val).ok_or(())?);
        Ok(())
    }

    pub fn read(&self, ram_enabled: bool) -> Option<u8> {
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

    pub const fn run_cycles(&mut self, cycles: i32) {
        if self.halt {
            return;
        }

        self.t_cycles += cycles;
        // TODO: this while is not at all necessary
        while self.t_cycles > TC_SEC {
            self.t_cycles -= TC_SEC + 1;
            self.update_secs();
        }
    }

    pub const fn unmap_reg(&mut self) {
        self.mapped = None;
    }

    const fn update_secs(&mut self) {
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

    #[must_use]
    pub fn write(&mut self, ram_enabled: bool, val: u8) -> Option<()> {
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

// Getters and Setters
impl Mbc3RTC {
    #[expect(clippy::cast_possible_truncation)]
    pub fn add_seconds(&mut self, val: u64) {
        let secs = u64::from(self.regs[0]) + val;
        self.regs[0] = (secs % 60) as u8;

        let mins = u64::from(self.regs[1]) + secs / 60;
        self.regs[1] = (mins % 60) as u8;

        let hours = u64::from(self.regs[2]) + mins / 60;
        self.regs[2] = (hours % 24) as u8;

        let days = u64::from(self.regs[3]) + hours / 24;
        self.regs[3] = (days % 256) as u8;

        let carry = days / 256;
        self.regs[4] = (self.regs[4] + carry as u8) & 0x1;

        self.carry = carry != 0;
    }

    pub fn control(&self) -> u8 {
        self.regs[4] | (u8::from(self.halt) << 6) | (u8::from(self.carry) << 7)
    }

    pub const fn days(&self) -> u8 {
        self.regs[3]
    }

    pub const fn hours(&self) -> u8 {
        self.regs[2]
    }

    pub const fn minutes(&self) -> u8 {
        self.regs[1]
    }

    pub const fn seconds(&self) -> u8 {
        self.regs[0]
    }

    pub const fn set_control(&mut self, val: u8) {
        let val = val & 0xC1;
        self.regs[4] = val;
        self.carry = val & 0x80 != 0;
        self.halt = val & 0x40 != 0;
    }

    pub const fn set_days(&mut self, val: u8) {
        self.regs[3] = val;
    }

    pub const fn set_hours(&mut self, val: u8) {
        self.regs[2] = val;
    }

    pub const fn set_minutes(&mut self, val: u8) {
        self.regs[1] = val;
    }

    pub const fn set_seconds(&mut self, val: u8) {
        self.regs[0] = val;
    }
}
