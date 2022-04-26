use super::RgbColor;
use super::SCREEN_PIXELS;

const BYTES_PER_PIXEL: usize = 4;

#[derive(Clone)]
pub struct PixelData {
    data: [u8; SCREEN_PIXELS as usize * BYTES_PER_PIXEL],
}

impl Default for PixelData {
    fn default() -> Self {
        Self::new()
    }
}

impl PixelData {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            data: [0xff; SCREEN_PIXELS as usize * BYTES_PER_PIXEL],
        }
    }

    pub fn set_pixel_color(&mut self, index: usize, color: RgbColor) {
        let index_of_first_byte_of_pixel = index * BYTES_PER_PIXEL;
        self.data[index_of_first_byte_of_pixel] = color.r;
        self.data[index_of_first_byte_of_pixel + 1] = color.g;
        self.data[index_of_first_byte_of_pixel + 2] = color.b;
    }

    #[must_use]
    pub const fn rgba(&self) -> &[u8] {
        &self.data
    }
}
