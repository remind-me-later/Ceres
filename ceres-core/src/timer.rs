use crate::interrupts::{Interrupt, InterruptController};
use bitflags::bitflags;

#[derive(Clone, Copy)]
pub enum Register {
    Div,
    Tima,
    Tma,
    Tac,
}

bitflags!(
  struct Tac: u8 {
    const ENABLE = 0b100;
    const MASK_1 = 0b010;
    const MASK_0 = 0b001;
  }
);

impl Tac {
    const fn counter_mask(self) -> u16 {
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
    pub const fn new() -> Self {
        Self {
            enabled: false,
            overflow: false,
            internal_counter: 0x630,
            counter: 0,
            modulo: 0,
            tac: Tac::empty(),
        }
    }

    const fn counter_bit(&self) -> bool {
        (self.internal_counter & self.tac.counter_mask()) != 0
    }

    fn increment(&mut self) {
        let (counter, overflow) = self.counter.overflowing_add(1);
        self.counter = counter;
        self.overflow = overflow;
    }

    // TODO: breaks without it, it shouldn't :)
    #[allow(clippy::branches_sharing_code)]
    pub fn tick_t_cycle(&mut self, interrupt_controller: &mut InterruptController) {
        if self.overflow {
            self.internal_counter = self.internal_counter.wrapping_add(1);
            self.counter = self.modulo;
            interrupt_controller.request(Interrupt::TIMER);
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

    pub fn read(
        &mut self,
        interrupt_controller: &mut InterruptController,
        register: Register,
    ) -> u8 {
        const TAC_MASK: u8 = 0xf8;
        match register {
            Register::Div => {
                self.tick_t_cycle(interrupt_controller);
                ((self.internal_counter >> 6) & 0xff) as u8
            }
            Register::Tima => {
                self.tick_t_cycle(interrupt_controller);
                self.counter
            }
            Register::Tma => {
                self.tick_t_cycle(interrupt_controller);
                self.modulo
            }
            Register::Tac => {
                self.tick_t_cycle(interrupt_controller);
                TAC_MASK | self.tac.bits()
            }
        }
    }

    pub fn write(
        &mut self,
        interrupt_controller: &mut InterruptController,
        register: Register,
        val: u8,
    ) {
        match register {
            Register::Div => {
                self.tick_t_cycle(interrupt_controller);

                if self.counter_bit() {
                    self.increment();
                }

                self.internal_counter = 0;
            }
            Register::Tima => {
                let overflow = self.overflow;
                self.tick_t_cycle(interrupt_controller);
                if !overflow {
                    self.overflow = false;
                    self.counter = val;
                }
            }
            Register::Tma => {
                let overflow = self.overflow;
                self.tick_t_cycle(interrupt_controller);
                self.modulo = val;
                if overflow {
                    self.counter = val;
                }
            }
            Register::Tac => {
                self.tick_t_cycle(interrupt_controller);
                let old_bit = self.enabled && self.counter_bit();
                self.tac = Tac::from_bits_truncate(val);
                self.enabled = self.tac.contains(Tac::ENABLE);
                let new_bit = self.enabled && self.counter_bit();
                if old_bit && !new_bit {
                    self.increment();
                }
            }
        }
    }
}
