use core::num::NonZeroU8;

pub(super) trait SweepTrait: Sized + Default {
    fn read(&self) -> u8;
    fn write(&mut self, val: u8);
    fn step(&mut self, freq: &mut u16, on: &mut bool);
    fn trigger(&mut self, freq: &mut u16, on: &mut bool);
}

#[derive(Clone, Copy, Default)]
enum SweepDirection {
    #[default]
    Add = 0,
    Sub = 1,
}

impl SweepDirection {
    const fn from_u8(val: u8) -> Self {
        if val & 8 == 0 {
            Self::Add
        } else {
            Self::Sub
        }
    }

    const fn to_u8(self) -> u8 {
        (self as u8) << 3
    }
}

pub(super) struct Sweep {
    // TODO: check on behaviour
    on: bool,
    dir: SweepDirection,

    // between 0 and 7
    pace: u8,
    // 0 is treated as 8
    shpc: NonZeroU8,
    // shift between 0 and 7
    shift: u8,
    timer: u8,
    // Shadow frequency
    shwf: u16,
}

impl Sweep {
    fn calculate_sweep(&mut self, freq: &mut u16, on: &mut bool) {
        {
            let t = self.shwf >> self.shift;
            self.shwf = match self.dir {
                SweepDirection::Sub => self.shwf - t,
                SweepDirection::Add => self.shwf + t,
            };
        }

        if self.shwf > 0x7FF {
            *on = false;
            return;
        }

        if self.shift != 0 {
            *freq = self.shwf & 0x7FF;
        }
    }
}

impl SweepTrait for Sweep {
    fn read(&self) -> u8 {
        0x80 | (self.pace << 4) | self.dir.to_u8() | self.shift
    }

    fn write(&mut self, val: u8) {
        self.pace = (val >> 4) & 7;

        if self.pace == 0 {
            self.on = false;
        }

        if self.pace != 0 && self.shpc.get() == 8 {
            self.shpc = NonZeroU8::new(self.pace).unwrap();
        }

        self.dir = SweepDirection::from_u8(val);
        self.shift = val & 7;
    }

    fn step(&mut self, freq: &mut u16, on: &mut bool) {
        if !self.on {
            return;
        }

        self.timer += 1;
        if self.timer > self.shpc.get() {
            self.timer = 0;
            self.calculate_sweep(freq, on);
            self.shpc = NonZeroU8::new(if self.pace == 0 { 8 } else { self.pace }).unwrap();
        }
    }

    fn trigger(&mut self, freq: &mut u16, on: &mut bool) {
        self.shwf = *freq;
        self.timer = 0;
        self.on = self.pace > 0 && self.shift != 0;
        self.shpc = NonZeroU8::new(if self.pace == 0 { 8 } else { self.pace }).unwrap();

        if self.shift != 0 {
            self.calculate_sweep(freq, on);
        }
    }
}

impl Default for Sweep {
    fn default() -> Self {
        Self {
            shpc: NonZeroU8::new(8).unwrap(),
            pace: 0,
            dir: SweepDirection::default(),
            shift: 0,
            timer: 0,
            shwf: 0,
            on: false,
        }
    }
}

impl SweepTrait for () {
    fn read(&self) -> u8 {
        0xFF
    }

    fn write(&mut self, _: u8) {}

    fn step(&mut self, _: &mut u16, _: &mut bool) {}

    fn trigger(&mut self, _: &mut u16, _: &mut bool) {}
}
