use bitflags::bitflags;

bitflags! {
    pub struct Register: u8 {
        const CURRENT_SPEED        = 0b1000_0000;
        const PREPARE_SPEED_SWITCH = 1;
    }
}

impl Register {
    pub const fn speed_switch_requested(self) -> bool {
        self.contains(Self::PREPARE_SPEED_SWITCH)
    }

    pub fn acknowledge_speed_switch(&mut self) {
        self.remove(Self::PREPARE_SPEED_SWITCH);
        self.toggle(Self::CURRENT_SPEED);
    }

    pub const fn read(self) -> u8 {
        const KEY1_MASK: u8 = 0x7e;
        KEY1_MASK | self.bits()
    }

    pub fn write(&mut self, val: u8) {
        *self = Self::from_bits_truncate(val & 1);
    }
}
