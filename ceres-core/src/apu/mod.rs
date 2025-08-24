mod envelope;
mod high_pass_filter;
mod length_timer;
mod master_volume;
mod noise;
mod period_counter;
mod square;
mod sweep;
mod wave;

use {
    crate::{
        apu::{high_pass_filter::HighPassFilter, master_volume::MasterVolume},
        timing::DOTS_PER_SEC,
    },
    length_timer::LengthTimer,
    noise::Noise,
    period_counter::PeriodCounter,
    square::Square,
    sweep::{Sweep, SweepTrait},
    wave::Wave,
};

pub type Sample = i16;

pub trait AudioCallback {
    fn audio_sample(&self, l: Sample, r: Sample);
}

#[derive(Clone, Copy, Default)]
enum PeriodHalf {
    #[default]
    First,
    Second,
}

pub struct Apu<A: AudioCallback> {
    audio_callback: A,
    ch1: Square<Sweep>,
    ch2: Square<()>,
    ch3: Wave,
    ch4: Noise,
    div_divider: u8,
    enabled: bool,
    ext_sample_period: i32,
    hpf: HighPassFilter,
    master_volume: MasterVolume,
    nr51: u8,
    render_timer: i32,
}

impl<A: AudioCallback> Apu<A> {
    pub fn new(sample_rate: i32, audio_callback: A) -> Self {
        Self {
            ext_sample_period: Self::sample_period_from_rate(sample_rate),
            audio_callback,
            nr51: 0,
            enabled: false,
            master_volume: MasterVolume::default(),
            ch1: Square::default(),
            ch2: Square::default(),
            ch3: Wave::default(),
            ch4: Noise::default(),
            div_divider: 0,
            render_timer: 0,
            hpf: HighPassFilter::new(),
        }
    }

    pub fn reset(&mut self) {
        self.enabled = false;
        self.master_volume = MasterVolume::default();

        // reset registers
        self.ch1 = Square::default();
        self.ch2 = Square::default();
        self.ch3.reset();
        self.ch4 = Noise::default();
        self.nr51 = 0;

        // reset div divider
        self.div_divider = 0;

        // reset render timer
        self.render_timer = 0;
    }

    pub fn run(&mut self, dots: i32) {
        fn mix_and_render<C1: AudioCallback>(apu: &Apu<C1>) -> (Sample, Sample) {
            let mut l = 0;
            let mut r = 0;

            for i in 0..4 {
                let out = match i {
                    0 => apu.ch1.output() * u8::from(apu.ch1.is_truly_enabled()),
                    1 => apu.ch2.output() * u8::from(apu.ch2.is_truly_enabled()),
                    2 => apu.ch3.output() * u8::from(apu.ch3.is_truly_enabled()),
                    3 => apu.ch4.output() * u8::from(apu.ch4.is_truly_enabled()),
                    _ => break,
                };

                let right_on = u8::from(apu.nr51 & (1 << i) != 0);
                let left_on = u8::from(apu.nr51 & (0x10 << i) != 0);

                l += left_on * out;
                r += right_on * out;
            }

            // transform to i16 sample
            let l = (0xF - i16::from(l) * 2) * i16::from(apu.master_volume.left_volume() + 1);
            let r = (0xF - i16::from(r) * 2) * i16::from(apu.master_volume.right_volume() + 1);

            // amplify
            let l = l * 32;
            let r = r * 32;

            // transform to f32 sample
            // let l = l as f32 / i16::MAX as f32;
            // let r = r as f32 / i16::MAX as f32;

            (l, r)
        }

        if self.enabled {
            self.ch1.step_sample(dots);
            self.ch2.step_sample(dots);
            self.ch3.step_sample(dots);
            self.ch4.step_sample(dots);
        }

        self.render_timer += dots;
        while self.render_timer >= self.ext_sample_period {
            self.render_timer -= self.ext_sample_period;

            let (l, r) = mix_and_render(self);

            let (l, r) = if self.ch1.is_enabled()
                || self.ch2.is_enabled()
                || self.ch3.is_enabled()
                || self.ch4.is_enabled()
            {
                self.hpf.high_pass(l, r)
            } else {
                (l, r)
            };

            self.audio_callback.audio_sample(l, r);
        }
    }

    const fn sample_period_from_rate(sample_rate: i32) -> i32 {
        DOTS_PER_SEC / sample_rate
    }

    pub const fn set_sample_rate(&mut self, sample_rate: i32) {
        self.ext_sample_period = Self::sample_period_from_rate(sample_rate);
    }

    pub fn step_div_apu(&mut self) {
        const fn set_period_half<C1: AudioCallback>(apu: &mut Apu<C1>, p_half: PeriodHalf) {
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
            self.ch1.step_envelope();
            self.ch2.step_envelope();
            self.ch4.step_envelope();
        }

        if self.div_divider & 1 == 1 {
            self.ch1.step_length_timer();
            self.ch2.step_length_timer();
            self.ch3.step_length_timer();
            self.ch4.step_length_timer();

            set_period_half(self, PeriodHalf::First);
        } else {
            set_period_half(self, PeriodHalf::Second);
        }

        if self.div_divider & 3 == 3 {
            self.ch1.step_sweep();
        }
    }
}

// IO
impl<A: AudioCallback> Apu<A> {
    pub const fn enabled(&self) -> bool {
        self.enabled
    }

    pub const fn pcm12(&self) -> u8 {
        self.ch1.output() | (self.ch2.output() << 4)
    }

    pub const fn pcm34(&self) -> u8 {
        self.ch3.output() | (self.ch4.output() << 4)
    }

    pub fn read_nr10(&self) -> u8 {
        self.ch1.read_nrx0()
    }

    pub const fn read_nr11(&self) -> u8 {
        self.ch1.read_nrx1()
    }

    pub fn read_nr12(&self) -> u8 {
        self.ch1.read_nrx2()
    }

    pub const fn read_nr14(&self) -> u8 {
        self.ch1.read_nrx4()
    }

    pub const fn read_nr21(&self) -> u8 {
        self.ch2.read_nrx1()
    }

    pub fn read_nr22(&self) -> u8 {
        self.ch2.read_nrx2()
    }

    pub const fn read_nr24(&self) -> u8 {
        self.ch2.read_nrx4()
    }

    pub const fn read_nr30(&self) -> u8 {
        self.ch3.read_nr30()
    }

    pub const fn read_nr32(&self) -> u8 {
        self.ch3.read_nr32()
    }

    pub const fn read_nr34(&self) -> u8 {
        self.ch3.read_nr34()
    }

    pub fn read_nr42(&self) -> u8 {
        self.ch4.read_nr42()
    }

    pub const fn read_nr43(&self) -> u8 {
        self.ch4.read_nr43()
    }

    pub const fn read_nr44(&self) -> u8 {
        self.ch4.read_nr44()
    }

    #[must_use]
    pub fn read_nr50(&self) -> u8 {
        self.master_volume.read_nr50()
    }

    #[must_use]
    pub const fn read_nr51(&self) -> u8 {
        self.nr51
    }

    #[must_use]
    pub const fn read_nr52(&self) -> u8 {
        // println!("read nr52, ch2: {}", self.ch1.on());
        // println!(
        //     "Ch1 length timer: {}, Max: {}",
        //     self.ch1.length_timer.length, 0x3f
        // );
        // println!(
        //     "Ch1 sweep timer: {}, shadow pace: {}",
        //     self.ch1.period_counter.sweep.timer, self.ch1.period_counter.sweep.shadow_pace
        // );

        ((self.enabled as u8) << 7)
            | 0x70
            | ((self.ch4.is_enabled() as u8) << 3)
            | ((self.ch3.is_enabled() as u8) << 2)
            | ((self.ch2.is_enabled() as u8) << 1)
            | (self.ch1.is_enabled() as u8)
    }

    pub const fn read_wave_ram(&self, addr: u8) -> u8 {
        self.ch3.read_wave_ram(addr)
    }

    pub fn write_nr10(&mut self, val: u8) {
        self.ch1.write_nrx0(val);
    }

    pub const fn write_nr11(&mut self, val: u8) {
        self.ch1.write_nrx1(val);
    }

    pub fn write_nr12(&mut self, val: u8) {
        self.ch1.write_nrx2(val);
    }

    pub fn write_nr13(&mut self, val: u8) {
        self.ch1.write_nrx3(val);
    }

    pub fn write_nr14(&mut self, val: u8) {
        self.ch1.write_nrx4(val);
    }

    pub const fn write_nr21(&mut self, val: u8) {
        self.ch2.write_nrx1(val);
    }

    pub fn write_nr22(&mut self, val: u8) {
        self.ch2.write_nrx2(val);
    }

    pub fn write_nr23(&mut self, val: u8) {
        self.ch2.write_nrx3(val);
    }

    pub fn write_nr24(&mut self, val: u8) {
        self.ch2.write_nrx4(val);
    }

    pub const fn write_nr30(&mut self, val: u8) {
        self.ch3.write_nr30(val);
    }

    pub const fn write_nr31(&mut self, val: u8) {
        self.ch3.write_nr31(val);
    }

    pub const fn write_nr32(&mut self, val: u8) {
        self.ch3.write_nr32(val);
    }

    pub fn write_nr33(&mut self, val: u8) {
        self.ch3.write_nr33(val);
    }

    pub fn write_nr34(&mut self, val: u8) {
        self.ch3.write_nr34(val);
    }

    pub const fn write_nr41(&mut self, val: u8) {
        self.ch4.write_nr41(val);
    }

    pub fn write_nr42(&mut self, val: u8) {
        self.ch4.write_nr42(val);
    }

    pub const fn write_nr43(&mut self, val: u8) {
        self.ch4.write_nr43(val);
    }

    pub const fn write_nr44(&mut self, val: u8) {
        self.ch4.write_nr44(val);
    }

    pub const fn write_nr50(&mut self, val: u8) {
        if self.enabled {
            self.master_volume.write_nr50(val);
        }
    }

    pub const fn write_nr51(&mut self, val: u8) {
        if self.enabled {
            self.nr51 = val;
        }
    }

    pub fn write_nr52(&mut self, val: u8) {
        self.enabled = val & 0x80 != 0;

        if !self.enabled {
            // reset
            // self.render_timer = 0;
            self.div_divider = 0;
            self.master_volume = MasterVolume::default();

            // reset registers
            self.ch1 = Square::default();
            self.ch2 = Square::default();
            self.ch3.reset();
            self.ch4 = Noise::default();
            self.nr51 = 0;
        }
    }

    pub const fn write_wave_ram(&mut self, addr: u8, val: u8) {
        self.ch3.write_wave_ram(addr, val);
    }
}
