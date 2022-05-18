use crate::{Gb, IF_TIMER_B};

const TAC_ENABLE: u8 = 4;

impl Gb {
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
        (self.clk_wide & self.counter_mask()) != 0
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

    pub(crate) fn read_div(&mut self) -> u8 {
        self.tick_timer();
        ((self.clk_wide >> 6) & 0xff) as u8
    }

    pub(crate) fn read_tima(&mut self) -> u8 {
        self.tick_timer();
        self.tima
    }

    pub(crate) fn read_tma(&mut self) -> u8 {
        self.tick_timer();
        self.tma
    }

    pub(crate) fn read_tac(&mut self) -> u8 {
        self.tick_timer();
        0xf8 | self.tac
    }

    pub(crate) fn write_div(&mut self) {
        self.tick_timer();

        if self.counter_bit() {
            self.inc_timer();
        }

        self.clk_wide = 0;
    }

    pub(crate) fn write_tima(&mut self, val: u8) {
        let overflow = self.clk_overflow;
        self.tick_timer();

        if !overflow {
            self.clk_overflow = false;
            self.tima = val;
        }
    }

    pub(crate) fn write_tma(&mut self, val: u8) {
        let overflow = self.clk_overflow;

        self.tick_timer();
        self.tma = val;

        if overflow {
            self.tima = val;
        }
    }

    pub(crate) fn write_tac(&mut self, val: u8) {
        self.tick_timer();

        let old_bit = self.clk_on && self.counter_bit();
        self.tac = val & 7;
        self.clk_on = val & TAC_ENABLE != 0;
        let new_bit = self.clk_on && self.counter_bit();

        if old_bit && !new_bit {
            self.inc_timer();
        }
    }
}
