use bitflags::bitflags;

bitflags! {
    pub struct Key1: u8 {
        const CURRENT_SPEED    = 0x80;
        const REQ_SPEED_SWITCH = 0x01;
    }
}

impl Key1 {
    pub fn is_req(self) -> bool {
        self.contains(Self::REQ_SPEED_SWITCH)
    }

    pub fn ack(&mut self) {
        self.remove(Self::REQ_SPEED_SWITCH);
        self.toggle(Self::CURRENT_SPEED);
    }

    pub fn read(self) -> u8 {
        0x7e | self.bits()
    }

    pub fn write(&mut self, val: u8) {
        *self = Self::from_bits_truncate(val & 1);
    }
}
