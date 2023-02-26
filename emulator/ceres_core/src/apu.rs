use crate::{Gb, TC_SEC};

const APU_TIMER_RES: i32 = (TC_SEC / 512) & 0xFFFF;

const SQ_MAX_LEN: u16 = 64;
const SQ_FREQ_P_MUL: u16 = 4;
const SQ_DUTY_TABLE: [u8; 4] = [0b0000_0001, 0b1000_0001, 0b1000_0111, 0b0111_1110];

const WAV_MAX_LENGTH: u16 = 256;
const WAV_RAM_SIZE: u8 = 0x10;
const WAV_SAMPLE_SIZE: u8 = WAV_RAM_SIZE * 2;
const WAV_PERIOD_MUL: u16 = 2;

const NOISE_MAX_LEN: u16 = 64;

pub type Sample = i16;

impl Gb {
    pub(crate) fn run_apu(&mut self, cycles: i32) {
        if self.apu_on {
            if self.apu_timer > APU_TIMER_RES {
                self.apu_timer -= APU_TIMER_RES;
                self.step_seq();
            }
            self.apu_timer += cycles;

            self.apu_ch1.step_sample(cycles);
            self.apu_ch2.step_sample(cycles);
            self.apu_ch3.step_sample(cycles);
            self.apu_ch4.step_sample(cycles);
        }

        while self.apu_render_timer > self.apu_ext_sample_period {
            self.apu_render_timer -= self.apu_ext_sample_period;

            if self.apu_on {
                let (l, r) = self.mix_and_render();

                self.apu_l_out = l;
                self.apu_r_out = r;
            } else {
                self.apu_l_out = 0;
                self.apu_r_out = 0;
            }

            self.samples_run += 2;
        }

        self.apu_render_timer += cycles;
    }

    fn step_seq(&mut self) {
        if self.apu_seq_step & 1 == 0 {
            self.apu_ch1.step_len();
            self.apu_ch2.step_len();
            self.apu_ch3.step_len();
            self.apu_ch4.step_len();

            self.set_period_half(0);
        } else {
            self.set_period_half(1);
        }

        match self.apu_seq_step {
            2 | 6 => self.apu_ch1.step_sweep(),
            7 => {
                self.apu_ch1.step_envelope();
                self.apu_ch2.step_envelope();
                self.apu_ch4.step_envelope();
            }
            _ => (),
        }

        self.apu_seq_step = (self.apu_seq_step + 1) & 7;
    }

    fn set_period_half(&mut self, p_half: u8) {
        self.apu_ch1.set_period_half(p_half);
        self.apu_ch2.set_period_half(p_half);
        self.apu_ch3.set_period_half(p_half);
        self.apu_ch4.set_period_half(p_half);
    }

    fn mix_and_render(&self) -> (Sample, Sample) {
        let mut l = 0;
        let mut r = 0;

        for i in 0..4 {
            let out = match i {
                0 => self.apu_ch1.out() * u8::from(self.apu_ch1.on()),
                1 => self.apu_ch2.out() * u8::from(self.apu_ch2.on()),
                2 => self.apu_ch3.out() * u8::from(self.apu_ch3.on()),
                3 => self.apu_ch4.out() * u8::from(self.apu_ch4.on()),
                _ => break,
            };

            let right_on = u8::from(self.nr51 & (1 << i) != 0);
            let left_on = u8::from(self.nr51 & (1 << (i + 4)) != 0);

            l += left_on * out;
            r += right_on * out;
        }

        // transform to i16 sample
        let l = (0xF - i16::from(l) * 2) * i16::from(self.apu_l_vol);
        let r = (0xF - i16::from(r) * 2) * i16::from(self.apu_r_vol);

        // amplify
        let l = l * 32;
        let r = r * 32;

        (l, r)
    }

    fn reset(&mut self) {
        self.apu_render_timer = 0;
        self.apu_ch1.reset();
        self.apu_ch2.reset();
        self.apu_ch3.reset();
        self.apu_ch4.reset();
        self.apu_l_vol = 0;
        self.apu_l_vin = false;
        self.apu_r_vol = 0;
        self.apu_r_vin = false;
        self.nr51 = 0;
    }

    #[must_use]
    pub(crate) fn read_nr50(&self) -> u8 {
        self.apu_r_vol
            | u8::from(self.apu_r_vin) << 3
            | self.apu_l_vol << 4
            | u8::from(self.apu_l_vin) << 7
    }

    #[must_use]
    pub(crate) fn read_nr52(&self) -> u8 {
        u8::from(self.apu_on) << 7
            | 0x70
            | u8::from(self.apu_ch4.on) << 3
            | u8::from(self.apu_ch3.on) << 2
            | u8::from(self.apu_ch2.on) << 1
            | u8::from(self.apu_ch1.on)
    }

    pub(crate) fn write_nr50(&mut self, val: u8) {
        if self.apu_on {
            self.apu_r_vol = val & 7;
            self.apu_r_vin = val & 8 != 0;
            self.apu_l_vol = (val >> 4) & 7;
            self.apu_l_vin = val & 0x80 != 0;
        }
    }

    pub(crate) fn write_nr51(&mut self, val: u8) {
        if self.apu_on {
            self.nr51 = val;
        }
    }

    pub(crate) fn write_nr52(&mut self, val: u8) {
        self.apu_on = val & 0x80 != 0;
        if !self.apu_on {
            self.reset();
        }
    }
}

pub struct Square1 {
    on:      bool,
    dac_on:  bool,
    use_len: bool,
    snd_len: u16,
    p_half:  u8, // period half: 0 or 1

    freq:     u16, // 11 bit frequency data
    duty:     u8,
    duty_bit: u8,
    period:   i32,
    out:      u8,

    // envelope
    env_vol:      u8,
    env_on:       bool,
    env_inc:      bool,
    env_base_vol: u8,
    env_period:   u8,
    env_timer:    u8,

    // sweep
    sw_on:          bool,
    sw_dec:         bool,
    sw_period:      u8,
    sw_shift:       u8,
    sw_timer:       u8,
    sw_shadow_freq: u16,
}

impl Default for Square1 {
    fn default() -> Self {
        Self {
            sw_period:      8,
            sw_dec:         false,
            sw_shift:       0,
            sw_timer:       8,
            sw_shadow_freq: 0,
            sw_on:          false,
            freq:           0,
            period:         i32::from(SQ_FREQ_P_MUL) * 2048,
            duty:           SQ_DUTY_TABLE[0],
            duty_bit:       0,
            on:             false,
            dac_on:         true,
            snd_len:        0,
            use_len:        false,
            p_half:         0, // doesn't matter
            out:            0,
            env_on:         false,
            env_base_vol:   0,
            env_period:     8,
            env_vol:        0,
            env_timer:      8,
            env_inc:        false,
        }
    }
}

impl Square1 {
    fn reset(&mut self) {
        self.on = false;
        self.dac_on = true;
        self.snd_len = 0;
        self.use_len = false;
        self.freq = 0;
        self.duty = SQ_DUTY_TABLE[0];
        self.duty_bit = 0;
        self.env_period = 8;
        self.env_timer = 8;
        self.env_inc = false;
        self.env_base_vol = 0;
        self.env_vol = 0;
        self.env_on = false;
        self.sw_period = 8;
        self.sw_timer = 8;
        self.sw_dec = false;
        self.sw_shift = 0;
        self.sw_on = false;
    }

    fn calculate_sweep(&mut self) {
        let tmp = self.sw_shadow_freq >> self.sw_shift;
        self.sw_shadow_freq = if self.sw_dec {
            self.sw_shadow_freq - tmp
        } else {
            self.sw_shadow_freq + tmp
        };

        if self.sw_shadow_freq > 0x7FF {
            self.on = false;
            return;
        }

        if self.sw_shift != 0 {
            self.freq = self.sw_shadow_freq & 0x7FF;
        }
    }

    pub(crate) fn read_nr10(&self) -> u8 {
        0x80 | ((self.sw_period & 7) << 4) | (u8::from(!self.sw_dec) << 3) | self.sw_shift
    }

    pub(crate) const fn read_nr11(&self) -> u8 {
        0x3F | (self.duty << 6)
    }

    pub(crate) fn read_nr12(&self) -> u8 {
        self.env_base_vol << 4 | u8::from(self.env_inc) | self.env_period & 7
    }

    pub(crate) fn read_nr14(&self) -> u8 {
        0xBF | (u8::from(self.use_len) << 6)
    }

    pub(crate) fn write_nr10(&mut self, val: u8) {
        self.sw_period = (val >> 4) & 7;
        if self.sw_period == 0 {
            self.sw_period = 8;
        }
        self.sw_dec = val & 8 != 0;
        self.sw_shift = val & 7;
    }

    pub(crate) fn write_nr11(&mut self, val: u8) {
        self.duty = (val >> 6) & 3;
        self.snd_len = SQ_MAX_LEN - (u16::from(val) & (SQ_MAX_LEN - 1));
    }

    pub(crate) fn write_nr12(&mut self, val: u8) {
        // println!("write nr12: {:x}", val);

        if val & 0xF8 == 0 {
            self.on = false;
            self.dac_on = false;
        } else {
            self.dac_on = true;
        }

        self.env_period = val & 7;
        self.env_on = self.env_period != 0;
        if self.env_period == 0 {
            self.env_period = 8;
        }
        self.env_timer = self.env_period;
        self.env_inc = val & 8 != 0;
        self.env_base_vol = val >> 4;
        self.env_vol = self.env_base_vol;
    }

    pub(crate) fn write_nr13(&mut self, val: u8) {
        self.freq = (self.freq & 0x700) | u16::from(val);
    }

    pub(crate) fn write_nr14(&mut self, val: u8) {
        self.freq = (u16::from(val) & 7) << 8 | (self.freq & 0xFF);

        let old_len = self.use_len;
        self.use_len = val & 0x40 != 0;

        // println!("write nr14: {:x}", val);

        if self.use_len && !old_len && self.p_half == 0 {
            self.step_len();
            // println!("step enabled, len: {}",
            // self.snd_len);
        }

        if val & 0x80 == 0 {
            return;
        }

        // trigger
        // println!("trigger");

        if self.dac_on {
            self.on = true;
        }

        if self.snd_len == 0 {
            // println!("reset len");
            self.snd_len = SQ_MAX_LEN;
            if self.use_len && self.p_half == 0 {
                // println!("freeze step enabled, len: {}", self.snd_len);
                self.step_len();
            }
        }

        self.period = i32::from(SQ_FREQ_P_MUL * (2048 - self.freq));
        self.out = 0;
        // self.duty_bit = 0;
        self.env_timer = self.env_period;
        self.env_vol = self.env_base_vol;

        self.sw_shadow_freq = self.freq;
        self.sw_timer = self.sw_period;
        self.sw_on = self.sw_period != 8 && self.sw_shift != 0;

        if self.sw_shift != 0 {
            self.calculate_sweep();
        }
    }

    fn step_sample(&mut self, cycles: i32) {
        if !(self.on && self.dac_on) {
            return;
        }

        self.period -= cycles;

        if self.period < 0 {
            self.period += i32::from(SQ_FREQ_P_MUL * (2048 - self.freq));
            self.out = (SQ_DUTY_TABLE[self.duty as usize] & (1 << self.duty_bit)) >> self.duty_bit;
            self.duty_bit = (self.duty_bit + 1) & 7;
        }
    }

    fn step_sweep(&mut self) {
        if self.on() {
            self.sw_timer -= 1;
            if self.sw_timer == 0 {
                self.sw_timer = self.sw_period;
                if self.sw_on {
                    self.calculate_sweep();
                }
            }
        }
    }

    fn step_envelope(&mut self) {
        if !(self.env_on && self.on && self.dac_on) {
            return;
        }

        if self.env_inc && self.env_vol == 15 || !self.env_inc && self.env_vol == 0 {
            self.env_on = false;
            return;
        }

        self.env_timer -= 1;
        if self.env_timer == 0 {
            self.env_timer = self.env_period;

            if self.env_inc {
                self.env_vol += 1;
            } else {
                self.env_vol -= 1;
            }
        }
    }

    const fn out(&self) -> u8 {
        self.out * self.env_vol
    }

    const fn on(&self) -> bool {
        self.on && self.dac_on
    }

    fn step_len(&mut self) {
        if self.use_len && self.snd_len > 0 {
            self.snd_len -= 1;

            if self.snd_len == 0 {
                self.on = false;
            }
        }
    }

    fn set_period_half(&mut self, p_half: u8) {
        debug_assert!(p_half == 0 || p_half == 1);
        self.p_half = p_half;
    }
}

pub struct Square2 {
    on:      bool,
    dac_on:  bool,
    use_len: bool,
    snd_len: u16,
    p_half:  u8, // period half: 0 or 1

    freq:     u16, // 11 bit frequency data
    duty:     u8,
    duty_bit: u8,
    period:   i32,
    out:      u8,

    // envelope
    env_vol:      u8,
    env_on:       bool,
    env_inc:      bool,
    env_base_vol: u8,
    env_period:   u8,
    env_timer:    u8,
}

impl Default for Square2 {
    fn default() -> Self {
        Self {
            freq:         0,
            period:       i32::from(SQ_FREQ_P_MUL) * 2048,
            duty:         SQ_DUTY_TABLE[0],
            duty_bit:     0,
            on:           false,
            dac_on:       true,
            snd_len:      0,
            use_len:      false,
            p_half:       0, // doesn't matter
            out:          0,
            env_on:       false,
            env_base_vol: 0,
            env_period:   8,
            env_vol:      0,
            env_timer:    8,
            env_inc:      false,
        }
    }
}

impl Square2 {
    fn reset(&mut self) {
        self.on = false;
        self.dac_on = true;
        self.snd_len = 0;
        self.use_len = false;
        self.freq = 0;
        self.duty = SQ_DUTY_TABLE[0];
        self.duty_bit = 0;
        self.env_period = 8;
        self.env_timer = 8;
        self.env_inc = false;
        self.env_base_vol = 0;
        self.env_vol = 0;
        self.env_on = false;
    }

    pub(crate) const fn read_nr21(&self) -> u8 {
        0x3F | (self.duty << 6)
    }

    pub(crate) fn read_nr22(&self) -> u8 {
        self.env_base_vol << 4 | u8::from(self.env_inc) | self.env_period & 7
    }

    pub(crate) fn read_nr24(&self) -> u8 {
        0xBF | (u8::from(self.use_len) << 6)
    }

    pub(crate) fn write_nr21(&mut self, val: u8) {
        self.duty = (val >> 6) & 3;
        self.snd_len = SQ_MAX_LEN - (u16::from(val) & (SQ_MAX_LEN - 1));
    }

    pub(crate) fn write_nr22(&mut self, val: u8) {
        if val & 0xF8 == 0 {
            self.on = false;
            self.dac_on = false;
        } else {
            self.dac_on = true;
        }

        self.env_period = val & 7;
        self.env_on = self.env_period != 0;
        if self.env_period == 0 {
            self.env_period = 8;
        }
        self.env_timer = self.env_period;
        self.env_inc = val & 8 != 0;
        self.env_base_vol = val >> 4;
        self.env_vol = self.env_base_vol;
    }

    pub(crate) fn write_nr23(&mut self, val: u8) {
        self.freq = (self.freq & 0x700) | u16::from(val);
    }

    pub(crate) fn write_nr24(&mut self, val: u8) {
        self.freq = (u16::from(val) & 7) << 8 | (self.freq & 0xFF);

        let old_len = self.use_len;
        self.use_len = val & 0x40 != 0;

        if self.use_len && !old_len && self.p_half == 0 {
            self.step_len();
        }

        // trigger
        if val & 0x80 != 0 {
            if self.dac_on {
                self.on = true;
            }

            if self.snd_len == 0 {
                self.snd_len = SQ_MAX_LEN;
                if self.use_len && self.p_half == 0 {
                    self.step_len();
                }
            }

            self.period = i32::from(SQ_FREQ_P_MUL * (2048 - self.freq));
            self.out = 0;
            // self.duty_bit = 0;
            self.env_timer = self.env_period;
            self.env_vol = self.env_base_vol;
        }
    }

    fn step_sample(&mut self, cycles: i32) {
        if !(self.on && self.dac_on) {
            return;
        }

        self.period -= cycles;

        if self.period < 0 {
            self.period += i32::from(SQ_FREQ_P_MUL * (2048 - self.freq));
            self.out = (SQ_DUTY_TABLE[self.duty as usize] & (1 << self.duty_bit)) >> self.duty_bit;
            self.duty_bit = (self.duty_bit + 1) & 7;
        }
    }

    fn step_envelope(&mut self) {
        if !(self.env_on && self.on && self.dac_on) {
            return;
        }

        if self.env_inc && self.env_vol == 15 || !self.env_inc && self.env_vol == 0 {
            self.env_on = false;
            return;
        }

        self.env_timer -= 1;
        if self.env_timer == 0 {
            self.env_timer = self.env_period;

            if self.env_inc {
                self.env_vol += 1;
            } else {
                self.env_vol -= 1;
            }
        }
    }

    const fn out(&self) -> u8 {
        self.out * self.env_vol
    }

    const fn on(&self) -> bool {
        self.on && self.dac_on
    }

    fn step_len(&mut self) {
        if self.use_len && self.snd_len > 0 {
            self.snd_len -= 1;

            if self.snd_len == 0 {
                self.on = false;
            }
        }
    }

    fn set_period_half(&mut self, p_half: u8) {
        debug_assert!(p_half == 0 || p_half == 1);
        self.p_half = p_half;
    }
}

pub struct Wave {
    on:            bool,
    dac_on:        bool,
    use_len:       bool,
    snd_len:       u16,
    p_half:        u8,  // period half: 0 or 1
    freq:          u16, // 11 bit frequency data
    period:        i32,
    sample_buffer: u8,
    ram:           [u8; WAV_RAM_SIZE as usize],
    samples:       [u8; WAV_SAMPLE_SIZE as usize],
    sample_idx:    u8,
    vol:           u8,
    nr30:          u8,
}

impl Default for Wave {
    fn default() -> Self {
        Self {
            ram:           [0; WAV_RAM_SIZE as usize],
            vol:           0,
            on:            false,
            dac_on:        true,
            snd_len:       0,
            use_len:       false,
            p_half:        0, // doesn't matter
            freq:          0,
            period:        i32::from(WAV_PERIOD_MUL) * 2048,
            sample_buffer: 0,
            samples:       [0; WAV_SAMPLE_SIZE as usize],
            sample_idx:    0,
            nr30:          0,
        }
    }
}

impl Wave {
    fn reset(&mut self) {
        self.on = false;
        self.dac_on = true;
        self.snd_len = 0;
        self.use_len = false;
        self.freq = 0;
        self.vol = 0;
        self.sample_idx = 0;
        self.period = 0;
        self.sample_buffer = 0;
        self.nr30 = 0;
    }

    pub(crate) const fn read_wave_ram(&self, addr: u8) -> u8 {
        let index = (addr - 0x30) as usize;
        self.ram[index]
    }

    pub(crate) fn write_wave_ram(&mut self, addr: u8, val: u8) {
        let index = (addr - 0x30) as usize;
        self.ram[index] = val;
        // upper 4 bits first
        self.samples[index * 2] = val >> 4;
        self.samples[index * 2 + 1] = val & 0xF;
    }

    pub(crate) const fn read_nr30(&self) -> u8 {
        self.nr30 | 0x7F
    }

    pub(crate) const fn read_nr32(&self) -> u8 {
        0x9F | (self.vol << 5)
    }

    pub(crate) fn read_nr34(&self) -> u8 {
        0xBF | (u8::from(self.use_len) << 6)
    }

    pub(crate) fn write_nr30(&mut self, val: u8) {
        self.nr30 = val;
        if val & 0x80 == 0 {
            self.on = false;
            self.dac_on = false;
        } else {
            self.dac_on = true;
        }
    }

    pub(crate) fn write_nr31(&mut self, val: u8) {
        self.snd_len = WAV_MAX_LENGTH - (u16::from(val) & (WAV_MAX_LENGTH - 1));
    }

    pub(crate) fn write_nr32(&mut self, val: u8) {
        self.vol = (val >> 5) & 3;
    }

    pub(crate) fn write_nr33(&mut self, val: u8) {
        self.freq = (self.freq & 0x700) | u16::from(val);
    }

    pub(crate) fn write_nr34(&mut self, val: u8) {
        self.freq = (u16::from(val) & 7) << 8 | (self.freq & 0xFF);

        let old_len = self.use_len;
        self.use_len = val & 0x40 != 0;

        if self.use_len && !old_len && self.p_half == 0 {
            self.step_len();
        }

        // trigger
        if val & 0x80 != 0 {
            if self.dac_on {
                self.on = true;
            }

            if self.snd_len == 0 {
                self.snd_len = WAV_MAX_LENGTH;
                if self.use_len && self.p_half == 0 {
                    self.step_len();
                }
            }

            self.period = i32::from(WAV_PERIOD_MUL * (2048 - self.freq));
            self.sample_idx = 0;
        }
    }

    fn step_sample(&mut self, cycles: i32) {
        if !self.on() {
            return;
        }

        self.period -= cycles;

        if self.period < 0 {
            self.period += i32::from(WAV_PERIOD_MUL * (2048 - self.freq));
            self.sample_idx = (self.sample_idx + 1) % WAV_SAMPLE_SIZE;
            self.sample_buffer = self.samples[self.sample_idx as usize];
        }
    }

    fn out(&self) -> u8 {
        // wrapping_shr is necessary because (vol - 1) can be -1
        self.sample_buffer
            .wrapping_shr(u32::from(self.vol.wrapping_sub(1)))
    }

    const fn on(&self) -> bool {
        self.on && self.dac_on
    }

    fn step_len(&mut self) {
        if self.use_len && self.snd_len > 0 {
            self.snd_len -= 1;

            if self.snd_len == 0 {
                self.on = false;
            }
        }
    }

    fn set_period_half(&mut self, p_half: u8) {
        debug_assert!(p_half == 0 || p_half == 1);
        self.p_half = p_half;
    }
}

pub struct Noise {
    on:           bool,
    dac_on:       bool,
    use_len:      bool,
    snd_len:      u16,
    p_half:       u8, // period half: 0 or 1
    timer:        i32,
    timer_period: u16,
    lfsr:         u16,
    wide_step:    bool,
    out:          u8,
    nr43:         u8,

    // envelope
    env_on:       bool,
    env_vol:      u8,
    env_inc:      bool,
    env_base_vol: u8,
    env_period:   u8,
    env_timer:    u8,
}

impl Default for Noise {
    fn default() -> Self {
        Self {
            on:           false,
            dac_on:       true,
            snd_len:      0,
            use_len:      false,
            p_half:       0, // doesn't matter
            env_period:   8,
            env_timer:    8,
            env_on:       false,
            env_base_vol: 0,
            env_vol:      0,
            env_inc:      false,
            timer:        1,
            timer_period: 0,
            lfsr:         0x7FFF,
            wide_step:    false,
            out:          0,
            nr43:         0,
        }
    }
}

impl Noise {
    fn reset(&mut self) {
        self.on = false;
        self.dac_on = true;
        self.snd_len = 0;
        self.use_len = false;

        self.env_period = 8;
        self.env_timer = 8;
        self.env_inc = false;
        self.env_base_vol = 0;
        self.env_vol = 0;
        self.env_on = false;

        self.timer_period = 0;
        self.timer = 1;
        self.lfsr = 0x7FFF;
        self.wide_step = false;
        self.out = 0;
        self.nr43 = 0;
    }

    pub(crate) fn read_nr42(&self) -> u8 {
        self.env_base_vol << 4 | u8::from(self.env_inc) | self.env_period & 7
    }

    pub(crate) const fn read_nr43(&self) -> u8 {
        self.nr43
    }

    pub(crate) fn read_nr44(&self) -> u8 {
        0xBF | (u8::from(self.use_len) << 6)
    }

    pub(crate) fn write_nr41(&mut self, val: u8) {
        self.snd_len = NOISE_MAX_LEN - (u16::from(val) & (NOISE_MAX_LEN - 1));
    }

    pub(crate) fn write_nr42(&mut self, val: u8) {
        if val & 0xF8 == 0 {
            self.on = false;
            self.dac_on = false;
        } else {
            self.dac_on = true;
        }

        self.env_period = val & 7;
        self.env_on = self.env_period != 0;
        if self.env_period == 0 {
            self.env_period = 8;
        }
        self.env_timer = self.env_period;
        self.env_inc = val & 8 != 0;
        self.env_base_vol = val >> 4;
        self.env_vol = self.env_base_vol;
    }

    pub(crate) fn write_nr43(&mut self, val: u8) {
        self.nr43 = val;
        self.wide_step = val & 8 != 0;

        let clock_shift = val >> 4;
        let divisor: u16 = match val & 7 {
            0 => 8,
            1 => 16,
            2 => 32,
            3 => 48,
            4 => 64,
            5 => 80,
            6 => 96,
            _ => 112,
        };

        self.timer_period = divisor << clock_shift;
        self.timer = 1;
    }

    pub(crate) fn write_nr44(&mut self, val: u8) {
        let old_len = self.use_len;
        self.use_len = val & 0x40 != 0;

        if self.use_len && !old_len && self.p_half == 0 {
            self.step_len();
        }

        // trigger
        if val & 0x80 != 0 {
            if self.dac_on {
                self.on = true;
            }

            if self.snd_len == 0 {
                self.snd_len = NOISE_MAX_LEN;
                if self.use_len && self.p_half == 0 {
                    self.step_len();
                }
            }

            self.timer = i32::from(self.timer_period);
            self.lfsr = 0x7FFF;
            self.env_timer = self.env_period;
            self.env_vol = self.env_base_vol;
        }
    }

    fn step_envelope(&mut self) {
        if !(self.env_on && self.on && self.dac_on) {
            return;
        }

        if self.env_inc && self.env_vol == 15 || !self.env_inc && self.env_vol == 0 {
            self.env_on = false;
            return;
        }

        self.env_timer -= 1;
        if self.env_timer == 0 {
            self.env_timer = self.env_period;

            if self.env_inc {
                self.env_vol += 1;
            } else {
                self.env_vol -= 1;
            }
        }
    }

    fn step_sample(&mut self, cycles: i32) {
        if !self.on() {
            return;
        }

        self.timer -= cycles;

        if self.timer < 0 {
            self.timer += i32::from(self.timer_period);

            let xor_bit = (self.lfsr & 1) ^ ((self.lfsr & 2) >> 1);
            self.lfsr >>= 1;
            self.lfsr |= xor_bit << 14;
            if self.wide_step {
                self.lfsr |= xor_bit << 6;
            }

            self.out = u8::from(self.lfsr & 1 == 0);
        }
    }

    const fn out(&self) -> u8 {
        self.out * self.env_vol
    }

    const fn on(&self) -> bool {
        self.on && self.dac_on
    }

    fn step_len(&mut self) {
        if self.use_len && self.snd_len > 0 {
            self.snd_len -= 1;

            if self.snd_len == 0 {
                self.on = false;
            }
        }
    }

    fn set_period_half(&mut self, p_half: u8) {
        debug_assert!(p_half == 0 || p_half == 1);
        self.p_half = p_half;
    }
}
