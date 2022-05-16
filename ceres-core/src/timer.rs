use crate::{interrupts::TIMER_INT, Gb};

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
        (self.internal_counter & self.counter_mask()) != 0
    }

    fn inc_timer(&mut self) {
        let (counter, overflow) = self.counter.overflowing_add(1);
        self.counter = counter;
        self.overflow = overflow;
    }

    pub(crate) fn tick_timer(&mut self) {
        if self.overflow {
            self.internal_counter = self.internal_counter.wrapping_add(1);
            self.counter = self.modulo;
            self.req_interrupt(TIMER_INT);
            self.overflow = false;
        } else if self.clock_on && self.counter_bit() {
            self.internal_counter = self.internal_counter.wrapping_add(1);
            let new_bit = self.counter_bit();
            if !new_bit {
                self.inc_timer();
            }
        } else {
            self.internal_counter = self.internal_counter.wrapping_add(1);
        }
    }

    pub(crate) fn read_div(&mut self) -> u8 {
        self.tick_timer();
        ((self.internal_counter >> 6) & 0xff) as u8
    }

    pub(crate) fn read_tima(&mut self) -> u8 {
        self.tick_timer();
        self.counter
    }

    pub(crate) fn read_tma(&mut self) -> u8 {
        self.tick_timer();
        self.modulo
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

        self.internal_counter = 0;
    }

    pub(crate) fn write_tima(&mut self, val: u8) {
        let overflow = self.overflow;
        self.tick_timer();

        if !overflow {
            self.overflow = false;
            self.counter = val;
        }
    }

    pub(crate) fn write_tma(&mut self, val: u8) {
        let overflow = self.overflow;

        self.tick_timer();
        self.modulo = val;

        if overflow {
            self.counter = val;
        }
    }

    pub(crate) fn write_tac(&mut self, val: u8) {
        self.tick_timer();

        let old_bit = self.clock_on && self.counter_bit();
        self.tac = val & 7;
        self.clock_on = val & TAC_ENABLE != 0;
        let new_bit = self.clock_on && self.counter_bit();

        if old_bit && !new_bit {
            self.inc_timer();
        }
    }
}
