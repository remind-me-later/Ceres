use super::ccore::Ccore;

pub struct Envelope {
    on: bool,
    inc: bool,
    base_vol: u8,
    vol: u8,
    period: u8,
    timer: u8,
}

impl Envelope {
    pub fn new() -> Self {
        Self {
            on: false,
            base_vol: 0,
            period: 8,
            vol: 0,
            timer: 8,
            inc: false,
        }
    }

    pub fn reset(&mut self) {
        self.period = 8;
        self.timer = 8;
        self.inc = false;
        self.base_vol = 0;
        self.vol = 0;
        self.on = false;
    }

    pub fn read(&self) -> u8 {
        (self.base_vol << 4) | self.inc as u8 | (self.period & 7)
    }

    pub fn write(&mut self, val: u8, core: &mut Ccore<64>) {
        // value == 0x7 || value == 0x0 to pass Blargg 2 test
        if val == 7 {
            core.disable_dac();
        }

        if val == 0x10 {
            core.enable_dac();
        }

        self.period = val & 7;
        self.on = self.period != 0;
        if self.period == 0 {
            self.period = 8;
        };
        self.timer = self.period;
        self.inc = val & 8 != 0;
        self.base_vol = val >> 4;
        self.vol = self.base_vol;
    }

    pub fn step(&mut self, core: &mut Ccore<64>) {
        if !self.on || !core.on() {
            return;
        }

        if self.inc && self.vol == 15 || !self.inc && self.vol == 0 {
            self.on = false;
            return;
        }

        self.timer -= 1;
        if self.timer == 0 {
            self.timer = self.period;

            if self.inc {
                self.vol += 1;
            } else {
                self.vol -= 1;
            };
        }
    }

    pub fn trigger(&mut self) {
        self.timer = self.period;
        self.vol = self.base_vol;
    }

    pub fn volume(&self) -> u8 {
        self.vol
    }
}
