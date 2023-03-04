use crate::{PX_HEIGHT, PX_WIDTH};

const PX_TOTAL: u16 = PX_WIDTH as u16 * PX_HEIGHT as u16;
const RGBA_BUF_SIZE: usize = PX_TOTAL as usize * 4;

#[derive(Clone)]
pub(super) struct RgbaBuf {
  data: [u8; RGBA_BUF_SIZE],
}

impl Default for RgbaBuf {
  fn default() -> Self { Self { data: [0xFF; RGBA_BUF_SIZE] } }
}

impl RgbaBuf {
  pub(super) fn set_px(&mut self, i: usize, rgb: (u8, u8, u8)) {
    let base = i * 4;
    self.data[base] = rgb.0;
    self.data[base + 1] = rgb.1;
    self.data[base + 2] = rgb.2;
  }

  // fn clear(&mut self) {
  //   self.data = [0xFF; RGB_BUF_SIZE];
  // }

  pub(crate) const fn pixel_data(&self) -> &[u8] { &self.data }
}
