use core::num::NonZeroU8;

/// 8 MHz units in a second: the RTC runs off its own crystal, so a second is
/// 4 194 304 CPU cycles in single speed and twice as many in double speed.
const SECOND_UNITS: u32 = 0x0080_0000;
const SECONDS_PER_DAY: u64 = 24 * 60 * 60;
/// The day counter has 9 bits: the carry is set when it overflows.
const DAYS: u64 = 0x200;

/// The RAM bank numbers that map a clock register instead.
pub const RTC_REG_FIRST: u8 = 0x08;
pub const RTC_REG_LAST: u8 = 0x0C;

/// The control register's index, and its bits.
const CONTROL: usize = 4;
/// Bit 8 of the day counter.
const CONTROL_DAY_HIGH_B: u8 = 0x01;
/// The clock is stopped.
const CONTROL_HALT_B: u8 = 0x40;
/// The day counter overflowed.
const CONTROL_CARRY_B: u8 = 0x80;
const CONTROL_MASK: u8 = CONTROL_CARRY_B | CONTROL_HALT_B | CONTROL_DAY_HIGH_B;

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
        if !(RTC_REG_FIRST..=RTC_REG_LAST).contains(&val) {
            return Err(());
        }
        self.mapped = NonZeroU8::new(val);
        Ok(())
    }

    /// The latched register that is mapped, if any.
    pub fn read(&self, ram_enabled: bool) -> Option<u8> {
        const MASKS: [u8; 5] = [0x3F, 0x3F, 0x1F, 0xFF, CONTROL_MASK];
        self.mapped.filter(|_| ram_enabled).map(|m| {
            let index = usize::from(m.get() - RTC_REG_FIRST);
            self.latched[index] & MASKS[index]
        })
    }

    /// Copies the real registers to the latched ones (any write to $6000-$7FFF).
    pub const fn latch(&mut self) {
        self.latched = self.real;
    }

    /// Advances the clock by `units` 8 MHz units (a CPU cycle in double speed,
    /// two in single speed).
    pub const fn run(&mut self, units: u32) {
        if self.real[CONTROL] & CONTROL_HALT_B != 0 {
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
        if r[CONTROL] & CONTROL_DAY_HIGH_B != 0 {
            r[CONTROL] |= CONTROL_CARRY_B;
        }
        r[CONTROL] ^= CONTROL_DAY_HIGH_B;
    }

    #[must_use]
    pub fn write(&mut self, ram_enabled: bool, val: u8) -> Option<()> {
        self.mapped.filter(|_| ram_enabled).map(|m| {
            let index = usize::from(m.get() - RTC_REG_FIRST);
            if index == 0 {
                // Writing the seconds restarts the sub-second count.
                self.cycles = 0;
            }
            self.real[index] = val;
        })
    }
}

// Getters and Setters
impl Mbc3RTC {
    /// `secs` seconds pass at once (a save state loaded later). As in
    /// SameBoy, the whole days are added first, then the rest a second at a
    /// time. A halted clock does not move.
    #[expect(clippy::cast_possible_truncation)]
    pub const fn add_seconds(&mut self, secs: u64) {
        if self.real[CONTROL] & CONTROL_HALT_B != 0 {
            return;
        }

        let day = self.real[3] as u64 | (((self.real[CONTROL] & CONTROL_DAY_HIGH_B) as u64) << 8);
        let day = day + secs / SECONDS_PER_DAY;
        if day >= DAYS {
            self.real[CONTROL] |= CONTROL_CARRY_B;
        }
        let day = day % DAYS;
        self.real[3] = day as u8;
        self.real[CONTROL] = (self.real[CONTROL] & !CONTROL_DAY_HIGH_B) | (day >> 8) as u8;

        let mut left = secs % SECONDS_PER_DAY;
        while left != 0 {
            self.tick();
            left -= 1;
        }
    }

    /// The clock registers: seconds, minutes, hours, days and control.
    pub const fn real(&self) -> [u8; 5] {
        self.real
    }

    pub const fn set_real(&mut self, real: [u8; 5]) {
        self.real = real;
        self.real[CONTROL] &= CONTROL_MASK;
    }

    pub const fn latched(&self) -> [u8; 5] {
        self.latched
    }

    pub const fn set_latched(&mut self, latched: [u8; 5]) {
        self.latched = latched;
    }
}

#[cfg(test)]
mod tests {
    use super::{CONTROL_CARRY_B, CONTROL_DAY_HIGH_B, CONTROL_HALT_B, Mbc3RTC, SECONDS_PER_DAY};

    fn rtc(real: [u8; 5]) -> Mbc3RTC {
        let mut rtc = Mbc3RTC::default();
        rtc.set_real(real);
        rtc
    }

    #[test]
    fn add_seconds_matches_ticking() {
        let starts = [
            [0, 0, 0, 0, 0],
            [59, 59, 23, 255, 0],
            [12, 34, 5, 255, CONTROL_DAY_HIGH_B],
            [30, 59, 23, 255, CONTROL_DAY_HIGH_B],
        ];
        let durations = [
            0,
            1,
            59,
            3600,
            SECONDS_PER_DAY - 1,
            SECONDS_PER_DAY,
            3 * SECONDS_PER_DAY + 12_345,
        ];
        for start in starts {
            for secs in durations {
                let mut ticked = rtc(start);
                for _ in 0..secs {
                    ticked.tick();
                }
                let mut added = rtc(start);
                added.add_seconds(secs);
                assert_eq!(added.real(), ticked.real(), "{start:?} + {secs} s");
            }
        }
    }

    #[test]
    fn day_carry_only_when_the_9_bit_counter_overflows() {
        let mut day_255 = rtc([0, 0, 0, 255, 0]);
        day_255.add_seconds(SECONDS_PER_DAY);
        assert_eq!(day_255.real(), [0, 0, 0, 0, CONTROL_DAY_HIGH_B], "day 256");

        let mut day_511 = rtc([0, 0, 0, 255, CONTROL_DAY_HIGH_B]);
        day_511.add_seconds(SECONDS_PER_DAY);
        assert_eq!(day_511.real(), [0, 0, 0, 0, CONTROL_CARRY_B], "day 512");
    }

    #[test]
    fn add_seconds_keeps_a_halted_clock_and_the_latched_registers() {
        let halted = [1, 2, 3, 4, CONTROL_HALT_B];
        let mut rtc = rtc(halted);
        rtc.set_latched([5, 6, 7, 8, 0]);
        rtc.add_seconds(1000);
        assert_eq!(rtc.real(), halted, "halted");
        assert_eq!(rtc.latched(), [5, 6, 7, 8, 0], "latched");
    }
}
