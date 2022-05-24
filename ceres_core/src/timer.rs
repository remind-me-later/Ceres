use crate::{Gb, IF_TIMER_B};

const TAC_ENABLE: u8 = 4;

impl Gb {
    pub(crate) fn advance_cycle(&mut self) {
        // affeected by speed boost
        self.tick_dma();
        self.tick_timer();

        // not affected by speed boost
        let mut cycles = 4;

        if self.double_speed {
            cycles >>= 1;
        }

        self.tick_apu(cycles);
        self.tick_ppu(cycles);
    }

    fn counter_mask(&self) -> u16 {
        match self.tac & 3 {
            3 => 1 << 5,
            2 => 1 << 3,
            1 => 1 << 1,
            0 => 1 << 7,
            _ => unreachable!(),
        }
    }

    fn counter_bit(&self) -> bool {
        self.clk_wide & self.counter_mask() != 0
    }

    fn inc_timer(&mut self) {
        let (counter, overflow) = self.tima.overflowing_add(1);
        self.tima = counter;
        self.clk_overflow = overflow;
    }

    pub(crate) fn tick_timer(&mut self) {
        if self.clk_overflow {
            self.clk_wide = self.clk_wide.wrapping_add(1);
            self.tima = self.tma;
            self.ifr |= IF_TIMER_B;
            self.clk_overflow = false;
        } else if self.clk_on && self.counter_bit() {
            self.clk_wide = self.clk_wide.wrapping_add(1);
            let new_bit = self.counter_bit();
            if !new_bit {
                self.inc_timer();
            }
        } else {
            self.clk_wide = self.clk_wide.wrapping_add(1);
        }
    }

    pub(crate) fn write_div(&mut self) {
        if self.counter_bit() {
            self.inc_timer();
        }

        self.clk_wide = 0;
    }

    pub(crate) fn write_tima(&mut self, val: u8) {
        let overflow = self.clk_overflow;

        if !overflow {
            self.clk_overflow = false;
            self.tima = val;
        }
    }

    pub(crate) fn write_tma(&mut self, val: u8) {
        let overflow = self.clk_overflow;

        self.tma = val;

        if overflow {
            self.tima = val;
        }
    }

    pub(crate) fn write_tac(&mut self, val: u8) {
        let old_bit = self.clk_on && self.counter_bit();
        self.tac = val & 7;
        self.clk_on = val & TAC_ENABLE != 0;
        let new_bit = self.clk_on && self.counter_bit();

        if old_bit && !new_bit {
            self.inc_timer();
        }
    }
}
