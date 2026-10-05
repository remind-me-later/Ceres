use core::num::NonZeroU8;

/// 8 MHz units in a second: the RTC runs off its own crystal, so a second is
/// 4 194 304 CPU cycles in single speed and twice as many in double speed.
const SECOND_UNITS: u32 = 0x0080_0000;

/// The MBC3's real-time clock (SameBoy's model): the clock ticks the real
/// registers, the CPU reads the latched copy.
#[derive(Default, Debug)]
pub struct Mbc3RTC {
    /// 8 MHz units since the last tick.
    cycles: u32,
    /// Seconds, minutes, hours, days (low) and control (day bit 8, halt and
    /// carry) as written / ticked.
    real: [u8; 5],
    latched: [u8; 5],
    mapped: Option<NonZeroU8>,
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

    /// The latched register that is mapped, if any.
    pub fn read(&self, ram_enabled: bool) -> Option<u8> {
        const MASKS: [u8; 5] = [0x3F, 0x3F, 0x1F, 0xFF, 0xC1];
        ram_enabled
            .then(|| {
                self.mapped.map(|m| {
                    let index = usize::from(m.get() - 0x8);
                    self.latched[index] & MASKS[index]
                })
            })
            .flatten()
    }

    /// Copies the real registers to the latched ones (any write to $6000-$7FFF).
    pub const fn latch(&mut self) {
        self.latched = self.real;
    }

    /// Advances the clock by `units` 8 MHz units (a CPU cycle in double speed,
    /// two in single speed).
    pub const fn run(&mut self, units: u32) {
        if self.real[4] & 0x40 != 0 {
            return;
        }

        self.cycles += units;
        while self.cycles >= SECOND_UNITS {
            self.cycles -= SECOND_UNITS;
            self.tick();
        }
    }

    pub const fn unmap_reg(&mut self) {
        self.mapped = None;
    }

    /// One second passes.
    const fn tick(&mut self) {
        let r = &mut self.real;
        r[0] = r[0].wrapping_add(1);
        if r[0] != 60 {
            return;
        }
        r[0] = 0;
        r[1] = r[1].wrapping_add(1);
        if r[1] != 60 {
            return;
        }
        r[1] = 0;
        r[2] = r[2].wrapping_add(1);
        if r[2] != 24 {
            return;
        }
        r[2] = 0;
        r[3] = r[3].wrapping_add(1);
        if r[3] != 0 {
            return;
        }
        if r[4] & 1 != 0 {
            r[4] |= 0x80;
        }
        r[4] ^= 1;
    }

    #[must_use]
    pub fn write(&mut self, ram_enabled: bool, val: u8) -> Option<()> {
        ram_enabled
            .then(|| {
                self.mapped.map(|m| {
                    let index = usize::from(m.get() - 0x8);
                    if index == 0 {
                        // Writing the seconds restarts the sub-second count.
                        self.cycles = 0;
                    }
                    self.real[index] = val;
                })
            })
            .flatten()
    }
}

// Getters and Setters
impl Mbc3RTC {
    #[expect(clippy::cast_possible_truncation)]
    pub fn add_seconds(&mut self, val: u64) {
        let secs = u64::from(self.real[0]) + val;
        self.real[0] = (secs % 60) as u8;

        let mins = u64::from(self.real[1]) + secs / 60;
        self.real[1] = (mins % 60) as u8;

        let hours = u64::from(self.real[2]) + mins / 60;
        self.real[2] = (hours % 24) as u8;

        let days = u64::from(self.real[3]) + hours / 24;
        self.real[3] = (days % 256) as u8;

        let carry = days / 256;
        self.real[4] = (self.real[4] & !1) | ((self.real[4] + carry as u8) & 1);
        if carry != 0 {
            self.real[4] |= 0x80;
        }
        self.latch();
    }

    pub const fn control(&self) -> u8 {
        self.real[4]
    }

    pub const fn days(&self) -> u8 {
        self.real[3]
    }

    pub const fn hours(&self) -> u8 {
        self.real[2]
    }

    pub const fn minutes(&self) -> u8 {
        self.real[1]
    }

    pub const fn seconds(&self) -> u8 {
        self.real[0]
    }

    pub const fn latched(&self) -> [u8; 5] {
        self.latched
    }

    pub const fn set_latched(&mut self, latched: [u8; 5]) {
        self.latched = latched;
    }

    pub const fn set_control(&mut self, val: u8) {
        self.real[4] = val & 0xC1;
    }

    pub const fn set_days(&mut self, val: u8) {
        self.real[3] = val;
    }

    pub const fn set_hours(&mut self, val: u8) {
        self.real[2] = val;
    }

    pub const fn set_minutes(&mut self, val: u8) {
        self.real[1] = val;
    }

    pub const fn set_seconds(&mut self, val: u8) {
        self.real[0] = val;
    }
}
