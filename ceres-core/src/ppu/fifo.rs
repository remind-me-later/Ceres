//! Pixel FIFO implementation for cycle-accurate PPU.

/// A single pixel in the FIFO.
#[derive(Clone, Copy, Default)]
pub struct FifoPixel {
    /// Color index (0-3).
    pub color: u8,
    /// Palette index (BG: 0-7 CGB, 0 DMG; OBJ: 0-7 CGB, 0-1 DMG).
    pub palette: u8,
    /// Sprite priority (OAM index for CGB, 0 for DMG).
    pub priority: u8,
    /// Background priority flag (BG-to-OAM priority in CGB mode).
    pub bg_priority: bool,
}

/// Fixed-size FIFO with 8-pixel capacity.
///
/// The Game Boy PPU uses two FIFOs: one for background/window pixels and one for
/// sprite (OAM) pixels. Each FIFO holds 8 pixels, but sprites can partially
/// overlay onto the "next" 8 pixels by wrapping around the buffer.
#[derive(Clone, Copy, Default)]
pub struct PixelFifo {
    pixels: [FifoPixel; 8],
    read_pos: u8,
    size: u8,
}

impl PixelFifo {
    /// Returns the number of pixels currently in the FIFO.
    #[inline]
    #[must_use]
    pub const fn size(&self) -> u8 {
        self.size
    }

    /// Returns true if the FIFO is empty.
    #[inline]
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.size == 0
    }

    /// Clears the FIFO, removing all pixels.
    #[inline]
    pub fn clear(&mut self) {
        self.read_pos = 0;
        self.size = 0;
    }

    /// Pushes a row of 8 background/window pixels to the FIFO.
    ///
    /// # Arguments
    /// * `data_low` - Low byte of tile data (bit 0 of each pixel color)
    /// * `data_high` - High byte of tile data (bit 1 of each pixel color)
    /// * `palette` - Palette index (0 for DMG, 0-7 for CGB)
    /// * `bg_priority` - CGB BG-to-OAM priority flag
    /// * `flip_x` - Whether to flip pixels horizontally
    ///
    /// # Panics
    /// Panics in debug mode if FIFO is not empty (bg push only happens when empty).
    pub fn push_bg_row(
        &mut self,
        mut data_low: u8,
        mut data_high: u8,
        palette: u8,
        bg_priority: bool,
        flip_x: bool,
    ) {
        debug_assert!(self.size == 0, "BG FIFO must be empty before push");

        self.size = 8;
        // SameBoy resets read_pos on empty, but since we use circular buffer logic,
        // we write relative to current read_pos (which should be aligned if empty?).
        // Actually, SameBoy says `fifo->read_end = 0` in `fifo_clear` but `fifo_push_bg_row` assumes empty.
        // In SameBoy: `fifo->read_end` is the READ pointer.
        // It writes to `fifo->fifo[i]` directly (0..7).
        // This implies it resets alignment?
        // SameBoy's `fifo_clear` resets read_end. `push_bg_row` asserts size 0.
        // If size is 0, read_pos doesn't matter unless we reset it.
        // Let's reset it to 0 to match SameBoy's implicit behavior.
        self.read_pos = 0;

        if !flip_x {
            for i in 0..8 {
                let color = ((data_low >> 7) & 1) | (((data_high >> 7) & 1) << 1);
                self.pixels[i] = FifoPixel {
                    color,
                    palette,
                    priority: 0,
                    bg_priority,
                };
                data_low <<= 1;
                data_high <<= 1;
            }
        } else {
            for i in 0..8 {
                let color = (data_low & 1) | ((data_high & 1) << 1);
                self.pixels[i] = FifoPixel {
                    color,
                    palette,
                    priority: 0,
                    bg_priority,
                };
                data_low >>= 1;
                data_high >>= 1;
            }
        }
    }

    /// Pops a single pixel from the FIFO.
    ///
    /// Returns `None` if the FIFO is empty.
    #[inline]
    pub fn pop(&mut self) -> Option<FifoPixel> {
        if self.size == 0 {
            return None;
        }
        let pixel = self.pixels[self.read_pos as usize];
        self.read_pos = (self.read_pos + 1) & 7;
        self.size -= 1;
        Some(pixel)
    }

    /// Overlays sprite pixels onto the OAM FIFO.
    ///
    /// Sprite pixels only replace existing pixels if:
    /// - The sprite pixel is non-transparent (color != 0)
    /// - The existing pixel is transparent OR the new sprite has higher priority
    ///
    /// # Arguments
    /// * `data_low` - Low byte of sprite tile data
    /// * `data_high` - High byte of sprite tile data
    /// * `palette` - Sprite palette index
    /// * `bg_priority` - Sprite's BG priority flag (from OAM attributes)
    /// * `priority` - Sprite OAM index (for CGB priority)
    /// * `flip_x` - Whether to flip sprite horizontally
    pub fn overlay_sprite_row(
        &mut self,
        mut data_low: u8,
        mut data_high: u8,
        palette: u8,
        bg_priority: bool,
        priority: u8,
        flip_x: bool,
    ) {
        // Ensure FIFO has space for overlay (SameBoy logic: size < 8 means we pad with transparent)
        while self.size < 8 {
            let idx = ((self.read_pos + self.size) & 7) as usize;
            self.pixels[idx] = FifoPixel::default();
            self.size += 1;
        }

        let flip_xor = if flip_x { 0 } else { 7 };

        // Iterate 8 pixels of the sprite (high to low)
        for i in (0..8).rev() {
            let pixel_color = ((data_low >> 7) & 1) | (((data_high >> 7) & 1) << 1);

            // Calculate target index in circular buffer
            let target_idx = (self.read_pos as usize + (i ^ flip_xor)) & 7;
            let target = &mut self.pixels[target_idx];

            if pixel_color != 0 && (target.color == 0 || priority < target.priority) {
                target.color = pixel_color;
                target.palette = palette;
                target.bg_priority = bg_priority;
                target.priority = priority;
            }

            data_low <<= 1;
            data_high <<= 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fifo_push_pop() {
        let mut fifo = PixelFifo::default();
        assert!(fifo.is_empty());
        assert_eq!(fifo.size(), 0);

        // Push a row with alternating colors
        fifo.push_bg_row(0b1010_1010, 0b0000_0000, 0, false, false);
        assert_eq!(fifo.size(), 8);

        // Pop and check colors
        for i in 0..8 {
            let pixel = fifo.pop().expect("should have pixel");
            let expected_color = if i % 2 == 0 { 1 } else { 0 };
            assert_eq!(pixel.color, expected_color, "pixel {i} color mismatch");
        }

        assert!(fifo.is_empty());
        assert!(fifo.pop().is_none());
    }

    #[test]
    fn test_fifo_flip_x() {
        let mut fifo_normal = PixelFifo::default();
        let mut fifo_flipped = PixelFifo::default();

        // Pattern: 0b1100_0011 = colors [1,1,0,0,0,0,1,1] when not flipped
        fifo_normal.push_bg_row(0b1100_0011, 0b0000_0000, 0, false, false);
        fifo_flipped.push_bg_row(0b1100_0011, 0b0000_0000, 0, false, true);

        // Collect all colors
        let mut colors_normal = Vec::new();
        let mut colors_flipped = Vec::new();
        for _ in 0..8 {
            colors_normal.push(fifo_normal.pop().unwrap().color);
            colors_flipped.push(fifo_flipped.pop().unwrap().color);
        }

        // Flipped should be reversed
        let mut expected_flipped = colors_normal.clone();
        expected_flipped.reverse();
        assert_eq!(colors_flipped, expected_flipped);
    }

    #[test]
    fn test_fifo_clear() {
        let mut fifo = PixelFifo::default();
        fifo.push_bg_row(0xFF, 0xFF, 0, false, false);
        assert_eq!(fifo.size(), 8);

        fifo.clear();
        assert!(fifo.is_empty());
    }
}
