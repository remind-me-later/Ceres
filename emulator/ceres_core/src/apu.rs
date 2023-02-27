pub use channel::{
  envelope::{
    noise::Noise,
    square::{Square1, Square2},
  },
  wave::Wave,
};

use crate::Gb;

pub type Sample = i16;

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum PHalf {
  #[default]
  First,
  Second,
}

impl Gb {
  pub(crate) fn run_apu(&mut self, cycles: i32) {
    if self.apu_on {
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

  pub(crate) fn apu_step_seq(&mut self) {
    fn set_period_half(gb: &mut Gb, p_half: PHalf) {
      gb.apu_ch1.set_period_half(p_half);
      gb.apu_ch2.set_period_half(p_half);
      gb.apu_ch3.set_period_half(p_half);
      gb.apu_ch4.set_period_half(p_half);
    }

    if self.apu_seq_step & 1 == 0 {
      self.apu_ch1.step_len();
      self.apu_ch2.step_len();
      self.apu_ch3.step_len();
      self.apu_ch4.step_len();

      set_period_half(self, PHalf::First);
    } else {
      set_period_half(self, PHalf::Second);
    }

    match self.apu_seq_step {
      2 | 6 => self.apu_ch1.step_sweep(),
      7 => {
        self.apu_ch1.step_env();
        self.apu_ch2.step_env();
        self.apu_ch4.step_env();
      }
      _ => (),
    }

    self.apu_seq_step = (self.apu_seq_step + 1) & 7;
  }

  fn mix_and_render(&self) -> (Sample, Sample) {
    let mut l = 0;
    let mut r = 0;

    for i in 0..4 {
      let out = match i {
        0 => self.apu_ch1.out() * u8::from(self.apu_ch1.true_on()),
        1 => self.apu_ch2.out() * u8::from(self.apu_ch2.true_on()),
        2 => self.apu_ch3.out() * u8::from(self.apu_ch3.true_on()),
        3 => self.apu_ch4.out() * u8::from(self.apu_ch4.true_on()),
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
      | u8::from(self.apu_ch4.on()) << 3
      | u8::from(self.apu_ch3.on()) << 2
      | u8::from(self.apu_ch2.on()) << 1
      | u8::from(self.apu_ch1.on())
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

mod channel {
  use super::PHalf;

  // TODO: wiki says max is 64 for all
  struct LengthTimer<const MAX_LEN: u16> {
    on:     bool,
    len:    u16,
    p_half: PHalf,
  }

  impl<const MAX_LEN: u16> Default for LengthTimer<MAX_LEN> {
    fn default() -> Self {
      Self {
        on:     false,
        len:    MAX_LEN + 1,
        p_half: PHalf::default(),
      }
    }
  }

  impl<const MAX_LEN: u16> LengthTimer<MAX_LEN> {
    fn reset(&mut self) {
      self.on = false;
      self.len = MAX_LEN + 1;
    }

    fn read_on(&self) -> u8 { u8::from(self.on) << 6 }

    fn write_on(&mut self, val: u8, on: &mut bool) {
      let old_len = self.on;
      self.on = val & 0x40 != 0;

      if self.on && !old_len && self.p_half == PHalf::First {
        self.step(on);
      }
    }

    fn write_len(&mut self, val: u8) { self.len = u16::from(val); }

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

  pub mod envelope {
    struct Envelope {
      on:       bool,
      vol:      u8,
      // TODO: enum?
      inc:      bool,
      base_vol: u8,
      period:   u8,
      timer:    u8,
    }

    impl Envelope {
      fn read(&self) -> u8 {
        self.base_vol << 4 | u8::from(self.inc) | self.period & 7
      }

      fn write(&mut self, val: u8) {
        self.period = val & 7;
        self.on = self.period != 0;
        if self.period == 0 {
          self.period = 8;
        }
        self.timer = 0;
        self.inc = val & 8 != 0;
        self.base_vol = val >> 4;
        self.vol = self.base_vol;
      }

      fn step(&mut self) {
        if self.inc && self.vol == 15 || !self.inc && self.vol == 0 {
          self.on = false;
          return;
        }

        self.timer += 1;
        if self.timer > self.period {
          self.timer = 0;

          if self.inc {
            self.vol += 1;
          } else {
            self.vol -= 1;
          }
        }
      }

      fn trigger(&mut self) {
        self.timer = 0;
        self.vol = self.base_vol;
      }

      const fn on(&self) -> bool { self.on }

      const fn vol(&self) -> u8 { self.vol }
    }

    impl Default for Envelope {
      fn default() -> Self {
        Self {
          period:   8,
          timer:    0,
          on:       false,
          base_vol: 0,
          vol:      0,
          inc:      false,
        }
      }
    }

    #[allow(clippy::module_name_repetitions)]
    pub mod square {
      use crate::apu::channel::LengthTimer;

      mod sweep {
        pub(super) trait SweepTrait: Sized + Default {
          fn reset(&mut self);
          fn calculate_sweep(&mut self, freq: &mut u16, on: &mut bool);
          fn read(&self) -> u8;
          fn write(&mut self, val: u8);
          fn step(&mut self, freq: &mut u16, on: &mut bool);
          fn trigger(&mut self, freq: &mut u16, on: &mut bool);
        }

        #[derive(Clone, Copy)]
        enum Direction {
          Inc,
          Dec,
        }

        impl Direction {
          const fn from_u8(val: u8) -> Self {
            if val & 8 == 0 { Self::Inc } else { Self::Dec }
          }

          const fn to_u8(self) -> u8 {
            let r = match self {
              Self::Inc => 0,
              Self::Dec => 1,
            };

            r << 3
          }
        }

        pub(in crate::apu) struct Sweep {
          // TODO: check on behaviour
          on:          bool,
          dir:         Direction,
          period:      u8,
          // shift between 0 and 7, 0 is sometimes treated as 8
          shift:       u8,
          timer:       u8,
          // Shadow frequency
          shadow_freq: u16,
        }

        impl SweepTrait for Sweep {
          fn reset(&mut self) {
            self.period = 8;
            self.timer = 0;
            self.dir = Direction::Inc;
            self.shift = 0;
            self.on = false;
          }

          fn calculate_sweep(&mut self, freq: &mut u16, on: &mut bool) {
            let tmp = self.shadow_freq >> self.shift;
            self.shadow_freq = match self.dir {
              Direction::Dec => self.shadow_freq - tmp,
              Direction::Inc => self.shadow_freq + tmp,
            };

            if self.shadow_freq > 0x7FF {
              // self.on = false;
              *on = false;
              return;
            }

            if self.shift != 0 {
              *freq = self.shadow_freq & 0x7FF;
            }
          }

          fn read(&self) -> u8 {
            // we treat 0 period as 8 so mask it
            0x80 | ((self.period & 7) << 4) | self.dir.to_u8() | self.shift
          }

          fn write(&mut self, val: u8) {
            self.period = (val >> 4) & 7;
            if self.period == 0 {
              self.period = 8;
            }
            self.dir = Direction::from_u8(val);
            self.shift = val & 7;
          }

          fn step(&mut self, freq: &mut u16, on: &mut bool) {
            self.timer += 1;
            if self.timer > self.period {
              self.timer = 0;
              if self.on {
                self.calculate_sweep(freq, on);
              }
            }
          }

          fn trigger(&mut self, freq: &mut u16, on: &mut bool) {
            self.shadow_freq = *freq;
            self.timer = 0;
            self.on = self.period != 8 && self.shift != 0;

            if self.shift != 0 {
              self.calculate_sweep(freq, on);
            }
          }
        }

        impl Default for Sweep {
          fn default() -> Self {
            Self {
              period:      8,
              dir:         Direction::Inc,
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

      const MAX_LEN: u16 = 64;
      const P_MUL: u16 = 4;
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

      struct AbstractSquare<Sw: sweep::SweepTrait> {
        ltimer: LengthTimer<MAX_LEN>,
        env:    Envelope,
        sweep:  Sw,

        on:       bool,
        dac_on:   bool,
        freq:     u16, // 11 bit frequency data
        duty:     u8,
        duty_bit: u8,
        period:   i32,
        out:      u8,
      }

      impl<Sw: sweep::SweepTrait> Default for AbstractSquare<Sw> {
        fn default() -> Self {
          Self {
            sweep:    Sw::default(),
            freq:     0,
            period:   i32::from(P_MUL) * 2048,
            duty:     DUTY_WAV[0],
            duty_bit: 0,
            on:       false,
            dac_on:   true,
            ltimer:   LengthTimer::default(),
            out:      0,
            env:      Envelope::default(),
          }
        }
      }

      impl<Sw: sweep::SweepTrait> AbstractSquare<Sw> {
        fn reset(&mut self) {
          self.on = false;
          self.dac_on = true;
          self.ltimer.reset();
          self.freq = 0;
          self.duty = DUTY_WAV[0];
          self.duty_bit = 0;
          self.env = Envelope::default();
          self.sweep.reset();
        }

        fn read0(&self) -> u8 { self.sweep.read() }

        const fn read1(&self) -> u8 { 0x3F | (self.duty << 6) }

        fn read2(&self) -> u8 { self.env.read() }

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
          self.freq = (u16::from(val) & 7) << 8 | (self.freq & 0xFF);

          self.ltimer.write_on(val, &mut self.on);

          // trigger
          if val & 0x80 != 0 {
            if self.dac_on {
              self.on = true;
            }

            self.ltimer.trigger(&mut self.on);

            self.period = i32::from(P_MUL * (2048 - self.freq));
            self.out = 0;
            // self.duty_bit = 0;
            self.env.trigger();
            self.sweep.trigger(&mut self.freq, &mut self.on);
          }
        }

        fn step_sample(&mut self, cycles: i32) {
          if !self.true_on() {
            return;
          }

          self.period -= cycles;

          if self.period < 0 {
            self.period += i32::from(P_MUL * (2048 - self.freq));
            self.out = (DUTY_WAV[self.duty as usize] & (1 << self.duty_bit))
              >> self.duty_bit;
            self.duty_bit = (self.duty_bit + 1) & 7;
          }
        }

        fn step_sweep(&mut self) {
          if self.true_on() {
            self.sweep.step(&mut self.freq, &mut self.on);
          }
        }

        fn step_env(&mut self) {
          if !(self.env.on() && self.true_on()) {
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
        pub(crate) fn read_nr10(&self) -> u8 { self.ab.read0() }

        pub(crate) const fn read_nr11(&self) -> u8 { self.ab.read1() }

        pub(crate) fn read_nr12(&self) -> u8 { self.ab.read2() }

        pub(crate) fn read_nr14(&self) -> u8 { self.ab.read4() }

        pub(crate) fn write_nr10(&mut self, val: u8) { self.ab.write0(val); }

        pub(crate) fn write_nr11(&mut self, val: u8) { self.ab.write1(val); }

        pub(crate) fn write_nr12(&mut self, val: u8) { self.ab.write2(val); }

        pub(crate) fn write_nr13(&mut self, val: u8) { self.ab.write3(val); }

        pub(crate) fn write_nr14(&mut self, val: u8) { self.ab.write4(val); }

        pub(crate) const fn out(&self) -> u8 { self.ab.out() }

        pub(in crate::apu) fn reset(&mut self) { self.ab.reset(); }

        pub(in crate::apu) fn step_sample(&mut self, cycles: i32) {
          self.ab.step_sample(cycles);
        }

        pub(in crate::apu) fn step_sweep(&mut self) { self.ab.step_sweep(); }

        pub(in crate::apu) fn step_env(&mut self) { self.ab.step_env(); }

        pub(in crate::apu) const fn true_on(&self) -> bool {
          self.ab.true_on()
        }

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
        pub(crate) const fn read_nr21(&self) -> u8 { self.ab.read1() }

        pub(crate) fn read_nr22(&self) -> u8 { self.ab.read2() }

        pub(crate) fn read_nr24(&self) -> u8 { self.ab.read4() }

        pub(crate) fn write_nr21(&mut self, val: u8) { self.ab.write1(val); }

        pub(crate) fn write_nr22(&mut self, val: u8) { self.ab.write2(val); }

        pub(crate) fn write_nr23(&mut self, val: u8) { self.ab.write3(val); }

        pub(crate) fn write_nr24(&mut self, val: u8) { self.ab.write4(val); }

        pub(crate) const fn out(&self) -> u8 { self.ab.out() }

        pub(in crate::apu) fn reset(&mut self) { self.ab.reset(); }

        pub(in crate::apu) fn step_sample(&mut self, cycles: i32) {
          self.ab.step_sample(cycles);
        }

        pub(in crate::apu) fn step_env(&mut self) { self.ab.step_env(); }

        pub(in crate::apu) const fn true_on(&self) -> bool {
          self.ab.true_on()
        }

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

      const MAX_LEN: u16 = 64;

      pub struct Noise {
        ltimer: LengthTimer<MAX_LEN>,
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
        pub(crate) fn read_nr42(&self) -> u8 { self.env.read() }

        pub(crate) const fn read_nr43(&self) -> u8 { self.nr43 }

        pub(crate) fn read_nr44(&self) -> u8 { 0xBF | self.ltimer.read_on() }

        pub(crate) fn write_nr41(&mut self, val: u8) {
          self.ltimer.write_len(val);
        }

        pub(crate) fn write_nr42(&mut self, val: u8) {
          if val & 0xF8 == 0 {
            self.on = false;
            self.dac_on = false;
          } else {
            self.dac_on = true;
          }

          self.env.write(val);
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

        pub(crate) const fn out(&self) -> u8 { self.out * self.env.vol() }

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
  }

  pub mod wave {
    use {super::LengthTimer, crate::apu::PHalf};

    const MAX_LEN: u16 = 64;
    const RAM_LEN: u8 = 0x10;
    const SAMPLE_LEN: u8 = RAM_LEN * 2;
    const PERIOD_MUL: u16 = 2;

    pub struct Wave {
      l_timer: LengthTimer<MAX_LEN>,

      on:            bool,
      dac_on:        bool,
      freq:          u16, // 11 bit frequency data
      period:        i32,
      sample_buffer: u8,
      ram:           [u8; RAM_LEN as usize],
      samples:       [u8; SAMPLE_LEN as usize],
      sample_idx:    u8,
      vol:           u8,
      nr30:          u8,
    }

    impl Default for Wave {
      fn default() -> Self {
        Self {
          ram:           [0; RAM_LEN as usize],
          vol:           0,
          on:            false,
          dac_on:        true,
          l_timer:       LengthTimer::default(),
          freq:          0,
          period:        i32::from(PERIOD_MUL) * 2048,
          sample_buffer: 0,
          samples:       [0; SAMPLE_LEN as usize],
          sample_idx:    0,
          nr30:          0,
        }
      }
    }

    impl Wave {
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

      pub(crate) const fn read_nr30(&self) -> u8 { self.nr30 | 0x7F }

      pub(crate) const fn read_nr32(&self) -> u8 { 0x9F | (self.vol << 5) }

      pub(crate) fn read_nr34(&self) -> u8 { 0xBF | self.l_timer.read_on() }

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
        self.l_timer.write_len(val);
      }

      pub(crate) fn write_nr32(&mut self, val: u8) {
        self.vol = (val >> 5) & 3;
      }

      pub(crate) fn write_nr33(&mut self, val: u8) {
        self.freq = (self.freq & 0x700) | u16::from(val);
      }

      pub(crate) fn write_nr34(&mut self, val: u8) {
        self.freq = (u16::from(val) & 7) << 8 | (self.freq & 0xFF);

        self.l_timer.write_on(val, &mut self.on);

        // trigger
        if val & 0x80 != 0 {
          if self.dac_on {
            self.on = true;
          }

          self.l_timer.trigger(&mut self.on);

          self.period = i32::from(PERIOD_MUL * (2048 - self.freq));
          self.sample_idx = 0;
        }
      }

      pub(crate) fn out(&self) -> u8 {
        // wrapping_shr is necessary because (vol - 1) can be -1
        self
          .sample_buffer
          .wrapping_shr(u32::from(self.vol.wrapping_sub(1)))
      }

      pub(in crate::apu) fn reset(&mut self) {
        self.on = false;
        self.dac_on = true;
        self.l_timer.reset();
        self.freq = 0;
        self.vol = 0;
        self.sample_idx = 0;
        self.period = 0;
        self.sample_buffer = 0;
        self.nr30 = 0;
      }

      pub(in crate::apu) fn step_sample(&mut self, cycles: i32) {
        if !self.true_on() {
          return;
        }

        self.period -= cycles;

        if self.period < 0 {
          self.period += i32::from(PERIOD_MUL * (2048 - self.freq));
          self.sample_idx = (self.sample_idx + 1) % SAMPLE_LEN;
          self.sample_buffer = self.samples[self.sample_idx as usize];
        }
      }

      pub(in crate::apu) const fn true_on(&self) -> bool {
        self.on && self.dac_on
      }

      pub(in crate::apu) fn step_len(&mut self) {
        self.l_timer.step(&mut self.on);
      }

      pub(in crate::apu) fn set_period_half(&mut self, p_half: PHalf) {
        self.l_timer.set_phalf(p_half);
      }

      pub(in crate::apu) const fn on(&self) -> bool { self.on }
    }
  }
}
