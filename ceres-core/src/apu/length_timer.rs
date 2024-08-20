use super::PHalf;

// LEN_MASK is the maximum length of the timer, 0x3F for all channels except wave, which is 0xFF
#[derive(Default)]
pub(super) struct LengthTimer<const LEN_MASK: u8> {
    on: bool,
    len: u8,
    p_half: PHalf,
    carry: bool,
}

impl<const LEN_MASK: u8> LengthTimer<LEN_MASK> {
    pub(super) fn read_on(&self) -> u8 {
        u8::from(self.on) << 6
    }

    pub(super) fn write_on(&mut self, val: u8, on: &mut bool) {
        let was_off = !self.on;
        self.on = val & 0x40 != 0;

        if was_off && matches!(self.p_half, PHalf::First) {
            self.step(on);
        }
    }

    pub(super) fn write_len(&mut self, val: u8) {
        self.len = val & LEN_MASK;
        self.carry = false;
    }

    pub(super) fn trigger(&mut self, on: &mut bool) {
        if self.carry {
            self.len = 0;
            self.carry = false;
            if matches!(self.p_half, PHalf::First) {
                self.step(on);
            }
        }
    }

    pub(super) fn step(&mut self, on: &mut bool) {
        if self.on {
            if self.len == LEN_MASK {
                *on = false;
                self.carry = true;
            } else {
                self.len += 1;
            }
        }
    }

    pub(super) fn set_phalf(&mut self, p_half: PHalf) {
        self.p_half = p_half;
    }
}
