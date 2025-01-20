use core::num::NonZeroU8;

pub(super) trait SweepTrait: Default {
    fn read(&self) -> u8;
    fn write(&mut self, val: u8);
    fn step(&mut self) -> SweepCalculationResult;
    fn trigger(&mut self, period: u16) -> SweepCalculationResult;
}

#[derive(Clone, Copy, Default, Debug)]
enum SweepDirection {
    #[default]
    Add = 0,
    Sub = 1,
}

impl From<u8> for SweepDirection {
    fn from(val: u8) -> Self {
        if val & 8 == 0 {
            Self::Add
        } else {
            Self::Sub
        }
    }
}

impl From<SweepDirection> for u8 {
    fn from(val: SweepDirection) -> Self {
        (val as Self) << 3
    }
}

#[derive(Debug)]
pub(super) enum SweepCalculationResult {
    DisableChannel,
    UpdatePeriod { period: u16 },
    None,
}

#[derive(Debug)]
pub(super) struct Sweep {
    // TODO: check on behaviour
    enabled: bool,
    dir: SweepDirection,

    // between 0 and 7
    pace: u8,
    // 0 is treated as 8
    shadow_pace: NonZeroU8,
    // shift between 0 and 7
    individual_step: u8,
    timer: u8,
    // between 0 and 0x7FF
    shadow_register: u16,
}

impl Sweep {
    fn calculate_sweep(&mut self) -> SweepCalculationResult {
        let t = self.shadow_register >> self.individual_step;

        self.shadow_register = match self.dir {
            SweepDirection::Sub => self.shadow_register - t,
            SweepDirection::Add => self.shadow_register + t,
        };

        if self.shadow_register > 0x7FF {
            SweepCalculationResult::DisableChannel
        } else {
            SweepCalculationResult::UpdatePeriod {
                period: self.shadow_register & 0x7FF,
            }
        }
    }
}

impl SweepTrait for Sweep {
    fn read(&self) -> u8 {
        0x80 | (self.pace << 4) | u8::from(self.dir) | self.individual_step
    }

    fn write(&mut self, val: u8) {
        self.pace = (val >> 4) & 7;

        if self.pace == 0 {
            self.enabled = false;
        }

        if self.pace != 0 && self.shadow_pace.get() == 8 {
            self.shadow_pace = NonZeroU8::new(self.pace).unwrap();
        }

        self.dir = SweepDirection::from(val);
        self.individual_step = val & 7;
    }

    fn step(&mut self) -> SweepCalculationResult {
        if !self.enabled {
            return SweepCalculationResult::None;
        }

        self.timer += 1;
        if self.timer >= self.shadow_pace.get() {
            self.timer = 0;
            self.shadow_pace = NonZeroU8::new(if self.pace == 0 { 8 } else { self.pace }).unwrap();

            if self.pace == 0 {
                SweepCalculationResult::None
            } else {
                self.calculate_sweep()
            }
        } else {
            SweepCalculationResult::None
        }
    }

    fn trigger(&mut self, period: u16) -> SweepCalculationResult {
        self.shadow_register = period;
        self.timer = 0;
        // restart
        self.enabled = self.pace != 0 || self.individual_step != 0;

        self.shadow_pace = NonZeroU8::new(if self.pace == 0 { 8 } else { self.pace }).unwrap();

        if self.individual_step != 0 {
            self.calculate_sweep()
        } else {
            SweepCalculationResult::None
        }
    }
}

impl Default for Sweep {
    fn default() -> Self {
        Self {
            shadow_pace: NonZeroU8::new(8).unwrap(),
            pace: 0,
            dir: SweepDirection::default(),
            individual_step: 0,
            timer: 0,
            shadow_register: 0,
            enabled: false,
        }
    }
}

impl SweepTrait for () {
    fn read(&self) -> u8 {
        0xFF
    }

    fn write(&mut self, _: u8) {}

    fn step(&mut self) -> SweepCalculationResult {
        SweepCalculationResult::None
    }

    fn trigger(&mut self, _: u16) -> SweepCalculationResult {
        SweepCalculationResult::None
    }
}
