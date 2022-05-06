const CURRENT_SPEED: u8 = 0x80;
const REQ_SPEED_SWITCH: u8 = 0x01;

#[derive(Default, Clone, Copy)]
pub struct Key1 {
    val: u8,
}

impl Key1 {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_req(self) -> bool {
        self.val & REQ_SPEED_SWITCH != 0
    }

    pub fn ack(&mut self) {
        self.val &= !REQ_SPEED_SWITCH;
        self.val ^= CURRENT_SPEED;
    }

    pub fn read(self) -> u8 {
        0x7e | self.val
    }

    pub fn write(&mut self, val: u8) {
        self.val = val & 1;
    }
}
