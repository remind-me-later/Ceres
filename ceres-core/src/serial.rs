use bitflags::bitflags;

bitflags!(
  struct Control: u8 {
    const SHIFT_CLOCK    = 1;
    const TRANSFER_START = 1 << 7;
  }
);

#[derive(Clone, Copy)]
pub enum Register {
    SB,
    SC,
}

pub struct Serial {
    data: u8,
    control: Control,
}

impl Serial {
    pub const fn new() -> Self {
        Self {
            data: 0x00,
            control: Control::empty(),
        }
    }

    pub const fn read(&self, register: Register) -> u8 {
        const SC_MASK: u8 = 0x7e;

        match register {
            Register::SB => self.data,
            Register::SC => self.control.bits | SC_MASK,
        }
    }

    pub fn write(&mut self, register: Register, val: u8) {
        match register {
            Register::SB => self.data = val,
            Register::SC => self.control = Control::from_bits_truncate(val),
        }
    }
}
