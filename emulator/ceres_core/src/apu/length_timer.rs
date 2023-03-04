use super::PHalf;

#[derive(Default)]
pub(super) struct LengthTimer<const MAX_LEN: u16> {
  on:     bool,
  len:    u16,
  p_half: PHalf,
}

impl<const MAX_LEN: u16> LengthTimer<MAX_LEN> {
  pub(super) fn read_on(&self) -> u8 { u8::from(self.on) << 6 }

  pub(super) fn write_on(&mut self, val: u8, on: &mut bool) {
    let was_off = !self.on;
    self.on = val & 0x40 != 0;

    if was_off && self.p_half == PHalf::First {
      self.step(on);
    }
  }

  pub(super) fn write_len(&mut self, val: u8) {
    self.len = MAX_LEN - (u16::from(val) & (MAX_LEN - 1));
  }

  pub(super) fn trigger(&mut self, on: &mut bool) {
    if self.len == 0 {
      self.len = MAX_LEN;
      if self.p_half == PHalf::First {
        self.step(on);
      }
    }
  }

  pub(super) fn step(&mut self, on: &mut bool) {
    // WARN: looks wrong but sameboy does it this way
    // https://github.com/LIJI32/SameBoy/blob/master/Core/apu.c line 528,
    // also "fixing" it breaks blargg cgb sound test 3
    if self.on && self.len > 0 {
      self.len -= 1;
      if self.len == 0 {
        *on = false;
      }
    }
  }

  pub(super) fn set_phalf(&mut self, p_half: PHalf) { self.p_half = p_half; }
}
