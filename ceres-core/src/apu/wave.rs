use super::{ccore::Ccore, freq::Freq};

const MAX_WAVE_CHANNEL_LENGTH: u16 = 0x100;
const WAVE_RAM_SIZE: u8 = 0x10;
const WAVE_SAMPLE_SIZE: u8 = WAVE_RAM_SIZE * 2;
const WAVE_CHANNEL_PERIOD_MULTIPLIER: u16 = 2;

pub struct Wave {
    core: Ccore<MAX_WAVE_CHANNEL_LENGTH>,
    frequency_data: Freq<WAVE_CHANNEL_PERIOD_MULTIPLIER>,
    current_frequency_period: u16,
    sample_buffer: u8,
    wave_ram: [u8; WAVE_RAM_SIZE as usize],
    wave_samples: [u8; WAVE_SAMPLE_SIZE as usize],
    wave_sample_index: u8,
    vol: u8,
    nr30: u8,
}

impl Wave {
    pub fn new() -> Self {
        let frequency_data: Freq<WAVE_CHANNEL_PERIOD_MULTIPLIER> = Freq::new();
        let current_frequency_period = frequency_data.period();

        Self {
            wave_ram: [0; WAVE_RAM_SIZE as usize],
            vol: 0,
            core: Ccore::new(),
            frequency_data,
            current_frequency_period,
            sample_buffer: 0,
            wave_samples: [0; WAVE_SAMPLE_SIZE as usize],
            wave_sample_index: 0,
            nr30: 0,
        }
    }

    pub fn reset(&mut self) {
        self.core.reset();
        self.frequency_data.reset();
        self.vol = 0;
        self.wave_sample_index = 0;
        self.current_frequency_period = 0;
        self.sample_buffer = 0;
        self.nr30 = 0;
    }

    pub fn read_wave_ram(&self, addr: u8) -> u8 {
        let index = (addr - 0x30) as usize;
        self.wave_ram[index]
    }

    pub fn write_wave_ram(&mut self, addr: u8, val: u8) {
        let index = (addr - 0x30) as usize;
        self.wave_ram[index] = val;
        // upper 4 bits first
        self.wave_samples[index * 2] = val >> 4;
        self.wave_samples[index * 2 + 1] = val & 0xf;
    }

    pub fn read_nr30(&self) -> u8 {
        self.nr30 | 0x7f
    }

    pub fn read_nr32(&self) -> u8 {
        0x9f | (self.vol << 5)
    }

    pub fn read_nr34(&self) -> u8 {
        self.core.read()
    }

    pub fn write_nr30(&mut self, val: u8) {
        self.nr30 = val;
        if val & (1 << 7) == 0 {
            self.core.disable_dac();
        } else {
            self.core.enable_dac();
        }
    }

    pub fn write_nr31(&mut self, val: u8) {
        self.core.write_len(val);
    }

    pub fn write_nr32(&mut self, val: u8) {
        self.vol = (val >> 5) & 3;
    }

    pub fn write_nr33(&mut self, val: u8) {
        self.frequency_data.set_lo(val);
    }

    pub fn write_nr34(&mut self, val: u8) {
        self.frequency_data.set_hi(val);
        if self.core.write_control(val) {
            self.trigger();
        }
    }

    pub fn trigger(&mut self) {
        self.current_frequency_period = self.frequency_data.period();
        self.wave_sample_index = 0;
    }

    pub fn step_sample(&mut self) {
        if !self.on() {
            return;
        }

        if self.current_frequency_period == 0 {
            self.current_frequency_period = self.frequency_data.period();
            self.wave_sample_index = (self.wave_sample_index + 1) % WAVE_SAMPLE_SIZE;
            self.sample_buffer = self.wave_samples[self.wave_sample_index as usize];
        }

        self.current_frequency_period -= 1;
    }

    pub fn out(&self) -> u8 {
        self.sample_buffer >> (self.vol.wrapping_sub(1) & 7)
    }

    pub fn on(&self) -> bool {
        self.core.on()
    }

    pub fn step_len(&mut self) {
        self.core.step_len();
    }

    pub fn mut_core(&mut self) -> &mut Ccore<MAX_WAVE_CHANNEL_LENGTH> {
        &mut self.core
    }
}
