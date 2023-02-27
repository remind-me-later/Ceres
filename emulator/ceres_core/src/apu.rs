pub use channel::{
  noise::Noise,
  square::{Square1, Square2},
  wave::Wave,
};

use crate::TC_SEC;

pub type Sample = i16;

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum PHalf {
  #[default]
  First,
  Second,
}

#[derive(Default)]
pub struct Apu {
  nr51: u8,

  on:    bool,
  r_vol: u8,
  l_vol: u8,
  r_vin: bool,
  l_vin: bool,

  ch1: Square1,
  ch2: Square2,
  ch3: Wave,
  ch4: Noise,

  render_timer:      i32,
  ext_sample_period: i32,
  seq_step:          u8,

  l_out: Sample,
  r_out: Sample,

  samples_run: usize,
}

impl Apu {
  pub fn new(sample_rate: i32) -> Self {
    Self {
      ext_sample_period: Self::sample_period_from_rate(sample_rate),
      ..Default::default()
    }
  }

  pub const fn samples_run(&self) -> usize { self.samples_run }

  pub fn reset_samples_run(&mut self) { self.samples_run = 0; }

  const fn sample_period_from_rate(sample_rate: i32) -> i32 {
    // maybe account for difference between 59.7 and target Hz?
    TC_SEC / sample_rate + 1
  }

  pub const fn out(&self) -> (Sample, Sample) { (self.l_out, self.r_out) }

  pub fn run(&mut self, cycles: i32) {
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
        let (l, r) = self.mix_and_render();

        self.l_out = l;
        self.r_out = r;
      } else {
        self.l_out = 0;
        self.r_out = 0;
      }

      self.samples_run += 2;
    }
  }

  pub fn step_seq(&mut self) {
    fn set_period_half(apu: &mut Apu, p_half: PHalf) {
      apu.ch1.set_period_half(p_half);
      apu.ch2.set_period_half(p_half);
      apu.ch3.set_period_half(p_half);
      apu.ch4.set_period_half(p_half);
    }

    if self.seq_step & 1 == 0 {
      self.ch1.step_len();
      self.ch2.step_len();
      self.ch3.step_len();
      self.ch4.step_len();

      set_period_half(self, PHalf::First);
    } else {
      set_period_half(self, PHalf::Second);
    }

    match self.seq_step {
      2 | 6 => self.ch1.step_sweep(),
      7 => {
        self.ch1.step_env();
        self.ch2.step_env();
        self.ch4.step_env();
      }
      _ => (),
    }

    self.seq_step = (self.seq_step + 1) & 7;
  }

  fn mix_and_render(&self) -> (Sample, Sample) {
    let mut l = 0;
    let mut r = 0;

    for i in 0..4 {
      let out = match i {
        0 => self.ch1.out() * u8::from(self.ch1.true_on()),
        1 => self.ch2.out() * u8::from(self.ch2.true_on()),
        2 => self.ch3.out() * u8::from(self.ch3.true_on()),
        3 => self.ch4.out() * u8::from(self.ch4.true_on()),
        _ => break,
      };

      let right_on = u8::from(self.nr51 & (1 << i) != 0);
      let left_on = u8::from(self.nr51 & (1 << (i + 4)) != 0);

      l += left_on * out;
      r += right_on * out;
    }

    // transform to i16 sample
    let l = (0xF - i16::from(l) * 2) * i16::from(self.l_vol);
    let r = (0xF - i16::from(r) * 2) * i16::from(self.r_vol);

    // amplify
    let l = l * 32;
    let r = r * 32;

    (l, r)
  }

  fn reset(&mut self) {
    self.render_timer = 0;
    self.ch1.reset();
    self.ch2.reset();
    self.ch3.reset();
    self.ch4.reset();
    self.l_vol = 0;
    self.l_vin = false;
    self.r_vol = 0;
    self.r_vin = false;
    self.nr51 = 0;
  }

  #[must_use]
  pub fn read_nr50(&self) -> u8 {
    self.r_vol
      | u8::from(self.r_vin) << 3
      | self.l_vol << 4
      | u8::from(self.l_vin) << 7
  }

  #[must_use]
  pub const fn read_nr51(&self) -> u8 { self.nr51 }

  #[must_use]
  pub fn read_nr52(&self) -> u8 {
    u8::from(self.on) << 7
      | 0x70
      | u8::from(self.ch4.on()) << 3
      | u8::from(self.ch3.on()) << 2
      | u8::from(self.ch2.on()) << 1
      | u8::from(self.ch1.on())
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
      self.reset();
    }
  }

  // IO
  pub const fn on(&self) -> bool { self.on }

  pub const fn pcm12(&self) -> u8 { self.ch1.out() | (self.ch1.out() << 4) }

  pub fn pcm34(&self) -> u8 { self.ch3.out() | (self.ch4.out() << 4) }

  // Channel 1
  pub fn read_nr10(&self) -> u8 { self.ch1.read_nr10() }

  pub const fn read_nr11(&self) -> u8 { self.ch1.read_nr11() }

  pub const fn read_nr12(&self) -> u8 { self.ch1.read_nr12() }

  pub fn read_nr14(&self) -> u8 { self.ch1.read_nr14() }

  pub fn write_nr10(&mut self, val: u8) { self.ch1.write_nr10(val); }

  pub fn write_nr11(&mut self, val: u8) { self.ch1.write_nr11(val); }

  pub fn write_nr12(&mut self, val: u8) { self.ch1.write_nr12(val); }

  pub fn write_nr13(&mut self, val: u8) { self.ch1.write_nr13(val); }

  pub fn write_nr14(&mut self, val: u8) { self.ch1.write_nr14(val); }

  // Channel 2
  pub const fn read_nr21(&self) -> u8 { self.ch2.read_nr21() }

  pub const fn read_nr22(&self) -> u8 { self.ch2.read_nr22() }

  pub fn read_nr24(&self) -> u8 { self.ch2.read_nr24() }

  pub fn write_nr21(&mut self, val: u8) { self.ch2.write_nr21(val); }

  pub fn write_nr22(&mut self, val: u8) { self.ch2.write_nr22(val); }

  pub fn write_nr23(&mut self, val: u8) { self.ch2.write_nr23(val); }

  pub fn write_nr24(&mut self, val: u8) { self.ch2.write_nr24(val); }

  // Channel 3
  pub const fn read_nr30(&self) -> u8 { self.ch3.read_nr30() }

  pub const fn read_nr32(&self) -> u8 { self.ch3.read_nr32() }

  pub fn read_nr34(&self) -> u8 { self.ch3.read_nr34() }

  pub const fn read_wave_ram(&self, addr: u8) -> u8 {
    self.ch3.read_wave_ram(addr)
  }

  pub fn write_nr30(&mut self, val: u8) { self.ch3.write_nr30(val); }

  pub fn write_nr31(&mut self, val: u8) { self.ch3.write_nr31(val); }

  pub fn write_nr32(&mut self, val: u8) { self.ch3.write_nr32(val); }

  pub fn write_nr33(&mut self, val: u8) { self.ch3.write_nr33(val); }

  pub fn write_nr34(&mut self, val: u8) { self.ch3.write_nr34(val); }

  pub fn write_wave_ram(&mut self, addr: u8, val: u8) {
    self.ch3.write_wave_ram(addr, val);
  }

  // Channel 4
  pub const fn read_nr42(&self) -> u8 { self.ch4.read_nr42() }

  pub const fn read_nr43(&self) -> u8 { self.ch4.read_nr43() }

  pub fn read_nr44(&self) -> u8 { self.ch4.read_nr44() }

  pub fn write_nr41(&mut self, val: u8) { self.ch4.write_nr41(val); }

  pub fn write_nr42(&mut self, val: u8) { self.ch4.write_nr42(val); }

  pub fn write_nr43(&mut self, val: u8) { self.ch4.write_nr43(val); }

  pub fn write_nr44(&mut self, val: u8) { self.ch4.write_nr44(val); }
}

mod channel {
  use super::PHalf;

  struct WaveLength<const PERIOD_MUL: u16> {
    period: i32,
  }

  impl<const PERIOD_MUL: u16> WaveLength<PERIOD_MUL> {
    const fn new(freq: u16) -> Self {
      Self {
        period: Self::calc_period(freq),
      }
    }

    fn trigger(&mut self, freq: u16) { self.reset(freq); }

    fn reset(&mut self, freq: u16) { self.period = Self::calc_period(freq); }

    fn step(&mut self, t_cycles: i32, freq: u16) -> bool {
      self.period -= t_cycles;

      if self.period < 0 {
        self.period += Self::calc_period(freq);
        return true;
      }

      false
    }

    const fn calc_period(freq: u16) -> i32 {
      const MAX_FREQ: u16 = 2048;
      (PERIOD_MUL * (MAX_FREQ - freq)) as i32
    }
  }

  // TODO: wiki says max is 64 for all
  struct LengthTimer<const MAX_LEN: u8> {
    on:     bool,
    len:    u8,
    p_half: PHalf,
  }

  impl<const MAX_LEN: u8> Default for LengthTimer<MAX_LEN> {
    fn default() -> Self {
      Self {
        on:     false,
        len:    MAX_LEN,
        p_half: PHalf::default(),
      }
    }
  }

  impl<const MAX_LEN: u8> LengthTimer<MAX_LEN> {
    fn reset(&mut self) {
      self.on = false;
      self.len = MAX_LEN;
    }

    fn read_on(&self) -> u8 { u8::from(self.on) << 6 }

    fn write_on(&mut self, val: u8, on: &mut bool) {
      let old_len = self.on;
      self.on = val & 0x40 != 0;

      if self.on && !old_len && self.p_half == PHalf::First {
        self.step(on);
      }
    }

    fn write_len(&mut self, val: u8) { self.len = val; }

    fn trigger(&mut self, on: &mut bool) {
      if self.len > MAX_LEN {
        self.len = 0;
        if self.on && self.p_half == PHalf::First {
          self.step(on);
        }
      }
    }

    fn step(&mut self, on: &mut bool) {
      if self.on && self.len <= MAX_LEN {
        self.len += 1;

        if self.len > MAX_LEN {
          *on = false;
        }
      }
    }

    fn set_phalf(&mut self, p_half: PHalf) { self.p_half = p_half; }
  }

  #[derive(Clone, Copy, Default, PartialEq, Eq)]
  enum EnvDirection {
    #[default]
    Dec = 0,
    Inc = 1,
  }

  impl EnvDirection {
    const fn from_u8(val: u8) -> Self {
      if val & 8 == 0 { Self::Dec } else { Self::Inc }
    }

    const fn to_u8(self) -> u8 { (self as u8) << 3 }
  }

  struct Envelope {
    on:  bool,
    inc: EnvDirection,

    // between 0 and F
    vol:     u8,
    vol_reg: u8,

    period: u8,
    timer:  u8,
  }

  impl Envelope {
    const fn read(&self) -> u8 {
      self.vol_reg << 4 | self.inc.to_u8() | self.period & 7
    }

    fn write(&mut self, val: u8) {
      self.period = val & 7;
      self.on = self.period != 0;
      if self.period == 0 {
        self.period = 8;
      }
      self.timer = 0;
      self.inc = EnvDirection::from_u8(val);
      self.vol_reg = val >> 4;
      self.vol = self.vol_reg;
    }

    fn step(&mut self) {
      if self.inc == EnvDirection::Inc && self.vol == 0xF
        || self.inc == EnvDirection::Dec && self.vol == 0
      {
        self.on = false;
        return;
      }

      self.timer += 1;

      if self.timer > self.period {
        self.timer = 0;

        match self.inc {
          EnvDirection::Inc => self.vol += 1,
          EnvDirection::Dec => self.vol -= 1,
        }
      }
    }

    fn trigger(&mut self) {
      self.timer = 0;
      self.vol = self.vol_reg;
    }

    const fn on(&self) -> bool { self.on }

    const fn vol(&self) -> u8 { self.vol }
  }

  impl Default for Envelope {
    fn default() -> Self {
      Self {
        period:  8,
        timer:   0,
        on:      false,
        vol_reg: 0,
        vol:     0,
        inc:     EnvDirection::default(),
      }
    }
  }

  #[allow(clippy::module_name_repetitions)]
  pub mod square {
    use crate::apu::channel::{LengthTimer, WaveLength};

    mod sweep {
      use core::num::NonZeroU8;

      pub(super) trait SweepTrait: Sized + Default {
        fn reset(&mut self);
        fn calculate_sweep(&mut self, freq: &mut u16, on: &mut bool);
        fn read(&self) -> u8;
        fn write(&mut self, val: u8);
        fn step(&mut self, freq: &mut u16, on: &mut bool);
        fn trigger(&mut self, freq: &mut u16, on: &mut bool);
      }

      #[derive(Clone, Copy, Default)]
      enum SweepDirection {
        #[default]
        Add = 0,
        Sub = 1,
      }

      impl SweepDirection {
        const fn from_u8(val: u8) -> Self {
          if val & 8 == 0 { Self::Add } else { Self::Sub }
        }

        const fn to_u8(self) -> u8 { (self as u8) << 3 }
      }

      pub(in crate::apu) struct Sweep {
        // TODO: check on behaviour
        on:          bool,
        dir:         SweepDirection,
        // 0 is treated as 8
        period:      NonZeroU8,
        // shift between 0 and 7
        shift:       u8,
        timer:       u8,
        // Shadow frequency
        shadow_freq: u16,
      }

      impl SweepTrait for Sweep {
        fn reset(&mut self) {
          self.period = NonZeroU8::new(8).unwrap();
          self.timer = 0;
          self.dir = SweepDirection::default();
          self.shift = 0;
          self.on = false;
        }

        fn calculate_sweep(&mut self, freq: &mut u16, on: &mut bool) {
          let tmp = self.shadow_freq >> self.shift;
          self.shadow_freq = match self.dir {
            SweepDirection::Sub => self.shadow_freq - tmp,
            SweepDirection::Add => self.shadow_freq + tmp,
          };

          if self.shadow_freq > 0x7FF {
            self.on = false;
            *on = false;
            return;
          }

          if self.shift != 0 {
            *freq = self.shadow_freq & 0x7FF;
          }
        }

        fn read(&self) -> u8 {
          // we treat 0 period as 8 so mask it
          0x80 | ((self.period.get() & 7) << 4) | self.dir.to_u8() | self.shift
        }

        fn write(&mut self, val: u8) {
          let period = (val >> 4) & 7;
          self.period = if period == 0 {
            NonZeroU8::new(8).unwrap()
          } else {
            NonZeroU8::new(period).unwrap()
          };

          self.dir = SweepDirection::from_u8(val);
          self.shift = val & 7;
        }

        fn step(&mut self, freq: &mut u16, on: &mut bool) {
          self.timer += 1;
          if self.timer > self.period.get() {
            self.timer = 0;
            if self.on {
              self.calculate_sweep(freq, on);
            }
          }
        }

        fn trigger(&mut self, freq: &mut u16, on: &mut bool) {
          self.shadow_freq = *freq;
          self.timer = 0;
          self.on = self.period.get() != 8 && self.shift != 0;

          if self.shift != 0 {
            self.calculate_sweep(freq, on);
          }
        }
      }

      impl Default for Sweep {
        fn default() -> Self {
          Self {
            period:      NonZeroU8::new(8).unwrap(),
            dir:         SweepDirection::default(),
            shift:       0,
            timer:       0,
            shadow_freq: 0,
            on:          false,
          }
        }
      }

      #[derive(Default)]
      pub(super) struct NoSweep;

      impl SweepTrait for NoSweep {
        fn reset(&mut self) {}

        fn calculate_sweep(&mut self, _: &mut u16, _: &mut bool) {}

        fn read(&self) -> u8 { 0xFF }

        fn write(&mut self, _: u8) {}

        fn step(&mut self, _: &mut u16, _: &mut bool) {}

        fn trigger(&mut self, _: &mut u16, _: &mut bool) {}
      }
    }

    use {super::Envelope, crate::apu::PHalf};

    struct AbstractSquare<Sw: sweep::SweepTrait> {
      ltimer: LengthTimer<64>,
      wvf:    WaveLength<4>,
      env:    Envelope,
      sweep:  Sw,

      on:     bool,
      dac_on: bool,
      out:    u8,

      freq:     u16, // 11 bit frequency data
      duty:     u8,
      duty_bit: u8,
    }

    impl<Sw: sweep::SweepTrait> Default for AbstractSquare<Sw> {
      fn default() -> Self {
        let freq = 0;
        Self {
          sweep: Sw::default(),
          freq,
          wvf: WaveLength::new(freq),
          duty: 0,
          duty_bit: 0,
          on: false,
          dac_on: true,
          ltimer: LengthTimer::default(),
          env: Envelope::default(),
          out: 0,
        }
      }
    }

    impl<Sw: sweep::SweepTrait> AbstractSquare<Sw> {
      fn read0(&self) -> u8 { self.sweep.read() }

      const fn read1(&self) -> u8 { 0x3F | (self.duty << 6) }

      const fn read2(&self) -> u8 { self.env.read() }

      fn read4(&self) -> u8 { 0xBF | self.ltimer.read_on() }

      fn write0(&mut self, val: u8) { self.sweep.write(val); }

      fn write1(&mut self, val: u8) {
        self.duty = (val >> 6) & 3;
        self.ltimer.write_len(val);
      }

      fn write2(&mut self, val: u8) {
        if val & 0xF8 == 0 {
          self.on = false;
          self.dac_on = false;
        } else {
          self.dac_on = true;
        }

        self.env.write(val);
      }

      fn write3(&mut self, val: u8) {
        self.freq = (self.freq & 0x700) | u16::from(val);
      }

      fn write4(&mut self, val: u8) {
        self.freq = ((u16::from(val) & 7) << 8) | (self.freq & 0xFF);

        self.ltimer.write_on(val, &mut self.on);

        // trigger
        if val & 0x80 != 0 {
          if self.dac_on {
            self.on = true;
          }

          self.out = 0;
          self.ltimer.trigger(&mut self.on);
          self.wvf.trigger(self.freq);
          self.env.trigger();
          self.sweep.trigger(&mut self.freq, &mut self.on);
        }
      }

      fn reset(&mut self) {
        self.on = false;
        self.dac_on = true;
        self.ltimer.reset();
        self.freq = 0;
        self.duty = 0;
        self.duty_bit = 0;
        self.env = Envelope::default();
        self.sweep.reset();
        self.wvf.reset(self.freq);
        self.out = 0;
      }

      fn step_sample(&mut self, cycles: i32) {
        // Shape of the duty waveform for a certain duty
        const DUTY_WAV: [u8; 4] = [
          // _______- : 12.5%
          0b0000_0001,
          // -______- : 25%
          0b1000_0001,
          // -____--- : 50%
          0b1000_0111,
          // _------_ : 75%
          0b0111_1110,
        ];

        if !self.on {
          return;
        }

        if self.wvf.step(cycles, self.freq) {
          self.duty_bit = (self.duty_bit + 1) & 7;
          self.out = u8::from(
            (DUTY_WAV[self.duty as usize] & (1 << self.duty_bit)) != 0,
          );
        }
      }

      fn step_sweep(&mut self) {
        if self.on {
          self.sweep.step(&mut self.freq, &mut self.on);
        }
      }

      fn step_env(&mut self) {
        if !(self.env.on() && self.on) {
          return;
        }

        self.env.step();
      }

      const fn out(&self) -> u8 { self.out * self.env.vol() }

      const fn true_on(&self) -> bool { self.on && self.dac_on }

      fn step_len(&mut self) { self.ltimer.step(&mut self.on); }

      fn set_period_half(&mut self, p_half: PHalf) {
        self.ltimer.set_phalf(p_half);
      }

      const fn on(&self) -> bool { self.on }
    }

    #[derive(Default)]
    pub struct Square1 {
      ab: AbstractSquare<sweep::Sweep>,
    }

    impl Square1 {
      pub(in crate::apu) fn read_nr10(&self) -> u8 { self.ab.read0() }

      pub(in crate::apu) const fn read_nr11(&self) -> u8 { self.ab.read1() }

      pub(in crate::apu) const fn read_nr12(&self) -> u8 { self.ab.read2() }

      pub(in crate::apu) fn read_nr14(&self) -> u8 { self.ab.read4() }

      pub(in crate::apu) fn write_nr10(&mut self, val: u8) {
        self.ab.write0(val);
      }

      pub(in crate::apu) fn write_nr11(&mut self, val: u8) {
        self.ab.write1(val);
      }

      pub(in crate::apu) fn write_nr12(&mut self, val: u8) {
        self.ab.write2(val);
      }

      pub(in crate::apu) fn write_nr13(&mut self, val: u8) {
        self.ab.write3(val);
      }

      pub(in crate::apu) fn write_nr14(&mut self, val: u8) {
        self.ab.write4(val);
      }

      pub(in crate::apu) const fn out(&self) -> u8 { self.ab.out() }

      pub(in crate::apu) fn reset(&mut self) { self.ab.reset(); }

      pub(in crate::apu) fn step_sample(&mut self, cycles: i32) {
        self.ab.step_sample(cycles);
      }

      pub(in crate::apu) fn step_sweep(&mut self) { self.ab.step_sweep(); }

      pub(in crate::apu) fn step_env(&mut self) { self.ab.step_env(); }

      pub(in crate::apu) const fn true_on(&self) -> bool { self.ab.true_on() }

      pub(in crate::apu) fn step_len(&mut self) { self.ab.step_len(); }

      pub(in crate::apu) fn set_period_half(&mut self, p_half: PHalf) {
        self.ab.set_period_half(p_half);
      }

      pub(in crate::apu) const fn on(&self) -> bool { self.ab.on() }
    }

    #[derive(Default)]
    pub struct Square2 {
      ab: AbstractSquare<sweep::NoSweep>,
    }

    impl Square2 {
      pub(in crate::apu) const fn read_nr21(&self) -> u8 { self.ab.read1() }

      pub(in crate::apu) const fn read_nr22(&self) -> u8 { self.ab.read2() }

      pub(in crate::apu) fn read_nr24(&self) -> u8 { self.ab.read4() }

      pub(in crate::apu) fn write_nr21(&mut self, val: u8) {
        self.ab.write1(val);
      }

      pub(in crate::apu) fn write_nr22(&mut self, val: u8) {
        self.ab.write2(val);
      }

      pub(in crate::apu) fn write_nr23(&mut self, val: u8) {
        self.ab.write3(val);
      }

      pub(in crate::apu) fn write_nr24(&mut self, val: u8) {
        self.ab.write4(val);
      }

      pub(in crate::apu) const fn out(&self) -> u8 { self.ab.out() }

      pub(in crate::apu) fn reset(&mut self) { self.ab.reset(); }

      pub(in crate::apu) fn step_sample(&mut self, cycles: i32) {
        self.ab.step_sample(cycles);
      }

      pub(in crate::apu) fn step_env(&mut self) { self.ab.step_env(); }

      pub(in crate::apu) const fn true_on(&self) -> bool { self.ab.true_on() }

      pub(in crate::apu) fn step_len(&mut self) { self.ab.step_len(); }

      pub(in crate::apu) fn set_period_half(&mut self, p_half: PHalf) {
        self.ab.set_period_half(p_half);
      }

      pub(in crate::apu) const fn on(&self) -> bool { self.ab.on() }
    }
  }

  pub mod noise {
    use {
      super::Envelope,
      crate::apu::{channel::LengthTimer, PHalf},
    };

    pub struct Noise {
      ltimer: LengthTimer<64>,
      env:    Envelope,

      on:           bool,
      dac_on:       bool,
      timer:        i32,
      timer_period: u16,
      lfsr:         u16,
      wide_step:    bool,
      out:          u8,
      nr43:         u8,
    }

    impl Default for Noise {
      fn default() -> Self {
        Self {
          on:           false,
          dac_on:       true,
          ltimer:       LengthTimer::default(),
          env:          Envelope::default(),
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
      pub(in crate::apu) const fn read_nr42(&self) -> u8 { self.env.read() }

      pub(in crate::apu) const fn read_nr43(&self) -> u8 { self.nr43 }

      pub(in crate::apu) fn read_nr44(&self) -> u8 {
        0xBF | self.ltimer.read_on()
      }

      pub(in crate::apu) fn write_nr41(&mut self, val: u8) {
        self.ltimer.write_len(val);
      }

      pub(in crate::apu) fn write_nr42(&mut self, val: u8) {
        if val & 0xF8 == 0 {
          self.on = false;
          self.dac_on = false;
        } else {
          self.dac_on = true;
        }

        self.env.write(val);
      }

      pub(in crate::apu) fn write_nr43(&mut self, val: u8) {
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

      pub(in crate::apu) fn write_nr44(&mut self, val: u8) {
        self.ltimer.write_on(val, &mut self.on);

        // trigger
        if val & 0x80 != 0 {
          if self.dac_on {
            self.on = true;
          }

          self.ltimer.trigger(&mut self.on);

          self.timer = i32::from(self.timer_period);
          self.lfsr = 0x7FFF;
          self.env.trigger();
        }
      }

      pub(in crate::apu) const fn out(&self) -> u8 {
        self.out * self.env.vol()
      }

      pub(in crate::apu) fn reset(&mut self) {
        self.on = false;
        self.dac_on = true;
        self.ltimer.reset();
        self.env = Envelope::default();
        self.timer_period = 0;
        self.timer = 1;
        // linear feedback shift register
        self.lfsr = 0x7FFF;
        self.wide_step = false;
        self.out = 0;
        self.nr43 = 0;
      }

      pub(in crate::apu) fn step_env(&mut self) {
        if !(self.env.on() && self.on && self.dac_on) {
          return;
        }

        self.env.step();
      }

      pub(in crate::apu) fn step_sample(&mut self, cycles: i32) {
        if !self.true_on() {
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

      pub(in crate::apu) const fn true_on(&self) -> bool {
        self.on && self.dac_on
      }

      pub(in crate::apu) fn step_len(&mut self) {
        self.ltimer.step(&mut self.on);
      }

      pub(in crate::apu) fn set_period_half(&mut self, p_half: PHalf) {
        self.ltimer.set_phalf(p_half);
      }

      pub(in crate::apu) const fn on(&self) -> bool { self.on }
    }
  }

  pub mod wave {
    use {
      super::{LengthTimer, WaveLength},
      crate::apu::PHalf,
    };

    const RAM_LEN: u8 = 0x10;
    const SAMPLE_LEN: u8 = RAM_LEN * 2;

    pub struct Wave {
      ltimer: LengthTimer<64>,

      on:            bool,
      dac_on:        bool,
      freq:          u16, // 11 bit frequency data
      wvf:           WaveLength<2>,
      sample_buffer: u8,
      ram:           [u8; RAM_LEN as usize],
      samples:       [u8; SAMPLE_LEN as usize],
      sample_idx:    u8,
      vol:           u8,
      nr30:          u8,
    }

    impl Default for Wave {
      fn default() -> Self {
        let freq = 0;
        Self {
          ram: [0; RAM_LEN as usize],
          vol: 0,
          on: false,
          dac_on: true,
          ltimer: LengthTimer::default(),
          freq,
          sample_buffer: 0,
          samples: [0; SAMPLE_LEN as usize],
          sample_idx: 0,
          nr30: 0,
          wvf: WaveLength::new(freq),
        }
      }
    }

    impl Wave {
      pub(in crate::apu) const fn read_wave_ram(&self, addr: u8) -> u8 {
        let index = (addr - 0x30) as usize;
        self.ram[index]
      }

      pub(in crate::apu) fn write_wave_ram(&mut self, addr: u8, val: u8) {
        let index = (addr - 0x30) as usize;
        self.ram[index] = val;
        // upper 4 bits first
        self.samples[index * 2] = val >> 4;
        self.samples[index * 2 + 1] = val & 0xF;
      }

      pub(in crate::apu) const fn read_nr30(&self) -> u8 { self.nr30 | 0x7F }

      pub(in crate::apu) const fn read_nr32(&self) -> u8 {
        0x9F | (self.vol << 5)
      }

      pub(in crate::apu) fn read_nr34(&self) -> u8 {
        0xBF | self.ltimer.read_on()
      }

      pub(in crate::apu) fn write_nr30(&mut self, val: u8) {
        self.nr30 = val;
        if val & 0x80 == 0 {
          self.on = false;
          self.dac_on = false;
        } else {
          self.dac_on = true;
        }
      }

      pub(in crate::apu) fn write_nr31(&mut self, val: u8) {
        self.ltimer.write_len(val);
      }

      pub(in crate::apu) fn write_nr32(&mut self, val: u8) {
        self.vol = (val >> 5) & 3;
      }

      pub(in crate::apu) fn write_nr33(&mut self, val: u8) {
        self.freq = (self.freq & 0x700) | u16::from(val);
      }

      pub(in crate::apu) fn write_nr34(&mut self, val: u8) {
        self.freq = (u16::from(val) & 7) << 8 | (self.freq & 0xFF);

        self.ltimer.write_on(val, &mut self.on);

        // trigger
        if val & 0x80 != 0 {
          if self.dac_on {
            self.on = true;
          }

          self.ltimer.trigger(&mut self.on);
          self.wvf.trigger(self.freq);
          self.sample_idx = 0;
        }
      }

      pub(in crate::apu) fn out(&self) -> u8 {
        // wrapping_shr is necessary because (vol - 1) can be -1
        self
          .sample_buffer
          .wrapping_shr(u32::from(self.vol.wrapping_sub(1)))
      }

      pub(in crate::apu) fn reset(&mut self) {
        self.on = false;
        self.dac_on = true;
        self.ltimer.reset();
        self.freq = 0;
        self.vol = 0;
        self.sample_idx = 0;
        self.wvf.reset(self.freq);
        self.sample_buffer = 0;
        self.nr30 = 0;
      }

      pub(in crate::apu) fn step_sample(&mut self, cycles: i32) {
        if !self.true_on() {
          return;
        }

        if self.wvf.step(cycles, self.freq) {
          self.sample_idx = (self.sample_idx + 1) % SAMPLE_LEN;
          self.sample_buffer = self.samples[self.sample_idx as usize];
        }
      }

      pub(in crate::apu) const fn true_on(&self) -> bool {
        self.on && self.dac_on
      }

      pub(in crate::apu) fn step_len(&mut self) {
        self.ltimer.step(&mut self.on);
      }

      pub(in crate::apu) fn set_period_half(&mut self, p_half: PHalf) {
        self.ltimer.set_phalf(p_half);
      }

      pub(in crate::apu) const fn on(&self) -> bool { self.on }
    }
  }
}
