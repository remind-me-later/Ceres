use super::Color;
use super::SCREEN_PIXELS;

const PIXEL_SIZE_IN_BYTES: usize = 4;

// rgba
#[derive(Clone)]
pub struct PixelData {
    data: [u8; SCREEN_PIXELS as usize * PIXEL_SIZE_IN_BYTES],
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
            data: [0xff; SCREEN_PIXELS as usize * PIXEL_SIZE_IN_BYTES],
        }
    }

    pub fn set_pixel_color(&mut self, index: usize, color: Color) {
        let index_of_first_byte_of_pixel = index * PIXEL_SIZE_IN_BYTES;
        self.data[index_of_first_byte_of_pixel] = color.red;
        self.data[index_of_first_byte_of_pixel + 1] = color.green;
        self.data[index_of_first_byte_of_pixel + 2] = color.blue;
    }

    #[must_use]
    pub const fn rgba(&self) -> &[u8] {
        &self.data
    }
}