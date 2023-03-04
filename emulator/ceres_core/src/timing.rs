use crate::{Gb, IF_TIMER_B};

#[derive(Clone, Copy, Default)]
pub enum TIMAState {
  Reloading,
  Reloaded,
  #[default]
  Running,
}

impl Gb {
  pub(crate) fn advance_t_cycles(&mut self, mut cycles: i32) {
    // affected by speed boost
    self.run_timers(cycles);
    self.dma_cycles += cycles;

    // not affected by speed boost
    if self.double_speed {
      cycles >>= 1;
    }

    // TODO: is this order right?I
    self.ppu.run(cycles, &mut self.ifr, self.compat_mode);
    self.run_dma();

    self.apu.run(cycles);
    self.cart.run(cycles);
  }

  const fn sys_clk_tac_mux(&self) -> u16 {
    match self.tac & 3 {
      0 => 1 << 9,
      1 => 1 << 3,
      2 => 1 << 5,
      _ => 1 << 7,
    }
  }

  fn advance_tima_state(&mut self) {
    match self.tima_state {
      TIMAState::Reloading => {
        self.ifr |= IF_TIMER_B;
        self.tima_state = TIMAState::Reloaded;
      }
      TIMAState::Reloaded => {
        self.tima_state = TIMAState::Running;
      }
      TIMAState::Running => (),
    }
  }

  fn inc_tima(&mut self) {
    self.tima = self.tima.wrapping_add(1);

    if self.tima == 0 {
      self.tima = self.tma;
      self.tima_state = TIMAState::Reloading;
    }
  }

  // only modify div inside this function
  // TODO: this could be optimized
  fn set_system_clk(&mut self, val: u16) {
    let triggers = self.wide_div_counter & !val;
    let apu_bit = if self.double_speed { 0x2000 } else { 0x1000 };

    // increase TIMA on falling edge of TAC mux
    if self.tac_enable && (triggers & self.sys_clk_tac_mux() != 0) {
      self.inc_tima();
    }

    // advance APU on falling edge of APU_DIV bit
    if triggers & apu_bit != 0 {
      self.apu.step_seq();
    }

    self.wide_div_counter = val;
  }

  pub(crate) fn run_timers(&mut self, cycles: i32) {
    for _ in 0..cycles / 4 {
      self.advance_tima_state();
      self.set_system_clk(self.wide_div_counter.wrapping_add(4));
    }
  }

  pub(crate) fn write_div(&mut self) { self.set_system_clk(0); }

  pub(crate) fn write_tima(&mut self, val: u8) { self.tima = val; }

  pub(crate) fn write_tma(&mut self, val: u8) { self.tma = val; }

  pub(crate) fn write_tac(&mut self, val: u8) {
    self.tac = val & 7;
    self.tac_enable = val & 4 != 0;
  }
}
