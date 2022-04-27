use bitflags::bitflags;

bitflags!(
  struct Control: u8 {
    const SHIFT_CLOCK    = 1;
    const TRANSFER_START = 1 << 7;
  }
);

// TODO: everything, dummy implementation
pub struct Serial {
    data: u8,
    control: Control,
}

impl Serial {
    pub fn new() -> Self {
        Self {
            data: 0x00,
            control: Control::empty(),
        }
    }

    pub fn read_sb(&self) -> u8 {
        self.data
    }

    pub fn read_sc(&self) -> u8 {
        self.control.bits | 0x7e
    }

    pub fn write_sb(&mut self, val: u8) {
        self.data = val
    }

    pub fn write_sc(&mut self, val: u8) {
        self.control = Control::from_bits_truncate(val)
    }
}
