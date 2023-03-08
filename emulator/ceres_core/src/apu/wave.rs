use {
    super::{LengthTimer, WaveLength},
    crate::apu::PHalf,
};

const RAM_LEN: u8 = 0x10;
const SAMPLE_LEN: u8 = RAM_LEN * 2;

#[derive(Default)]
pub(super) struct Wave {
    ltim: LengthTimer<0x100>,
    wl: WaveLength<2, ()>,

    on: bool,
    dac_on: bool,
    sample_buf: u8,
    ram: [u8; RAM_LEN as usize],
    samples: [u8; SAMPLE_LEN as usize],
    sample_idx: u8,
    vol: u8,
    nr30: u8,
}

impl Wave {
    pub(super) const fn read_wave_ram(&self, addr: u8) -> u8 {
        let index = addr - 0x30;
        self.ram[index as usize]
    }

    pub(super) fn write_wave_ram(&mut self, addr: u8, val: u8) {
        let index = addr - 0x30;
        self.ram[index as usize] = val;
        // upper 4 bits first
        self.samples[index as usize * 2] = val >> 4;
        self.samples[index as usize * 2 + 1] = val & 0xF;
    }

    pub(super) const fn read_nr30(&self) -> u8 {
        self.nr30 | 0x7F
    }

    pub(super) const fn read_nr32(&self) -> u8 {
        0x9F | (self.vol << 5)
    }

    pub(super) fn read_nr34(&self) -> u8 {
        0xBF | self.ltim.read_on()
    }

    pub(super) fn write_nr30(&mut self, val: u8) {
        self.nr30 = val;
        if val & 0x80 == 0 {
            self.on = false;
            self.dac_on = false;
        } else {
            self.dac_on = true;
        }
    }

    pub(super) fn write_nr31(&mut self, val: u8) {
        self.ltim.write_len(val);
    }

    pub(super) fn write_nr32(&mut self, val: u8) {
        self.vol = (val >> 5) & 3;
    }

    pub(super) fn write_nr33(&mut self, val: u8) {
        self.wl.write_low(val);
    }

    pub(super) fn write_nr34(&mut self, val: u8) {
        self.wl.write_high(val);
        self.ltim.write_on(val, &mut self.on);

        // trigger
        if val & 0x80 != 0 {
            if self.dac_on {
                self.on = true;
            }

            self.ltim.trigger(&mut self.on);
            self.wl.trigger(&mut self.on);
            self.sample_idx = 0;
        }
    }

    pub(super) fn out(&self) -> u8 {
        // wrapping_shr is necessary because (vol - 1) can be -1
        self.sample_buf
            .wrapping_shr(u32::from(self.vol.wrapping_sub(1)))
    }

    pub(super) fn step_sample(&mut self, cycles: i32) {
        if !self.on() {
            return;
        }

        if self.wl.step(cycles) {
            self.sample_idx = (self.sample_idx + 1) & (SAMPLE_LEN - 1);
            self.sample_buf = self.samples[self.sample_idx as usize];
        }
    }

    pub(super) const fn true_on(&self) -> bool {
        self.on && self.dac_on
    }

    pub(super) fn step_len(&mut self) {
        self.ltim.step(&mut self.on);
    }

    pub(super) fn set_period_half(&mut self, p_half: PHalf) {
        self.ltim.set_phalf(p_half);
    }

    pub(super) fn reset(&mut self) {
        self.vol = 0;
        self.on = false;
        self.dac_on = false;
        self.ltim = LengthTimer::default();
        self.sample_buf = 0;
        self.samples = [0; SAMPLE_LEN as usize];
        self.sample_idx = 0;
        self.nr30 = 0;
        self.wl = WaveLength::default();
    }

    pub(super) const fn on(&self) -> bool {
        self.on
    }
}
