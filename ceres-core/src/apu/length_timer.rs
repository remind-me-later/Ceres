use super::PeriodHalf;

pub enum LengthTimerCalculationResult {
    DisableChannel,
    None,
}

// LEN_MASK is the maximum length of the timer, 0x3F for all channels except wave, which is 0xFF
#[derive(Default, Debug)]
pub struct LengthTimer<const LENGTH_TIMER_MASK: u8> {
    enabled: bool,
    length: u8,
    period_half: PeriodHalf,
    carry: bool,
}

impl<const LENGTH_TIMER_MASK: u8> LengthTimer<LENGTH_TIMER_MASK> {
    pub const fn read_enabled(&self) -> u8 {
        (self.enabled as u8) << 6
    }

    pub const fn write_enabled(&mut self, val: u8) -> LengthTimerCalculationResult {
        let was_disabled = !self.enabled;
        self.enabled = val & 0x40 != 0;

        if was_disabled && matches!(self.period_half, PeriodHalf::First) {
            self.step()
        } else {
            LengthTimerCalculationResult::None
        }
    }

    pub const fn write_len(&mut self, val: u8) {
        self.length = val & LENGTH_TIMER_MASK;
        self.carry = false;
    }

    pub const fn trigger(&mut self) -> LengthTimerCalculationResult {
        if self.carry {
            self.length = 0;
            self.carry = false;
            if matches!(self.period_half, PeriodHalf::First) {
                return self.step();
            }
        }

        LengthTimerCalculationResult::None
    }

    pub const fn step(&mut self) -> LengthTimerCalculationResult {
        if self.enabled {
            if self.length == LENGTH_TIMER_MASK {
                self.carry = true;
                return LengthTimerCalculationResult::DisableChannel;
            }
            self.length += 1;
        }

        LengthTimerCalculationResult::None
    }

    pub const fn set_phalf(&mut self, p_half: PeriodHalf) {
        self.period_half = p_half;
    }
}
