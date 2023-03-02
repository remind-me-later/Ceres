use {
  crate::TC_SEC,
  noise::Noise,
  square::{Square1, Square2},
  wave::Wave,
};
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
    fn mix_and_render(apu: &mut Apu) -> (Sample, Sample) {
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
        self.l_out = l;
        self.r_out = r;
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

    // TODO: is this ok?
    if !self.on() {
      return;
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
      0 | 4 => self.ch1.step_sweep(),
      7 => {
        self.ch1.step_env();
        self.ch2.step_env();
        self.ch4.step_env();
      }
      _ => (),
    }

    self.seq_step = (self.seq_step + 1) & 7;
  }

  // IO
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
      self.l_out = 0;
      self.r_out = 0;
      self.seq_step = 0;
    }
  }

  pub const fn on(&self) -> bool { self.on }

  pub const fn pcm12(&self) -> u8 { self.ch1.out() | (self.ch2.out() << 4) }

  pub fn pcm34(&self) -> u8 { self.ch3.out() | (self.ch4.out() << 4) }

  // Channel 1 IO
  pub fn read_nr10(&self) -> u8 { self.ch1.read_nr10() }

  pub const fn read_nr11(&self) -> u8 { self.ch1.read_nr11() }

  pub const fn read_nr12(&self) -> u8 { self.ch1.read_nr12() }

  pub fn read_nr14(&self) -> u8 { self.ch1.read_nr14() }

  pub fn write_nr10(&mut self, val: u8) { self.ch1.write_nr10(val); }

  pub fn write_nr11(&mut self, val: u8) { self.ch1.write_nr11(val); }

  pub fn write_nr12(&mut self, val: u8) { self.ch1.write_nr12(val); }

  pub fn write_nr13(&mut self, val: u8) { self.ch1.write_nr13(val); }

  pub fn write_nr14(&mut self, val: u8) { self.ch1.write_nr14(val); }

  // Channel 2 IO
  pub const fn read_nr21(&self) -> u8 { self.ch2.read_nr21() }

  pub const fn read_nr22(&self) -> u8 { self.ch2.read_nr22() }

  pub fn read_nr24(&self) -> u8 { self.ch2.read_nr24() }

  pub fn write_nr21(&mut self, val: u8) { self.ch2.write_nr21(val); }

  pub fn write_nr22(&mut self, val: u8) { self.ch2.write_nr22(val); }

  pub fn write_nr23(&mut self, val: u8) { self.ch2.write_nr23(val); }

  pub fn write_nr24(&mut self, val: u8) { self.ch2.write_nr24(val); }

  // Channel 3 IO
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

  // Channel 4 IO
  pub const fn read_nr42(&self) -> u8 { self.ch4.read_nr42() }

  pub const fn read_nr43(&self) -> u8 { self.ch4.read_nr43() }

  pub fn read_nr44(&self) -> u8 { self.ch4.read_nr44() }

  pub fn write_nr41(&mut self, val: u8) { self.ch4.write_nr41(val); }

  pub fn write_nr42(&mut self, val: u8) { self.ch4.write_nr42(val); }

  pub fn write_nr43(&mut self, val: u8) { self.ch4.write_nr43(val); }

  pub fn write_nr44(&mut self, val: u8) { self.ch4.write_nr44(val); }
}

struct WaveLength<const PERIOD_MUL: u16> {
  period: i32,
}

impl<const PERIOD_MUL: u16> Default for WaveLength<PERIOD_MUL> {
  fn default() -> Self { Self { period: Self::calc_period(0) } }
}

impl<const PERIOD_MUL: u16> WaveLength<PERIOD_MUL> {
  fn trigger(&mut self, freq: u16) { self.period = Self::calc_period(freq); }

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

#[derive(Default)]
struct LengthTimer<const MAX_LEN: u16> {
  on:     bool,
  len:    u16,
  p_half: PHalf,
}

impl<const MAX_LEN: u16> LengthTimer<MAX_LEN> {
  fn read_on(&self) -> u8 { u8::from(self.on) << 6 }

  fn write_on(&mut self, val: u8, on: &mut bool) {
    let was_off = !self.on;
    self.on = val & 0x40 != 0;

    if was_off && self.p_half == PHalf::First {
      self.step(on);
    }
  }

  fn write_len(&mut self, val: u8) {
    self.len = MAX_LEN - (u16::from(val) & (MAX_LEN - 1));
  }

  fn trigger(&mut self, on: &mut bool) {
    if self.len == 0 {
      self.len = MAX_LEN;
      if self.p_half == PHalf::First {
        self.step(on);
      }
    }
  }

  fn step(&mut self, on: &mut bool) {
    if self.on {
      if self.len == 0 {
        *on = false;
      } else {
        self.len -= 1;
      }
    }
  }

  fn set_phalf(&mut self, p_half: PHalf) { self.p_half = p_half; }
}

mod envelope {
  #[derive(Clone, Copy, Default, PartialEq, Eq)]
  enum EnvelopeDirection {
    #[default]
    Dec = 0,
    Inc = 1,
  }

  impl EnvelopeDirection {
    const fn from_u8(val: u8) -> Self {
      if val & 8 == 0 { Self::Dec } else { Self::Inc }
    }

    const fn to_u8(self) -> u8 { (self as u8) << 3 }
  }

  #[derive(Default)]
  pub(super) struct Envelope {
    on:  bool,
    inc: EnvelopeDirection,

    // between 0 and F
    vol:     u8,
    vol_reg: u8,

    period: u8,
    timer:  u8,
  }

  impl Envelope {
    pub(super) const fn read(&self) -> u8 {
      self.vol_reg << 4 | self.inc.to_u8() | self.period
    }

    pub(super) fn write(&mut self, val: u8) {
      self.period = val & 7;
      self.on = self.period != 0;

      self.timer = 0;
      self.inc = EnvelopeDirection::from_u8(val);
      self.vol_reg = val >> 4;
      self.vol = self.vol_reg;
    }

    pub(super) fn step(&mut self) {
      if !self.on {
        return;
      }

      if self.inc == EnvelopeDirection::Inc && self.vol == 0xF
        || self.inc == EnvelopeDirection::Dec && self.vol == 0
      {
        self.on = false;
        return;
      }

      self.timer += 1;

      if self.timer > self.period {
        self.timer = 0;

        match self.inc {
          EnvelopeDirection::Inc => self.vol += 1,
          EnvelopeDirection::Dec => self.vol -= 1,
        }
      }
    }

    pub(super) fn trigger(&mut self) {
      self.timer = 0;
      self.vol = self.vol_reg;
    }

    pub(super) const fn vol(&self) -> u8 { self.vol }
  }
}

#[allow(clippy::module_name_repetitions)]
pub mod square {
  use {
    super::envelope::Envelope,
    crate::apu::{LengthTimer, PHalf, WaveLength},
  };

  mod sweep {
    use core::num::NonZeroU8;

    pub(super) trait SweepTrait: Sized + Default {
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
      on:  bool,
      dir: SweepDirection,

      // between 0 and 7
      pace:  u8,
      // 0 is treated as 8
      shpc:  NonZeroU8,
      // shift between 0 and 7
      shift: u8,
      timer: u8,
      // Shadow frequency
      shwf:  u16,
    }

    impl Sweep {
      fn calculate_sweep(&mut self, freq: &mut u16, on: &mut bool) {
        {
          let t = self.shwf >> self.shift;
          self.shwf = match self.dir {
            SweepDirection::Sub => self.shwf - t,
            SweepDirection::Add => self.shwf + t,
          };
        }

        if self.shwf > 0x7FF {
          *on = false;
          return;
        }

        if self.shift != 0 {
          *freq = self.shwf & 0x7FF;
        }
      }
    }

    impl SweepTrait for Sweep {
      fn read(&self) -> u8 {
        0x80 | (self.pace << 4) | self.dir.to_u8() | self.shift
      }

      fn write(&mut self, val: u8) {
        self.pace = (val >> 4) & 7;

        if self.pace == 0 {
          self.on = false;
        } else if self.shpc.get() == 8 {
          self.shpc = NonZeroU8::new(self.pace).unwrap();
        }

        self.dir = SweepDirection::from_u8(val);
        self.shift = val & 7;
      }

      fn step(&mut self, freq: &mut u16, on: &mut bool) {
        if !self.on {
          return;
        }

        self.timer += 1;
        if self.timer > self.shpc.get() {
          self.timer = 0;
          self.calculate_sweep(freq, on);
          self.shpc =
            NonZeroU8::new(if self.pace == 0 { 8 } else { self.pace })
              .unwrap();
        }
      }

      fn trigger(&mut self, freq: &mut u16, on: &mut bool) {
        self.shwf = *freq;
        self.timer = 0;
        self.on = self.pace > 0 && self.shift != 0;
        self.shpc =
          NonZeroU8::new(if self.pace == 0 { 8 } else { self.pace }).unwrap();

        if self.shift != 0 {
          self.calculate_sweep(freq, on);
        }
      }
    }

    impl Default for Sweep {
      fn default() -> Self {
        Self {
          shpc:  NonZeroU8::new(8).unwrap(),
          pace:  0,
          dir:   SweepDirection::default(),
          shift: 0,
          timer: 0,
          shwf:  0,
          on:    false,
        }
      }
    }

    impl SweepTrait for () {
      fn read(&self) -> u8 { 0xFF }

      fn write(&mut self, _: u8) {}

      fn step(&mut self, _: &mut u16, _: &mut bool) {}

      fn trigger(&mut self, _: &mut u16, _: &mut bool) {}
    }
  }

  #[derive(Default)]
  struct AbstractSquare<S: sweep::SweepTrait> {
    ltimer: LengthTimer<64>,
    wl:     WaveLength<4>,
    env:    Envelope,
    sweep:  S,

    on:     bool,
    dac_on: bool,
    out:    u8,

    freq:     u16, // 11 bit frequency data
    duty:     u8,
    duty_bit: u8,
  }

  impl<S: sweep::SweepTrait> AbstractSquare<S> {
    fn read_nrx0(&self) -> u8 { self.sweep.read() }

    const fn read_nrx1(&self) -> u8 { 0x3F | (self.duty << 6) }

    const fn read_nrx2(&self) -> u8 { self.env.read() }

    fn read_nrx4(&self) -> u8 { 0xBF | self.ltimer.read_on() }

    fn write_nrx0(&mut self, val: u8) { self.sweep.write(val); }

    fn write_nrx1(&mut self, val: u8) {
      self.duty = (val >> 6) & 3;
      self.ltimer.write_len(val);
    }

    fn write_nrx2(&mut self, val: u8) {
      if val & 0xF8 == 0 {
        self.on = false;
        self.dac_on = false;
      } else {
        self.dac_on = true;
      }

      self.env.write(val);
    }

    fn write_nrx3(&mut self, val: u8) {
      self.freq = (self.freq & 0x700) | u16::from(val);
    }

    fn write_nrx4(&mut self, val: u8) {
      self.freq = (u16::from(val & 7) << 8) | (self.freq & 0xFF);

      self.ltimer.write_on(val, &mut self.on);

      // trigger
      if val & 0x80 != 0 {
        if self.dac_on {
          self.on = true;
        }

        self.out = 0;
        self.wl.trigger(self.freq);
        self.env.trigger();
        self.ltimer.trigger(&mut self.on);
        self.sweep.trigger(&mut self.freq, &mut self.on);
      }
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

      if self.wl.step(cycles, self.freq) {
        self.duty_bit = (self.duty_bit + 1) & 7;
        self.out =
          u8::from((DUTY_WAV[self.duty as usize] & (1 << self.duty_bit)) != 0);
      }
    }

    fn step_sweep(&mut self) {
      if self.on {
        self.sweep.step(&mut self.freq, &mut self.on);
      }
    }

    fn step_env(&mut self) {
      if self.on {
        self.env.step();
      }
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
    pub(in crate::apu) fn read_nr10(&self) -> u8 { self.ab.read_nrx0() }

    pub(in crate::apu) const fn read_nr11(&self) -> u8 { self.ab.read_nrx1() }

    pub(in crate::apu) const fn read_nr12(&self) -> u8 { self.ab.read_nrx2() }

    pub(in crate::apu) fn read_nr14(&self) -> u8 { self.ab.read_nrx4() }

    pub(in crate::apu) fn write_nr10(&mut self, val: u8) {
      self.ab.write_nrx0(val);
    }

    pub(in crate::apu) fn write_nr11(&mut self, val: u8) {
      self.ab.write_nrx1(val);
    }

    pub(in crate::apu) fn write_nr12(&mut self, val: u8) {
      self.ab.write_nrx2(val);
    }

    pub(in crate::apu) fn write_nr13(&mut self, val: u8) {
      self.ab.write_nrx3(val);
    }

    pub(in crate::apu) fn write_nr14(&mut self, val: u8) {
      self.ab.write_nrx4(val);
    }

    pub(in crate::apu) const fn out(&self) -> u8 { self.ab.out() }

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
    ab: AbstractSquare<()>,
  }

  impl Square2 {
    pub(in crate::apu) const fn read_nr21(&self) -> u8 { self.ab.read_nrx1() }

    pub(in crate::apu) const fn read_nr22(&self) -> u8 { self.ab.read_nrx2() }

    pub(in crate::apu) fn read_nr24(&self) -> u8 { self.ab.read_nrx4() }

    pub(in crate::apu) fn write_nr21(&mut self, val: u8) {
      self.ab.write_nrx1(val);
    }

    pub(in crate::apu) fn write_nr22(&mut self, val: u8) {
      self.ab.write_nrx2(val);
    }

    pub(in crate::apu) fn write_nr23(&mut self, val: u8) {
      self.ab.write_nrx3(val);
    }

    pub(in crate::apu) fn write_nr24(&mut self, val: u8) {
      self.ab.write_nrx4(val);
    }

    pub(in crate::apu) const fn out(&self) -> u8 { self.ab.out() }

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
    super::envelope::Envelope,
    crate::apu::{LengthTimer, PHalf},
  };

  pub struct Noise {
    ltimer: LengthTimer<64>,
    env:    Envelope,

    on:           bool,
    dac_on:       bool,
    timer:        i32,
    timer_period: u16,
    // linear feedback shift register
    lfsr:         u16,
    wide_step:    bool,
    out:          u8,
    nr43:         u8,
  }

  impl Default for Noise {
    fn default() -> Self {
      Self {
        on:           false,
        dac_on:       false,
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

    pub(in crate::apu) const fn out(&self) -> u8 { self.out * self.env.vol() }

    pub(in crate::apu) fn step_env(&mut self) {
      if !self.on {
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

  #[derive(Default)]
  pub struct Wave {
    ltimer: LengthTimer<256>,

    on:         bool,
    dac_on:     bool,
    freq:       u16, // 11 bit frequency data
    wl:         WaveLength<2>,
    sample_buf: u8,
    ram:        [u8; RAM_LEN as usize],
    samples:    [u8; SAMPLE_LEN as usize],
    sample_idx: u8,
    vol:        u8,
    nr30:       u8,
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
        self.wl.trigger(self.freq);
        self.sample_idx = 0;
      }
    }

    pub(in crate::apu) fn out(&self) -> u8 {
      // wrapping_shr is necessary because (vol - 1) can be -1
      self.sample_buf.wrapping_shr(u32::from(self.vol.wrapping_sub(1)))
    }

    pub(in crate::apu) fn step_sample(&mut self, cycles: i32) {
      if !self.true_on() {
        return;
      }

      if self.wl.step(cycles, self.freq) {
        self.sample_idx = (self.sample_idx + 1) % SAMPLE_LEN;
        self.sample_buf = self.samples[self.sample_idx as usize];
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

    pub(in crate::apu) fn reset(&mut self) {
      self.vol = 0;
      self.on = false;
      self.dac_on = false;
      self.ltimer = LengthTimer::default();
      self.freq = 0;
      self.sample_buf = 0;
      self.samples = [0; SAMPLE_LEN as usize];
      self.sample_idx = 0;
      self.nr30 = 0;
      self.wl = WaveLength::default();
    }

    pub(in crate::apu) const fn on(&self) -> bool { self.on }
  }
}
