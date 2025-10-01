use crate::timing::DOTS_PER_SEC;
use core::num::NonZeroU8;

#[derive(Default, Debug)]
pub struct Mbc3RTC {
    carry: bool,
    dots: i32,
    halt: bool,
    mapped: Option<NonZeroU8>,
    regs: [u8; 5],
}

impl Mbc3RTC {
    /// Maps the given RTC register for reading and writing.
    /// Valid values are between 0x8 and 0xC inclusive.
    /// Returns an error if the value is out of range.
    ///
    /// # Errors
    /// Returns `Err(())` if `val` is not in the range 0x8..=0xC.  
    pub fn map_reg(&mut self, val: u8) -> Result<(), ()> {
        if !(0x8..=0xC).contains(&val) {
            return Err(());
        }

        #[expect(
            clippy::unwrap_used,
            reason = "val can only be 0x8..=0xC it will panic only when passed 0"
        )]
        {
            self.mapped = Some(NonZeroU8::new(val).unwrap());
        }
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

    pub const fn run(&mut self, dots: i32) {
        if self.halt {
            return;
        }

        self.dots += dots;
        // TODO: this while is not at all necessary
        while self.dots > DOTS_PER_SEC {
            self.dots -= DOTS_PER_SEC + 1;
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
