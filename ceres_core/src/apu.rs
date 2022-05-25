use crate::{Gb, TC_SEC};

const APU_TIMER_RES: u16 = (TC_SEC / 512) as u16;

const SQ_MAX_LEN: u16 = 64;
const SQ_FREQ_P_MUL: u16 = 4;
const SQ_DUTY_TABLE: [u8; 4] = [0b0000_0001, 0b1000_0001, 0b1000_0111, 0b0111_1110];

const WAV_MAX_LENGTH: u16 = 256;
const WAV_RAM_SIZE: u8 = 0x10;
const WAV_SAMPLE_SIZE: u8 = WAV_RAM_SIZE * 2;
const WAV_PERIOD_MUL: u16 = 2;

const NOISE_MAX_LEN: u16 = 64;

impl Gb {
    fn period(&self) -> u32 {
        TC_SEC / (*self.apu_callbacks).sample_rate()
    }

    pub fn tick_apu(&mut self, mut cycles: i16) {
        if !self.apu_on {
            return;
        }

        while cycles > 0 {
            cycles -= 1;

            if self.apu_timer == APU_TIMER_RES {
                self.apu_timer = 0;
                self.step();
            } else {
                self.apu_timer += 1;
            }

            self.apu_ch1.step_sample();
            self.apu_ch2.step_sample();
            self.apu_ch3.step_sample();
            self.apu_ch4.step_sample();

            if self.apu_render_timer == self.period() {
                self.apu_render_timer = 0;
                self.mix_and_render();
            } else {
                self.apu_render_timer += 1;
            }
        }
    }

    fn step(&mut self) {
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

    fn mix_and_render(&mut self) {
        let (l, r) = self
            .ch_out_iter()
            .fold((0, 0), |(l, r), (lp, rp)| (l + lp, r + rp));

        // transform to i16 sample
        let l = (0xf - l as i16 * 2) * self.apu_l_vol as i16;
        let r = (0xf - r as i16 * 2) * self.apu_r_vol as i16;

        // filter and transform to f32
        let l = self.high_pass_filter(l);
        let r = self.high_pass_filter(r);

        self.apu_callbacks.push_frame(l, r);
    }

    fn high_pass_filter(&mut self, sample: i16) -> f32 {
        let charge_factor = 0.998_943;
        if self.apu_on {
            let input = (sample * 32) as f32 / 32768.0;
            let output = input - self.apu_cap;
            self.apu_cap = input - output * charge_factor;
            output
        } else {
            0.0
        }
    }

    fn reset(&mut self) {
        self.apu_render_timer = self.period();
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
    pub fn read_nr50(&self) -> u8 {
        self.apu_r_vol
            | (self.apu_r_vin as u8) << 3
            | self.apu_l_vol << 4
            | (self.apu_l_vin as u8) << 7
    }

    #[must_use]
    pub fn read_nr52(&self) -> u8 {
        (self.apu_on as u8) << 7
            | 0x70
            | (self.apu_ch4.on() as u8) << 3
            | (self.apu_ch3.on() as u8) << 2
            | (self.apu_ch2.on() as u8) << 1
            | self.apu_ch1.on() as u8
    }

    pub fn write_wave(&mut self, addr: u8, val: u8) {
        self.apu_ch3.write_wave_ram(addr, val);
    }

    pub fn write_nr50(&mut self, val: u8) {
        if self.apu_on {
            self.apu_r_vol = val & 7;
            self.apu_r_vin = val & 8 != 0;
            self.apu_l_vol = (val >> 4) & 7;
            self.apu_l_vin = val & 0x80 != 0;
        }
    }

    pub fn write_nr51(&mut self, val: u8) {
        if self.apu_on {
            self.nr51 = val;
        }
    }

    pub fn write_nr52(&mut self, val: u8) {
        self.apu_on = val & 0x80 != 0;
        if !self.apu_on {
            self.reset();
        }
    }

    fn ch_out_iter(&self) -> ChOutIter {
        ChOutIter { i: 0, gb: self }
    }
}

struct ChOutIter<'a> {
    i: u8,
    gb: &'a Gb,
}

impl<'a> Iterator for ChOutIter<'a> {
    type Item = (u8, u8);

    fn next(&mut self) -> Option<Self::Item> {
        let out = match self.i {
            0 => self.gb.apu_ch1.out() * self.gb.apu_ch1.on() as u8,
            1 => self.gb.apu_ch2.out() * self.gb.apu_ch2.on() as u8,
            2 => self.gb.apu_ch3.out() * self.gb.apu_ch3.on() as u8,
            3 => self.gb.apu_ch4.out() * self.gb.apu_ch4.on() as u8,
            _ => return None,
        };

        let ch_r_on = (self.gb.nr51 & (1 << self.i) != 0) as u8;
        let ch_l_on = (self.gb.nr51 & (1 << (self.i + 4)) != 0) as u8;

        self.i += 1;

        Some((ch_l_on * out, ch_r_on * out))
    }
}

pub struct Square1 {
    on: bool,
    dac_on: bool,
    snd_counter: bool,
    snd_len: u16,
    p_half: u8, // period half: 0 or 1

    freq: u16, // 11 bit frequency data
    duty: u8,
    duty_bit: u8,
    period: u16,
    out: u8,

    // envelope
    env_vol: u8,
    env_on: bool,
    env_inc: bool,
    env_base_vol: u8,
    env_period: u8,
    env_timer: u8,

    // sweep
    sw_on: bool,
    sw_dec: bool,
    sw_period: u8,
    sw_shift: u8,
    sw_timer: u8,
    sw_shadow_freq: u16,
}

impl Square1 {
    pub fn new() -> Self {
        Self {
            sw_period: 8,
            sw_dec: false,
            sw_shift: 0,
            sw_timer: 8,
            sw_shadow_freq: 0,
            sw_on: false,
            freq: 0,
            period: SQ_FREQ_P_MUL * 2048,
            duty: SQ_DUTY_TABLE[0],
            duty_bit: 0,
            on: false,
            dac_on: true,
            snd_len: 0,
            snd_counter: false,
            p_half: 0, // doesn't matter
            out: 0,
            env_on: false,
            env_base_vol: 0,
            env_period: 8,
            env_vol: 0,
            env_timer: 8,
            env_inc: false,
        }
    }

    pub fn reset(&mut self) {
        self.on = false;
        self.dac_on = true;
        self.snd_len = 0;
        self.snd_counter = false;
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

        if self.sw_shadow_freq > 0x7ff {
            self.on = false;
        } else if self.sw_shift != 0 {
            self.freq = self.sw_shadow_freq & 0x7ff;
        }
    }

    pub fn read_nr10(&self) -> u8 {
        0x80 | ((self.sw_period as u8 & 7) << 4) | ((!self.sw_dec as u8) << 3) | self.sw_shift
    }

    pub fn read_nr11(&self) -> u8 {
        0x3f | ((self.duty as u8) << 6)
    }

    pub fn read_nr12(&self) -> u8 {
        self.env_base_vol << 4 | self.env_inc as u8 | self.env_period & 7
    }

    pub fn read_nr14(&self) -> u8 {
        0xbf | ((self.snd_counter as u8) << 6)
    }

    pub fn write_nr10(&mut self, val: u8) {
        self.sw_period = (val >> 4) & 7;
        if self.sw_period == 0 {
            self.sw_period = 8;
        }
        self.sw_dec = (val & 8) != 0;
        self.sw_shift = val & 7;
    }

    pub fn write_nr11(&mut self, val: u8) {
        self.duty = (val >> 6) & 3;
        self.snd_len = SQ_MAX_LEN - ((val as u16) & (SQ_MAX_LEN - 1));
    }

    pub fn write_nr12(&mut self, val: u8) {
        // TODO: likely wrong, value == 0x7 || value == 0x0 to pass Blargg 2 test
        if val == 7 {
            self.on = false;
            self.dac_on = false;
        }

        if val == 0x10 {
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

    pub fn write_nr13(&mut self, val: u8) {
        self.freq = (self.freq & 0x700) | (val as u16);
    }

    pub fn write_nr14(&mut self, val: u8) {
        self.freq = (val as u16 & 7) << 8 | (self.freq & 0xff);
        self.snd_counter = val & 0x40 != 0;

        if self.snd_counter && self.p_half == 0 {
            self.step_len();
        }

        if val & 0x80 != 0 {
            if self.dac_on {
                self.on = true;
            }

            if self.snd_len == 0 {
                self.snd_len = SQ_MAX_LEN;
            }

            self.period = SQ_FREQ_P_MUL * (2048 - self.freq);
            self.out = 0;
            self.duty_bit = 0;
            self.env_timer = self.env_period;
            self.env_vol = self.env_base_vol;

            self.sw_shadow_freq = self.freq;
            self.sw_timer = self.sw_period;
            self.sw_on = self.sw_period != 8 && self.sw_shift != 0;

            if self.sw_shift != 0 {
                self.calculate_sweep();
            }
        }
    }

    pub fn step_sample(&mut self) {
        if !(self.on && self.dac_on) {
            return;
        }

        let (period, overflow) = self.period.overflowing_sub(1);

        if overflow {
            self.period = SQ_FREQ_P_MUL * (2048 - self.freq);

            if self.on && self.dac_on {
                self.out =
                    (SQ_DUTY_TABLE[self.duty as usize] & (1 << self.duty_bit)) >> self.duty_bit;
                self.duty_bit = (self.duty_bit + 1) & 7;
            }
        } else {
            self.period = period;
        }
    }

    pub fn step_sweep(&mut self) {
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

    pub fn step_envelope(&mut self) {
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

    pub fn out(&self) -> u8 {
        self.out * self.env_vol
    }

    pub fn on(&self) -> bool {
        self.on && self.dac_on
    }

    pub fn step_len(&mut self) {
        if self.snd_counter && self.snd_len > 0 {
            self.snd_len -= 1;

            if self.snd_len == 0 {
                self.on = false;
            }
        }
    }

    pub fn set_period_half(&mut self, p_half: u8) {
        debug_assert!(p_half == 0 || p_half == 1);
        self.p_half = p_half;
    }
}

pub struct Square2 {
    on: bool,
    dac_on: bool,
    snd_counter: bool,
    snd_len: u16,
    p_half: u8, // period half: 0 or 1

    freq: u16, // 11 bit frequency data
    duty: u8,
    duty_bit: u8,
    period: u16,
    out: u8,

    // envelope
    env_vol: u8,
    env_on: bool,
    env_inc: bool,
    env_base_vol: u8,
    env_period: u8,
    env_timer: u8,
}

impl Square2 {
    pub fn new() -> Self {
        Self {
            freq: 0,
            period: SQ_FREQ_P_MUL * 2048,
            duty: SQ_DUTY_TABLE[0],
            duty_bit: 0,
            on: false,
            dac_on: true,
            snd_len: 0,
            snd_counter: false,
            p_half: 0, // doesn't matter
            out: 0,
            env_on: false,
            env_base_vol: 0,
            env_period: 8,
            env_vol: 0,
            env_timer: 8,
            env_inc: false,
        }
    }

    pub fn reset(&mut self) {
        self.on = false;
        self.dac_on = true;
        self.snd_len = 0;
        self.snd_counter = false;
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

    pub fn read_nr21(&self) -> u8 {
        0x3f | ((self.duty as u8) << 6)
    }

    pub fn read_nr22(&self) -> u8 {
        self.env_base_vol << 4 | self.env_inc as u8 | self.env_period & 7
    }

    pub fn read_nr24(&self) -> u8 {
        0xbf | ((self.snd_counter as u8) << 6)
    }

    pub fn write_nr21(&mut self, val: u8) {
        self.duty = (val >> 6) & 3;
        self.snd_len = SQ_MAX_LEN - ((val as u16) & (SQ_MAX_LEN - 1));
    }

    pub fn write_nr22(&mut self, val: u8) {
        // TODO: likely wrong, value == 0x7 || value == 0x0 to pass Blargg 2 test
        if val == 7 {
            self.on = false;
            self.dac_on = false;
        }

        if val == 0x10 {
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

    pub fn write_nr23(&mut self, val: u8) {
        self.freq = (self.freq & 0x700) | (val as u16);
    }

    pub fn write_nr24(&mut self, val: u8) {
        self.freq = (val as u16 & 7) << 8 | (self.freq & 0xff);
        self.snd_counter = val & 0x40 != 0;

        if self.snd_counter && self.p_half == 0 {
            self.step_len();
        }

        if val & 0x80 != 0 {
            if self.dac_on {
                self.on = true;
            }

            if self.snd_len == 0 {
                self.snd_len = SQ_MAX_LEN;
            }

            self.period = SQ_FREQ_P_MUL * (2048 - self.freq);
            self.out = 0;
            self.duty_bit = 0;
            self.env_timer = self.env_period;
            self.env_vol = self.env_base_vol;
        }
    }

    pub fn step_sample(&mut self) {
        if !(self.on && self.dac_on) {
            return;
        }

        let (period, overflow) = self.period.overflowing_sub(1);

        if overflow {
            self.period = SQ_FREQ_P_MUL * (2048 - self.freq);

            if self.on && self.dac_on {
                self.out =
                    (SQ_DUTY_TABLE[self.duty as usize] & (1 << self.duty_bit)) >> self.duty_bit;
                self.duty_bit = (self.duty_bit + 1) & 7;
            }
        } else {
            self.period = period;
        }
    }

    pub fn step_envelope(&mut self) {
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

    pub fn out(&self) -> u8 {
        self.out * self.env_vol
    }

    pub fn on(&self) -> bool {
        self.on && self.dac_on
    }

    pub fn step_len(&mut self) {
        if self.snd_counter && self.snd_len > 0 {
            self.snd_len -= 1;

            if self.snd_len == 0 {
                self.on = false;
            }
        }
    }

    pub fn set_period_half(&mut self, p_half: u8) {
        debug_assert!(p_half == 0 || p_half == 1);
        self.p_half = p_half;
    }
}

pub struct Wave {
    on: bool,
    dac_on: bool,
    use_len: bool,
    snd_len: u16,
    p_half: u8, // period half: 0 or 1
    freq: u16,  // 11 bit frequency data
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
        Self {
            ram: [0; WAV_RAM_SIZE as usize],
            vol: 0,
            on: false,
            dac_on: true,
            snd_len: 0,
            use_len: false,
            p_half: 0, // doesn't matter
            freq: 0,
            period: WAV_PERIOD_MUL * 2048,
            sample_buffer: 0,
            samples: [0; WAV_SAMPLE_SIZE as usize],
            sample_idx: 0,
            nr30: 0,
        }
    }

    pub fn reset(&mut self) {
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
        0xbf | ((self.use_len as u8) << 6)
    }

    pub fn write_nr30(&mut self, val: u8) {
        self.nr30 = val;
        if val & 0x80 == 0 {
            self.on = false;
            self.dac_on = false;
        } else {
            self.dac_on = true;
        }
    }

    pub fn write_nr31(&mut self, val: u8) {
        self.snd_len = WAV_MAX_LENGTH - ((val as u16) & (WAV_MAX_LENGTH - 1));
    }

    pub fn write_nr32(&mut self, val: u8) {
        self.vol = (val >> 5) & 3;
    }

    pub fn write_nr33(&mut self, val: u8) {
        self.freq = (self.freq & 0x700) | (val as u16);
    }

    pub fn write_nr34(&mut self, val: u8) {
        self.freq = (val as u16 & 7) << 8 | (self.freq & 0xff);
        self.use_len = val & 0x40 != 0;

        if self.use_len && self.p_half == 0 {
            self.step_len();
        }

        // trigger channel reset?
        if val & 0x80 != 0 {
            if self.dac_on {
                self.on = true;
            }

            if self.snd_len == 0 {
                self.snd_len = WAV_MAX_LENGTH;
            }

            self.period = WAV_PERIOD_MUL * (2048 - self.freq);
            self.sample_idx = 0;
        }
    }

    pub fn step_sample(&mut self) {
        if !self.on() {
            return;
        }

        if self.period == 0 {
            self.period = WAV_PERIOD_MUL * (2048 - self.freq);
            self.sample_idx = (self.sample_idx + 1) % WAV_SAMPLE_SIZE;
            self.sample_buffer = self.samples[self.sample_idx as usize];
        }

        self.period -= 1;
    }

    pub fn out(&self) -> u8 {
        self.sample_buffer >> (self.vol.wrapping_sub(1) & 7)
    }

    pub fn on(&self) -> bool {
        self.on && self.dac_on
    }

    pub fn step_len(&mut self) {
        if self.use_len && self.snd_len > 0 {
            self.snd_len -= 1;

            if self.snd_len == 0 {
                self.on = false;
            }
        }
    }

    pub fn set_period_half(&mut self, p_half: u8) {
        debug_assert!(p_half == 0 || p_half == 1);
        self.p_half = p_half;
    }
}

pub struct Noise {
    on: bool,
    dac_on: bool,
    snd_counter: bool,
    snd_len: u16,
    p_half: u8, // period half: 0 or 1
    timer: u16,
    timer_period: u16,
    lfsr: u16,
    wide_step: bool,
    out: u8,
    nr43: u8,

    // envelope
    env_on: bool,
    env_vol: u8,
    env_inc: bool,
    env_base_vol: u8,
    env_period: u8,
    env_timer: u8,
}

impl Noise {
    pub fn new() -> Self {
        Self {
            on: false,
            dac_on: true,
            snd_len: 0,
            snd_counter: false,
            p_half: 0, // doesn't matter
            env_period: 8,
            env_timer: 8,
            env_on: false,
            env_base_vol: 0,
            env_vol: 0,
            env_inc: false,
            timer: 1,
            timer_period: 0,
            lfsr: 0x7fff,
            wide_step: false,
            out: 0,
            nr43: 0,
        }
    }

    pub fn reset(&mut self) {
        self.on = false;
        self.dac_on = true;
        self.snd_len = 0;
        self.snd_counter = false;

        self.env_period = 8;
        self.env_timer = 8;
        self.env_inc = false;
        self.env_base_vol = 0;
        self.env_vol = 0;
        self.env_on = false;

        self.timer_period = 0;
        self.timer = 1;
        self.lfsr = 0x7fff;
        self.wide_step = false;
        self.out = 0;
        self.nr43 = 0;
    }

    pub fn read_nr42(&self) -> u8 {
        self.env_base_vol << 4 | self.env_inc as u8 | self.env_period & 7
    }

    pub fn read_nr43(&self) -> u8 {
        self.nr43
    }

    pub fn read_nr44(&self) -> u8 {
        0xbf | ((self.snd_counter as u8) << 6)
    }

    pub fn write_nr41(&mut self, val: u8) {
        self.snd_len = NOISE_MAX_LEN - ((val as u16) & (NOISE_MAX_LEN - 1));
    }

    pub fn write_nr42(&mut self, val: u8) {
        // TODO: likely wrong, value == 0x7 || value == 0x0 to pass Blargg 2 test
        if val == 7 {
            self.on = false;
            self.dac_on = false;
        }

        if val == 0x10 {
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

    pub fn write_nr43(&mut self, val: u8) {
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

    pub fn write_nr44(&mut self, val: u8) {
        self.snd_counter = val & 0x40 != 0;

        if self.snd_counter && self.p_half == 0 {
            self.step_len();
        }

        // trigger channel reset?
        if val & 0x80 != 0 {
            if self.dac_on {
                self.on = true;
            }

            if self.snd_len == 0 {
                self.snd_len = NOISE_MAX_LEN;
            }

            self.timer = self.timer_period;
            self.lfsr = 0x7fff;
            self.env_timer = self.env_period;
            self.env_vol = self.env_base_vol;
        }
    }

    pub fn step_envelope(&mut self) {
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

    pub fn step_sample(&mut self) {
        if !self.on() {
            return;
        }

        let (new_timer_cycle, overflow) = self.timer.overflowing_sub(1);

        if overflow {
            self.timer = self.timer_period;

            let xor_bit = (self.lfsr & 1) ^ ((self.lfsr & 2) >> 1);
            self.lfsr >>= 1;
            self.lfsr |= xor_bit << 14;
            if self.wide_step {
                self.lfsr |= xor_bit << 6;
            }

            self.out = if self.lfsr & 1 == 0 { 1 } else { 0 };
        } else {
            self.timer = new_timer_cycle;
        }
    }

    pub fn out(&self) -> u8 {
        self.out * self.env_vol
    }

    pub fn on(&self) -> bool {
        self.on && self.dac_on
    }

    pub fn step_len(&mut self) {
        if self.snd_counter && self.snd_len > 0 {
            self.snd_len -= 1;

            if self.snd_len == 0 {
                self.on = false;
            }
        }
    }

    pub fn set_period_half(&mut self, p_half: u8) {
        debug_assert!(p_half == 0 || p_half == 1);
        self.p_half = p_half;
    }
}
