use crate::{Gb, IF_TIMER_B};

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
    self.run_ppu(cycles);
    self.run_dma();
    self.run_apu(cycles);
    self.cart.run_cycles(cycles);
  }

  fn sys_clk_tac_mux(&self) -> bool {
    let mask = {
      match self.tac & 3 {
        0 => 1 << 9,
        3 => 1 << 7,
        2 => 1 << 5,
        1 => 1 << 3,
        _ => unreachable!(),
      }
    };

    self.system_clk & mask != 0
  }

  fn inc_tima(&mut self) {
    let (tima, tima_overflow) = self.tima.overflowing_add(1);
    self.tima = tima;

    if tima_overflow {
      // Fixme: a full m-cycle should pass between overflow and
      // tima reset
      self.reset_tima_overflow();
    }
  }

  pub(crate) fn run_timers(&mut self, cycles: i32) {
    for _ in 0..cycles {
      // Falling edge detector
      let old_bit = self.tac_enable && self.sys_clk_tac_mux();
      self.system_clk = self.system_clk.wrapping_add(1);
      let new_bit = self.tac_enable && self.sys_clk_tac_mux();

      // increase TIMA on falling edge of TAC mux
      if old_bit && !new_bit {
        self.inc_tima();
      }
    }
  }

  fn reset_tima_overflow(&mut self) {
    self.tima = self.tma;
    self.ifr |= IF_TIMER_B;
  }

  pub(crate) fn write_div(&mut self) {
    if self.sys_clk_tac_mux() {
      self.inc_tima();
    }

    self.system_clk = 0;
  }

  pub(crate) fn write_tima(&mut self, val: u8) {
    self.tima = val;
  }

  pub(crate) fn write_tma(&mut self, val: u8) {
    self.tma = val;
  }

  pub(crate) fn write_tac(&mut self, val: u8) {
    let old_bit = self.tac_enable && self.sys_clk_tac_mux();
    self.tac = val & 7;
    self.tac_enable = val & 4 != 0;
    let new_bit = self.tac_enable && self.sys_clk_tac_mux();

    if old_bit && !new_bit {
      self.inc_tima();
    }
  }
}
