use crate::interrupts::{Interrupt, Interrupts};
use bitflags::bitflags;

bitflags!(
  struct Tac: u8 {
    const ENABLE = 0b100;
    const MASK_1 = 0b010;
    const MASK_0 = 0b001;
  }
);

impl Tac {
    fn counter_mask(self) -> u16 {
        match self.bits() & 0b11 {
            0b11 => (1 << 5),
            0b10 => (1 << 3),
            0b01 => (1 << 1),
            _ => (1 << 7),
        }
    }
}

// TODO: Document obscure behaviour

pub struct Timer {
    enabled: bool,
    internal_counter: u16,

    // registers
    counter: u8,
    modulo: u8,
    tac: Tac,
    overflow: bool,
}

impl Timer {
    pub fn new() -> Self {
        Self {
            enabled: false,
            overflow: false,
            internal_counter: 0x630,
            counter: 0,
            modulo: 0,
            tac: Tac::empty(),
        }
    }

    fn counter_bit(&self) -> bool {
        (self.internal_counter & self.tac.counter_mask()) != 0
    }

    fn increment(&mut self) {
        let (counter, overflow) = self.counter.overflowing_add(1);
        self.counter = counter;
        self.overflow = overflow;
    }

    pub fn tick_t_cycle(&mut self, ints: &mut Interrupts) {
        if self.overflow {
            self.internal_counter = self.internal_counter.wrapping_add(1);
            self.counter = self.modulo;
            ints.request(Interrupt::TIMER);
            self.overflow = false;
        } else if self.enabled && self.counter_bit() {
            self.internal_counter = self.internal_counter.wrapping_add(1);
            let new_bit = self.counter_bit();
            if !new_bit {
                self.increment();
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
        0xf8 | self.tac.bits()
    }

    pub fn write_div(&mut self, ints: &mut Interrupts) {
        self.tick_t_cycle(ints);

        if self.counter_bit() {
            self.increment();
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
        self.tac = Tac::from_bits_truncate(val);
        self.enabled = self.tac.contains(Tac::ENABLE);
        let new_bit = self.enabled && self.counter_bit();
        if old_bit && !new_bit {
            self.increment();
        }
    }
}
