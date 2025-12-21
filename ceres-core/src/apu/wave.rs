use {
    super::{
        LengthTimer, PeriodCounter,
        length_timer::LengthTimerCalculationResult,
        period_counter::{PeriodStepResult, PeriodTriggerResult},
    },
    crate::apu::PeriodHalf,
};

const RAM_LEN: u8 = 0x10;
const SAMPLE_LEN: u8 = RAM_LEN * 2;

#[derive(Default)]
pub struct Wave {
    dac_enabled: bool,
    enabled: bool,
    length_timer: LengthTimer<0xFF>,
    nr30: u8,
    period_counter: PeriodCounter<2, ()>,
    ram: [u8; RAM_LEN as usize],
    sample_buffer: u8,
    sample_index: u8,
    samples: [u8; SAMPLE_LEN as usize],
    volume: u8,
    wave_form_just_read: bool,
}

impl Wave {
    pub const fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub const fn is_truly_enabled(&self) -> bool {
        self.enabled && self.dac_enabled
    }

    pub const fn output(&self) -> u8 {
        // wrapping_shr is necessary because (vol - 1) can be -1
        self.sample_buffer
            .wrapping_shr(self.volume.wrapping_sub(1) as u32)
    }

    pub const fn read_nr30(&self) -> u8 {
        self.nr30 | 0x7F
    }

    pub const fn read_nr32(&self) -> u8 {
        0x9F | (self.volume << 5)
    }

    pub const fn read_nr34(&self) -> u8 {
        0xBF | self.length_timer.read_enabled()
    }

    pub const fn length(&self) -> u8 {
        self.length_timer.length()
    }

    pub const fn set_length(&mut self, val: u8) {
        self.length_timer.set_length(val);
    }

    pub const fn read_wave_ram(&self, addr: u8, is_cgb: bool) -> u8 {
        let index = if self.enabled {
            if !is_cgb && !self.wave_form_just_read {
                return 0xFF;
            }
            self.sample_index / 2
        } else {
            addr - 0x30
        };

        self.ram[index as usize]
    }

    // Necessary because powering off the APU doesn't clear the wave RAM
    pub fn reset(&mut self) {
        let ram = self.ram;
        *self = Self::default();
        self.ram = ram;
    }

    pub const fn set_period_half(&mut self, p_half: PeriodHalf) {
        self.length_timer.set_phalf(p_half);
    }

    pub const fn step_length_timer(&mut self) {
        if matches!(
            self.length_timer.step(),
            LengthTimerCalculationResult::DisableChannel
        ) {
            self.enabled = false;
        }
    }

    pub fn step_sample(&mut self, dots: i32) -> Option<i32> {
        if !self.is_enabled() {
            return None;
        }

        if let PeriodStepResult::AdvanceFrequency(offset) = self.period_counter.step(dots) {
            self.sample_index = (self.sample_index + 1) & (SAMPLE_LEN - 1);
            self.sample_buffer = self.samples[self.sample_index as usize];
            self.wave_form_just_read = true;
            Some(offset)
        } else {
            self.wave_form_just_read = false;
            None
        }
    }

    const fn write_ram_direct(&mut self, index: u8, val: u8) {
        self.ram[index as usize] = val;
        // upper 4 bits first
        self.samples[index as usize * 2] = val >> 4;
        self.samples[index as usize * 2 + 1] = val & 0xF;
    }

    pub const fn write_nr30(&mut self, val: u8) {
        self.nr30 = val;
        if val & 0x80 == 0 {
            self.enabled = false;
            self.dac_enabled = false;
        } else {
            self.dac_enabled = true;
        }
    }

    pub const fn write_nr31(&mut self, val: u8) {
        self.length_timer.write_len(val);
    }

    pub const fn write_nr32(&mut self, val: u8) {
        self.volume = (val >> 5) & 3;
    }

    pub fn write_nr33(&mut self, val: u8) {
        self.period_counter.write_low(val);
    }

    pub fn write_nr34(&mut self, val: u8, is_cgb: bool) {
        self.period_counter.write_high(val);

        if matches!(
            self.length_timer.write_enabled(val),
            LengthTimerCalculationResult::DisableChannel
        ) {
            self.enabled = false;
        }

        // trigger
        if val & 0x80 != 0 {
            if !is_cgb && self.enabled && self.period_counter.timer() <= 4 {
                let offset = self.sample_index.div_ceil(2) & 0xF;
                if offset < 4 {
                    self.write_ram_direct(0, self.ram[offset as usize]);
                } else {
                    let base = (offset & !3) as usize;
                    for i in 0..4 {
                        self.write_ram_direct(i as u8, self.ram[base + i]);
                    }
                }
            }

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
                self.period_counter.trigger(),
                PeriodTriggerResult::DisableChannel
            ) {
                self.enabled = false;
            }

            self.sample_index = 0;
        }
    }

    pub const fn write_wave_ram(&mut self, addr: u8, val: u8, is_cgb: bool) {
        let index = if self.enabled {
            if !is_cgb && !self.wave_form_just_read {
                return;
            }
            self.sample_index / 2
        } else {
            addr - 0x30
        };

        self.ram[index as usize] = val;
        // upper 4 bits first
        self.samples[index as usize * 2] = val >> 4;
        self.samples[index as usize * 2 + 1] = val & 0xF;
    }
}
