use super::{Rgb24Color, SCREEN_PIXELS};

const BYTES_PER_PIXEL: usize = 4;
const BUFFER_SIZE: usize = SCREEN_PIXELS as usize * BYTES_PER_PIXEL;

#[derive(Clone)]
pub struct PixelData {
    data: [u8; BUFFER_SIZE],
}

impl Default for PixelData {
    fn default() -> Self {
        Self {
            data: [0xff; BUFFER_SIZE],
        }
    }
}

impl PixelData {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_pixel_color(&mut self, index: usize, color: Rgb24Color) {
        let index_of_first_byte_of_pixel = index * BYTES_PER_PIXEL;
        self.data[index_of_first_byte_of_pixel] = color.r;
        self.data[index_of_first_byte_of_pixel + 1] = color.g;
        self.data[index_of_first_byte_of_pixel + 2] = color.b;
    }

    #[must_use]
    pub fn rgba(&self) -> &[u8] {
        &self.data
    }
}
