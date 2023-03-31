use crate::{PX_HEIGHT, PX_WIDTH};

const PX_TOTAL: u16 = PX_WIDTH as u16 * PX_HEIGHT as u16;
const RGB_BUF_SIZE: u32 = PX_TOTAL as u32 * BPP;
const BPP: u32 = 3;

#[derive(Clone)]
pub(super) struct RgbaBuf {
    data: [u8; RGB_BUF_SIZE as usize],
}

impl Default for RgbaBuf {
    fn default() -> Self {
        Self {
            data: [0xff; RGB_BUF_SIZE as usize],
        }
    }
}

impl RgbaBuf {
    #[inline]
    pub(super) fn set_px(&mut self, i: u32, rgb: (u8, u8, u8)) {
        let base = i * BPP;
        self.data[base as usize] = rgb.0;
        self.data[base as usize + 1] = rgb.1;
        self.data[base as usize + 2] = rgb.2;
    }

    #[must_use]
    #[inline]
    pub(crate) const fn pixel_data(&self) -> &[u8] {
        &self.data
    }
}
