use core::num::NonZeroU8;

pub trait SweepTrait: Default {
    fn read(&self) -> u8;
    fn step(&mut self) -> SweepCalculationResult;
    fn trigger(&mut self, period: u16) -> SweepCalculationResult;
    fn write(&mut self, val: u8) -> SweepCalculationResult;
}

#[derive(Clone, Copy, Default, PartialEq)]
enum SweepDirection {
    #[default]
    Add,
    Sub,
}

impl From<u8> for SweepDirection {
    fn from(val: u8) -> Self {
        if val & 8 == 0 { Self::Add } else { Self::Sub }
    }
}

impl From<SweepDirection> for u8 {
    #[inline]
    fn from(val: SweepDirection) -> Self {
        match val {
            SweepDirection::Add => 0,
            SweepDirection::Sub => 8,
        }
    }
}

pub enum SweepCalculationResult {
    DisableChannel,
    None,
    UpdatePeriod { period: u16 },
    UpdatePeriodAndDisable { period: u16 },
}

pub struct Sweep {
    dir: SweepDirection,
    enabled: bool,          // TODO: check on behaviour
    individual_step: u8,    // shift between 0 and 7
    pace: u8,               // 3 bits
    shadow_pace: NonZeroU8, // 0 is treated as 8
    shadow_register: u16,   // between 0 and 0x7FF
    timer: u8,
    last_delta: u16,
}

impl Sweep {
    const fn calculate_sweep(&self) -> (u16, u16) {
        let t = self.shadow_register >> self.individual_step;

        let (new_freq, delta) = match self.dir {
            SweepDirection::Sub => (self.shadow_register - t, t ^ 0x7FF),
            SweepDirection::Add => (self.shadow_register + t, t),
        };

        (new_freq, delta)
    }
}

impl SweepTrait for Sweep {
    fn read(&self) -> u8 {
        0x80 | (self.pace << 4) | u8::from(self.dir) | self.individual_step
    }

    fn step(&mut self) -> SweepCalculationResult {
        if !self.enabled {
            return SweepCalculationResult::None;
        }

        self.timer += 1;
        if self.timer >= self.shadow_pace.get() {
            self.timer = 0;
            #[expect(clippy::unwrap_used)]
            {
                self.shadow_pace =
                    NonZeroU8::new(if self.pace == 0 { 8 } else { self.pace }).unwrap();
            }

                        if self.pace == 0 {
                            SweepCalculationResult::None
                        } else {
                            let (new_val, delta) = self.calculate_sweep();
                            self.last_delta = delta;
            
                            if new_val > 0x7FF {
                                SweepCalculationResult::DisableChannel
                            } else if self.individual_step != 0 {
                                self.shadow_register = new_val;
            
                                let (next_val, _) = self.calculate_sweep();
                                if next_val > 0x7FF {
                                    SweepCalculationResult::UpdatePeriodAndDisable {
                                        period: self.shadow_register & 0x7FF,
                                    }
                                } else {
                                    SweepCalculationResult::UpdatePeriod {
                                        period: self.shadow_register & 0x7FF,
                                    }
                                }
                            } else {
                                SweepCalculationResult::None
                            }
                        }        } else {
            SweepCalculationResult::None
        }
    }

    fn trigger(&mut self, period: u16) -> SweepCalculationResult {
        self.shadow_register = period;
        self.timer = 0;
        // restart
        self.enabled = self.pace != 0 || self.individual_step != 0;

        #[expect(clippy::unwrap_used)]
        {
            self.shadow_pace = NonZeroU8::new(if self.pace == 0 { 8 } else { self.pace }).unwrap();
        }

        if self.individual_step != 0 {
            let (val, delta) = self.calculate_sweep();
            self.last_delta = delta;
            if val > 0x7FF {
                SweepCalculationResult::DisableChannel
            } else {
                SweepCalculationResult::None
            }
        } else {
            SweepCalculationResult::None
        }
    }

    fn write(&mut self, val: u8) -> SweepCalculationResult {
        let old_dir = self.dir;
        self.pace = (val >> 4) & 7;
        self.dir = SweepDirection::from(val);
        self.individual_step = val & 7;

        if old_dir == SweepDirection::Sub && self.dir == SweepDirection::Add {
            // "Exiting negate mode after calculation disables channel"
            // The check essentially sees if the *subtracted* value would have caused an overflow
            // if it had been *added* (plus the negate bit interaction).
            // shadow + last_delta + 1 (because old_negate was 1)
            // Note: strict check for > 0x7FF
            if self.shadow_register + self.last_delta + 1 > 0x7FF {
                self.enabled = false;
                return SweepCalculationResult::DisableChannel;
            }
        }

        SweepCalculationResult::None
    }
}

impl Default for Sweep {
    fn default() -> Self {
        #[expect(clippy::unwrap_used)]
        Self {
            shadow_pace: NonZeroU8::new(8).unwrap(),
            pace: 0,
            dir: SweepDirection::default(),
            individual_step: 0,
            timer: 0,
            shadow_register: 0,
            enabled: false,
            last_delta: 0,
        }
    }
}

impl SweepTrait for () {
    fn read(&self) -> u8 {
        0xFF
    }

    fn step(&mut self) -> SweepCalculationResult {
        SweepCalculationResult::None
    }

    fn trigger(&mut self, _: u16) -> SweepCalculationResult {
        SweepCalculationResult::None
    }

    fn write(&mut self, _: u8) -> SweepCalculationResult {
        SweepCalculationResult::None
    }
}
