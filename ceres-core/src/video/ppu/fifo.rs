mod fetcher;
mod pixel;

use pixel::Pixel;

pub struct BgFifo {
    fifo: [Pixel; 16],
    index: u8,
}

impl BgFifo {
    pub fn clear(&mut self) {
        self.index = 0;
    }

    pub fn is_empty(&self) -> bool {
        self.index == 0
    }

    pub fn push_pixel(&mut self, pixel: Pixel) {
        self.fifo[self.index as usize] = pixel;
        self.index += 1;
    }
}
