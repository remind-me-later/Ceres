use crate::interrupts::{Interrupts, TIMER_INT};

const TAC_ENABLE: u8 = 4;

#[derive(Default)]
pub struct Timer {
    enabled: bool,
    internal_counter: u16,

    // registers
    counter: u8,
    modulo: u8,
    tac: u8,
    overflow: bool,
}

impl Timer {
    pub fn new() -> Self {
        Self::default()
    }

    fn counter_mask(&self) -> u16 {
        match self.tac & 0b11 {
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

    fn inc(&mut self) {
        let (counter, overflow) = self.counter.overflowing_add(1);
        self.counter = counter;
        self.overflow = overflow;
    }

    pub fn tick_t_cycle(&mut self, ints: &mut Interrupts) {
        if self.overflow {
            self.internal_counter = self.internal_counter.wrapping_add(1);
            self.counter = self.modulo;
            ints.request(TIMER_INT);
            self.overflow = false;
        } else if self.enabled && self.counter_bit() {
            self.internal_counter = self.internal_counter.wrapping_add(1);
            let new_bit = self.counter_bit();
            if !new_bit {
                self.inc();
            }
        } else {
            self.internal_counter = self.internal_counter.wrapping_add(1);
        }
    }

    pub fn read_div(&mut self, ints: &mut Interrupts) -> u8 {
        self.tick_t_cycle(ints);
        ((self.internal_counter >> 6) & 0xff) as u8
    }

    pub fn read_tima(&mut self, ints: &mut Interrupts) -> u8 {
        self.tick_t_cycle(ints);
        self.counter
    }

    pub fn read_tma(&mut self, ints: &mut Interrupts) -> u8 {
        self.tick_t_cycle(ints);
        self.modulo
    }

    pub fn read_tac(&mut self, ints: &mut Interrupts) -> u8 {
        self.tick_t_cycle(ints);
        0xf8 | self.tac
    }

    pub fn write_div(&mut self, ints: &mut Interrupts) {
        self.tick_t_cycle(ints);

        if self.counter_bit() {
            self.inc();
        }

        self.internal_counter = 0;
    }

    pub fn write_tima(&mut self, ints: &mut Interrupts, val: u8) {
        let overflow = self.overflow;
        self.tick_t_cycle(ints);

        if !overflow {
            self.overflow = false;
            self.counter = val;
        }
    }

    pub fn write_tma(&mut self, ints: &mut Interrupts, val: u8) {
        let overflow = self.overflow;

        self.tick_t_cycle(ints);
        self.modulo = val;

        if overflow {
            self.counter = val;
        }
    }

    pub fn write_tac(&mut self, ints: &mut Interrupts, val: u8) {
        self.tick_t_cycle(ints);

        let old_bit = self.enabled && self.counter_bit();
        self.tac = val & 7;
        self.enabled = val & TAC_ENABLE != 0;
        let new_bit = self.enabled && self.counter_bit();

        if old_bit && !new_bit {
            self.inc();
        }
    }
}
