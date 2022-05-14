// 11 bit frequency data
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub struct Freq<const MUL: u16> {
    pub val: u16,
}

impl<const MUL: u16> Freq<MUL> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        self.val = 0;
    }

    pub fn set_lo(&mut self, val: u8) {
        self.val = (self.val & 0x700) | (val as u16);
    }

    pub fn set_hi(&mut self, val: u8) {
        self.val = (val as u16 & 7) << 8 | (self.val & 0xff);
    }

    pub fn period(self) -> u16 {
        MUL * (2048 - self.val)
    }
}
