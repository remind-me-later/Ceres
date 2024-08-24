use {
    super::{
        length_timer::LengthTimerCalculationResult, wave_length::WaveLengthCalculationResult,
        LengthTimer, WaveLength,
    },
    crate::apu::PeriodHalf,
};

const RAM_LEN: u8 = 0x10;
const SAMPLE_LEN: u8 = RAM_LEN * 2;

#[derive(Default)]
pub(super) struct Wave {
    length_timer: LengthTimer<0xFF>,
    wave_length: WaveLength<2, ()>,

    enabled: bool,
    dac_enabled: bool,
    sample_buffer: u8,
    ram: [u8; RAM_LEN as usize],
    samples: [u8; SAMPLE_LEN as usize],
    sample_index: u8,
    volume: u8,
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
        0x9F | (self.volume << 5)
    }

    pub(super) fn read_nr34(&self) -> u8 {
        0xBF | self.length_timer.read_enabled()
    }

    pub(super) fn write_nr30(&mut self, val: u8) {
        self.nr30 = val;
        if val & 0x80 == 0 {
            self.enabled = false;
            self.dac_enabled = false;
        } else {
            self.dac_enabled = true;
        }
    }

    pub(super) fn write_nr31(&mut self, val: u8) {
        self.length_timer.write_len(val);
    }

    pub(super) fn write_nr32(&mut self, val: u8) {
        self.volume = (val >> 5) & 3;
    }

    pub(super) fn write_nr33(&mut self, val: u8) {
        self.wave_length.write_low(val);
    }

    pub(super) fn write_nr34(&mut self, val: u8) {
        self.wave_length.write_high(val);

        if matches!(
            self.length_timer.write_enabled(val),
            LengthTimerCalculationResult::DisableChannel
        ) {
            self.enabled = false;
        }

        // trigger
        if val & 0x80 != 0 {
            if self.dac_enabled {
                self.enabled = true;
            }

            if matches!(
                self.length_timer.trigger(),
                LengthTimerCalculationResult::DisableChannel
            ) {
                self.enabled = false;
            }

            if matches!(
                self.wave_length.trigger(),
                WaveLengthCalculationResult::DisableChannel
            ) {
                self.enabled = false;
            }

            self.sample_index = 0;
        }
    }

    pub(super) const fn output(&self) -> u8 {
        // wrapping_shr is necessary because (vol - 1) can be -1
        self.sample_buffer
            .wrapping_shr(self.volume.wrapping_sub(1) as u32)
    }

    pub(super) fn step_sample(&mut self, cycles: i32) {
        if !self.enabled() {
            return;
        }

        if self.wave_length.step(cycles) {
            self.sample_index = (self.sample_index + 1) & (SAMPLE_LEN - 1);
            self.sample_buffer = self.samples[self.sample_index as usize];
        }
    }

    pub(super) const fn true_on(&self) -> bool {
        self.enabled && self.dac_enabled
    }

    pub(super) fn step_length_timer(&mut self) {
        if matches!(
            self.length_timer.step(),
            LengthTimerCalculationResult::DisableChannel
        ) {
            self.enabled = false;
        }
    }

    pub(super) fn set_period_half(&mut self, p_half: PeriodHalf) {
        self.length_timer.set_phalf(p_half);
    }

    pub(super) const fn enabled(&self) -> bool {
        self.enabled
    }

    // Necessary because powering off the APU doesn't clear the wave RAM
    pub(super) fn reset(&mut self) {
        let ram = self.ram;
        *self = Default::default();
        self.ram = ram;
    }
}
