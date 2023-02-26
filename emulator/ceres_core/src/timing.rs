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

  const fn sys_clk_tac_mux(&self) -> bool {
    let mask = {
      match self.tac & 3 {
        0 => 1 << 9,
        1 => 1 << 3,
        2 => 1 << 5,
        _ => 1 << 7,
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
      let old_apu_div =
        self.system_clk & if self.double_speed { 0x2000 } else { 0x1000 } != 0;
      let old_bit = self.tac_enable && self.sys_clk_tac_mux();

      self.system_clk = self.system_clk.wrapping_add(1);

      let new_bit = self.tac_enable && self.sys_clk_tac_mux();
      let new_apu_div =
        self.system_clk & if self.double_speed { 0x2000 } else { 0x1000 } != 0;

      // increase TIMA on falling edge of TAC mux
      if old_bit && !new_bit {
        self.inc_tima();
      }

      // advance APU on falling edge of APU_DIV bit
      if old_apu_div && !new_apu_div {
        self.apu_step_seq();
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

  pub(crate) fn write_tima(&mut self, val: u8) { self.tima = val; }

  pub(crate) fn write_tma(&mut self, val: u8) { self.tma = val; }

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
