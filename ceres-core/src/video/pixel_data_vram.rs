use super::RgbColor;

pub const TILES_PER_WIDTH: usize = 16;
pub const TILES_PER_HEIGTH: usize = 24;
pub const VRAM_DISPLAY_WIDTH: usize = TILES_PER_WIDTH * 8;
pub const VRAM_DISPLAY_HEIGHT: usize = TILES_PER_HEIGTH * 8;
const PIXEL_SIZE_IN_BYTES: usize = 4;

#[derive(Clone)]
pub struct PixelDataVram {
    data: [u8; VRAM_DISPLAY_WIDTH * VRAM_DISPLAY_HEIGHT * PIXEL_SIZE_IN_BYTES],
}

impl Default for PixelDataVram {
    fn default() -> Self {
        Self::new()
    }
}

impl PixelDataVram {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            data: [0xff; VRAM_DISPLAY_HEIGHT * VRAM_DISPLAY_WIDTH * PIXEL_SIZE_IN_BYTES],
        }
    }

    pub fn set_pixel_color_ij(&mut self, i: usize, j: usize, color: RgbColor) {
        let index = i * VRAM_DISPLAY_WIDTH + j;
        let index_of_first_byte_of_pixel = index * PIXEL_SIZE_IN_BYTES;
        self.data[index_of_first_byte_of_pixel] = color.r;
        self.data[index_of_first_byte_of_pixel + 1] = color.g;
        self.data[index_of_first_byte_of_pixel + 2] = color.b;
    }

    #[must_use]
    pub const fn rgba(&self) -> &[u8] {
        &self.data
    }
}
