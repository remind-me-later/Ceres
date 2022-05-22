pub use self::{
    noise::Noise,
    square::{Ch1, Ch2},
    wave::Wave,
};
use crate::{Gb, TC_SEC};

mod common;
mod envelope;
mod noise;
mod square;
mod wave;

const APU_TIMER_RES: u16 = (TC_SEC / 512) as u16;

impl Gb {
    fn period(&self) -> u32 {
        unsafe { TC_SEC / (*self.apu_callbacks).sample_rate() }
    }

    pub fn tick_apu(&mut self) {
        let mut t_elapsed = self.t_elapsed();

        if !self.apu_on {
            return;
        }

        while t_elapsed > 0 {
            t_elapsed -= 1;

            self.apu_timer += 1;
            if self.apu_timer == APU_TIMER_RES + 1 {
                self.apu_timer = 0;
                self.step();
            }

            self.apu_ch1.step_sample();
            self.apu_ch2.step_sample();
            self.apu_ch3.step_sample();
            self.apu_ch4.step_sample();

            self.apu_render_timer += 1;
            if self.apu_render_timer == self.period() + 1 {
                self.apu_render_timer = 0;
                self.mix_and_render();
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

    fn set_period_half(&mut self, period_half: u8) {
        self.apu_ch1.mut_com().set_period_half(period_half);
        self.apu_ch2.mut_com().set_period_half(period_half);
        self.apu_ch3.mut_com().set_period_half(period_half);
        self.apu_ch4.mut_com().set_period_half(period_half);
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

        unsafe { (*self.apu_callbacks).push_frame(l, r) };
    }

    fn high_pass_filter(&mut self, sample: i16) -> f32 {
        let charge_factor = 0.998_943;
        // TODO: amplification
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
