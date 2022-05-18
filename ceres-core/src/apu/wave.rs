use super::common::{Common, Freq};

const WAV_MAX_LENGTH: u16 = 0x100;
const WAV_RAM_SIZE: u8 = 0x10;
const WAV_SAMPLE_SIZE: u8 = WAV_RAM_SIZE * 2;
const WAV_PERIOD_MUL: u16 = 2;

pub struct Wave {
    com: Common<WAV_MAX_LENGTH>,
    freq: Freq<WAV_PERIOD_MUL>,
    period: u16,
    sample_buffer: u8,
    ram: [u8; WAV_RAM_SIZE as usize],
    samples: [u8; WAV_SAMPLE_SIZE as usize],
    sample_idx: u8,
    vol: u8,
    nr30: u8,
}

impl Wave {
    pub fn new() -> Self {
        let freq: Freq<WAV_PERIOD_MUL> = Freq::new();
        let period = freq.period();

        Self {
            ram: [0; WAV_RAM_SIZE as usize],
            vol: 0,
            com: Common::new(),
            freq,
            period,
            sample_buffer: 0,
            samples: [0; WAV_SAMPLE_SIZE as usize],
            sample_idx: 0,
            nr30: 0,
        }
    }

    pub fn reset(&mut self) {
        self.com.reset();
        self.freq.reset();
        self.vol = 0;
        self.sample_idx = 0;
        self.period = 0;
        self.sample_buffer = 0;
        self.nr30 = 0;
    }

    pub fn read_wave_ram(&self, addr: u8) -> u8 {
        let index = (addr - 0x30) as usize;
        self.ram[index]
    }

    pub fn write_wave_ram(&mut self, addr: u8, val: u8) {
        let index = (addr - 0x30) as usize;
        self.ram[index] = val;
        // upper 4 bits first
        self.samples[index * 2] = val >> 4;
        self.samples[index * 2 + 1] = val & 0xf;
    }

    pub fn read_nr30(&self) -> u8 {
        self.nr30 | 0x7f
    }

    pub fn read_nr32(&self) -> u8 {
        0x9f | (self.vol << 5)
    }

    pub fn read_nr34(&self) -> u8 {
        self.com.read()
    }

    pub fn write_nr30(&mut self, val: u8) {
        self.nr30 = val;
        if val & (1 << 7) == 0 {
            self.com.disable_dac();
        } else {
            self.com.enable_dac();
        }
    }

    pub fn write_nr31(&mut self, val: u8) {
        self.com.write_len(val);
    }

    pub fn write_nr32(&mut self, val: u8) {
        self.vol = (val >> 5) & 3;
    }

    pub fn write_nr33(&mut self, val: u8) {
        self.freq.set_lo(val);
    }

    pub fn write_nr34(&mut self, val: u8) {
        self.freq.set_hi(val);
        if self.com.write_control(val) {
            self.trigger();
        }
    }

    pub fn trigger(&mut self) {
        self.period = self.freq.period();
        self.sample_idx = 0;
    }

    pub fn step_sample(&mut self) {
        if !self.on() {
            return;
        }

        if self.period == 0 {
            self.period = self.freq.period();
            self.sample_idx = (self.sample_idx + 1) % WAV_SAMPLE_SIZE;
            self.sample_buffer = self.samples[self.sample_idx as usize];
        }

        self.period -= 1;
    }

    pub fn out(&self) -> u8 {
        self.sample_buffer >> (self.vol.wrapping_sub(1) & 7)
    }

    pub fn on(&self) -> bool {
        self.com.on()
    }

    pub fn step_len(&mut self) {
        self.com.step_len();
    }

    pub fn mut_com(&mut self) -> &mut Common<WAV_MAX_LENGTH> {
        &mut self.com
    }
}
