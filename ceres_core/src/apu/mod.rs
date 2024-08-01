use {
    crate::TC_SEC,
    length_timer::LengthTimer,
    noise::Noise,
    square::{Square1, Square2},
    sweep::{Sweep, SweepTrait},
    wave::Wave,
    wave_length::WaveLength,
};

mod envelope;
mod length_timer;
mod noise;
mod square;
mod sweep;
mod wave;
mod wave_length;

pub type Sample = f32;

pub trait AudioCallback {
    fn audio_sample(&self, l: Sample, r: Sample);
}

#[derive(Clone, Copy, Default)]
enum PHalf {
    #[default]
    First,
    Second,
}

// #[derive(Default)]
pub struct Apu<C: AudioCallback> {
    nr51: u8,

    on: bool,
    r_vol: u8,
    l_vol: u8,
    r_vin: bool,
    l_vin: bool,

    ch1: Square1,
    ch2: Square2,
    ch3: Wave,
    ch4: Noise,

    div_divider: u8,

    render_timer: i32,
    ext_sample_period: i32,

    audio_callback: C,

    capacitor_l: f32,
    capacitor_r: f32,
}

impl<C: AudioCallback> Apu<C> {
    pub fn new(sample_rate: i32, audio_callback: C) -> Self {
        Self {
            ext_sample_period: Self::sample_period_from_rate(sample_rate),
            audio_callback,
            nr51: 0,
            on: false,
            r_vol: 0,
            l_vol: 0,
            r_vin: false,
            l_vin: false,
            ch1: Square1::default(),
            ch2: Square2::default(),
            ch3: Wave::default(),
            ch4: Noise::default(),
            div_divider: 0,
            render_timer: 0,
            capacitor_l: 0.0,
            capacitor_r: 0.0,
        }
    }

    const fn sample_period_from_rate(sample_rate: i32) -> i32 {
        // TODO: maybe account for difference between 59.7 and target Hz?
        // FIXME: definitely wrong, but avoids underrun
        // check if frequency is too high
        ((TC_SEC / sample_rate) * 597) / 600 - 1
    }

    pub fn run(&mut self, cycles: i32) {
        fn mix_and_render<C1: AudioCallback>(apu: &Apu<C1>) -> (Sample, Sample) {
            let mut l = 0;
            let mut r = 0;

            for i in 0..4 {
                let out = match i {
                    0 => apu.ch1.out() * u8::from(apu.ch1.true_on()),
                    1 => apu.ch2.out() * u8::from(apu.ch2.true_on()),
                    2 => apu.ch3.out() * u8::from(apu.ch3.true_on()),
                    3 => apu.ch4.out() * u8::from(apu.ch4.true_on()),
                    _ => break,
                };

                let right_on = u8::from(apu.nr51 & (1 << i) != 0);
                let left_on = u8::from(apu.nr51 & (1 << (i + 4)) != 0);

                l += left_on * out;
                r += right_on * out;
            }

            // transform to i16 sample
            let l = (0xF - i16::from(l) * 2) * i16::from(apu.l_vol);
            let r = (0xF - i16::from(r) * 2) * i16::from(apu.r_vol);

            // amplify
            let l = l * 32;
            let r = r * 32;

            // transform to f32 sample
            let l = l as f32 / i16::MAX as f32;
            let r = r as f32 / i16::MAX as f32;

            (l, r)
        }

        if self.on {
            self.ch1.step_sample(cycles);
            self.ch2.step_sample(cycles);
            self.ch3.step_sample(cycles);
            self.ch4.step_sample(cycles);
        }

        self.render_timer += cycles;
        while self.render_timer > self.ext_sample_period {
            self.render_timer -= self.ext_sample_period;

            if self.on {
                let (l, r) = mix_and_render(self);
                let (l, r) = self.high_pass(l, r);

                self.audio_callback.audio_sample(l, r);
            } else {
                self.audio_callback.audio_sample(0.0, 0.0);
            }
        }
    }

    fn high_pass(&mut self, l: Sample, r: Sample) -> (Sample, Sample) {
        let mut outl = 0.0;
        let mut outr = 0.0;

        if self.ch1.on() || self.ch2.on() || self.ch3.on() || self.ch4.on() {
            outl = l - self.capacitor_l;
            outr = r - self.capacitor_r;

            self.capacitor_l = l - outl * 0.999958;
            self.capacitor_r = r - outr * 0.999958;
        }

        (outl, outr)
    }

    pub fn step_seq(&mut self) {
        fn set_period_half<C1: AudioCallback>(apu: &mut Apu<C1>, p_half: PHalf) {
            apu.ch1.set_period_half(p_half);
            apu.ch2.set_period_half(p_half);
            apu.ch3.set_period_half(p_half);
            apu.ch4.set_period_half(p_half);
        }

        // TODO: is this ok?
        // if !self.on() {
        //   return;
        // }

        self.div_divider = self.div_divider.wrapping_add(1);

        if self.div_divider & 7 == 7 {
            self.ch1.step_env();
            self.ch2.step_env();
            self.ch4.step_env();
        }

        if self.div_divider & 1 == 1 {
            self.ch1.step_len();
            self.ch2.step_len();
            self.ch3.step_len();
            self.ch4.step_len();

            set_period_half(self, PHalf::First);
        } else {
            set_period_half(self, PHalf::Second);
        }

        if self.div_divider & 3 == 3 {
            self.ch1.step_sweep();
        }
    }
}

// IO
impl<C: AudioCallback> Apu<C> {
    #[must_use]
    pub fn read_nr50(&self) -> u8 {
        self.r_vol | u8::from(self.r_vin) << 3 | self.l_vol << 4 | u8::from(self.l_vin) << 7
    }

    #[must_use]
    pub const fn read_nr51(&self) -> u8 {
        self.nr51
    }

    #[must_use]
    pub const fn read_nr52(&self) -> u8 {
        // println!("read nr52, ch2: {}", self.ch1.on());

        (self.on as u8) << 7
            | 0x70
            | (self.ch4.on() as u8) << 3
            | (self.ch3.on() as u8) << 2
            | (self.ch2.on() as u8) << 1
            | (self.ch1.on() as u8)
    }

    pub fn write_nr50(&mut self, val: u8) {
        if self.on {
            self.r_vol = val & 7;
            self.r_vin = val & 8 != 0;
            self.l_vol = (val >> 4) & 7;
            self.l_vin = val & 0x80 != 0;
        }
    }

    pub fn write_nr51(&mut self, val: u8) {
        if self.on {
            self.nr51 = val;
        }
    }

    pub fn write_nr52(&mut self, val: u8) {
        self.on = val & 0x80 != 0;

        if !self.on {
            // reset
            self.render_timer = 0;
            self.ch1 = Square1::default();
            self.ch2 = Square2::default();
            self.ch3.reset();
            self.ch4 = Noise::default();
            self.l_vol = 0;
            self.l_vin = false;
            self.r_vol = 0;
            self.r_vin = false;
            self.nr51 = 0;
            self.div_divider = 0;
        }
    }

    pub const fn on(&self) -> bool {
        self.on
    }

    pub const fn pcm12(&self) -> u8 {
        self.ch1.out() | (self.ch2.out() << 4)
    }

    pub const fn pcm34(&self) -> u8 {
        self.ch3.out() | (self.ch4.out() << 4)
    }

    // Channel 1 IO
    pub fn read_nr10(&self) -> u8 {
        self.ch1.read_nr10()
    }

    pub const fn read_nr11(&self) -> u8 {
        self.ch1.read_nr11()
    }

    pub const fn read_nr12(&self) -> u8 {
        self.ch1.read_nr12()
    }

    pub fn read_nr14(&self) -> u8 {
        self.ch1.read_nr14()
    }

    pub fn write_nr10(&mut self, val: u8) {
        self.ch1.write_nr10(val);
    }

    pub fn write_nr11(&mut self, val: u8) {
        self.ch1.write_nr11(val);
    }

    pub fn write_nr12(&mut self, val: u8) {
        self.ch1.write_nr12(val);
    }

    pub fn write_nr13(&mut self, val: u8) {
        self.ch1.write_nr13(val);
    }

    pub fn write_nr14(&mut self, val: u8) {
        self.ch1.write_nr14(val);
    }

    // Channel 2 IO
    pub const fn read_nr21(&self) -> u8 {
        self.ch2.read_nr21()
    }

    pub const fn read_nr22(&self) -> u8 {
        self.ch2.read_nr22()
    }

    pub fn read_nr24(&self) -> u8 {
        self.ch2.read_nr24()
    }

    pub fn write_nr21(&mut self, val: u8) {
        self.ch2.write_nr21(val);
    }

    pub fn write_nr22(&mut self, val: u8) {
        self.ch2.write_nr22(val);
    }

    pub fn write_nr23(&mut self, val: u8) {
        self.ch2.write_nr23(val);
    }

    pub fn write_nr24(&mut self, val: u8) {
        self.ch2.write_nr24(val);
    }

    // Channel 3 IO
    pub const fn read_nr30(&self) -> u8 {
        self.ch3.read_nr30()
    }

    pub const fn read_nr32(&self) -> u8 {
        self.ch3.read_nr32()
    }

    pub fn read_nr34(&self) -> u8 {
        self.ch3.read_nr34()
    }

    pub const fn read_wave_ram(&self, addr: u8) -> u8 {
        self.ch3.read_wave_ram(addr)
    }

    pub fn write_nr30(&mut self, val: u8) {
        self.ch3.write_nr30(val);
    }

    pub fn write_nr31(&mut self, val: u8) {
        self.ch3.write_nr31(val);
    }

    pub fn write_nr32(&mut self, val: u8) {
        self.ch3.write_nr32(val);
    }

    pub fn write_nr33(&mut self, val: u8) {
        self.ch3.write_nr33(val);
    }

    pub fn write_nr34(&mut self, val: u8) {
        self.ch3.write_nr34(val);
    }

    pub fn write_wave_ram(&mut self, addr: u8, val: u8) {
        self.ch3.write_wave_ram(addr, val);
    }

    // Channel 4 IO
    pub const fn read_nr42(&self) -> u8 {
        self.ch4.read_nr42()
    }

    pub const fn read_nr43(&self) -> u8 {
        self.ch4.read_nr43()
    }

    pub fn read_nr44(&self) -> u8 {
        self.ch4.read_nr44()
    }

    pub fn write_nr41(&mut self, val: u8) {
        self.ch4.write_nr41(val);
    }

    pub fn write_nr42(&mut self, val: u8) {
        self.ch4.write_nr42(val);
    }

    pub fn write_nr43(&mut self, val: u8) {
        self.ch4.write_nr43(val);
    }

    pub fn write_nr44(&mut self, val: u8) {
        self.ch4.write_nr44(val);
    }
}
