//! Pixel FIFO implementation for cycle-accurate PPU.
//!
//! Reference: SameBoy display.h/display.c FIFO implementation

/// SameBoy: #define GB_FIFO_LENGTH 8
#[allow(dead_code)]
const FIFO_LENGTH: usize = 8;

/// A single item in the FIFO.
///
/// Reference: SameBoy display.h GB_fifo_item_t
#[derive(Clone, Copy, Default)]
pub struct FifoItem {
    /// Color index (0-3).
    /// SameBoy: pixel field
    pub color: u8,
    /// Palette index (BG: 0-7 CGB, 0 DMG; OBJ: 0-7 CGB, 0-1 DMG).
    /// SameBoy: palette field
    pub palette: u8,
    /// Sprite priority (OAM index for CGB, 0 for DMG).
    /// SameBoy: priority field
    pub priority: u8,
    /// Background priority flag (BG-to-OAM priority in CGB mode).
    /// SameBoy: bg_priority field
    pub bg_priority: bool,
}

/// Fixed-size FIFO with 8-pixel capacity.
///
/// Reference: SameBoy display.h GB_fifo_t
///
/// The Game Boy PPU uses two FIFOs: one for background/window pixels and one for
/// sprite (OAM) pixels.
#[derive(Clone, Copy, Default)]
pub struct PixelFifo {
    fifo: [FifoItem; FIFO_LENGTH],
    read_end: u8,
    size: u8,
}

impl PixelFifo {
    /// Returns the number of pixels currently in the FIFO.
    /// SameBoy: fifo_size()
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
    /// SameBoy: fifo_clear()
    #[inline]
    pub fn clear(&mut self) {
        self.read_end = 0;
        self.size = 0;
    }

    /// Pushes a row of 8 background/window pixels to the FIFO.
    ///
    /// Reference: SameBoy display.c fifo_push_bg_row()
    ///
    /// # Arguments
    /// * `data_low` - Low byte of tile data (bit 0 of each pixel color)
    /// * `data_high` - High byte of tile data (bit 1 of each pixel color)
    /// * `palette` - Palette index (0 for DMG, 0-7 for CGB)
    /// * `bg_priority` - CGB BG-to-OAM priority flag
    /// * `flip_x` - Whether to flip pixels horizontally
    ///
    /// # Panics
    /// Panics in debug mode if FIFO has more than 8 pixels.
    pub fn push_bg_row(
        &mut self,
        data_low: u8,
        data_high: u8,
        palette: u8,
        bg_priority: bool,
        flip_x: bool,
    ) {
        assert!(
            self.is_empty(),
            "BG FIFO must be empty before pushing new row"
        );

        self.size = 8;

        if flip_x {
            for i in 0..8 {
                let color = ((data_low >> i) & 1) | (((data_high >> i) & 1) << 1);
                self.fifo[i] = FifoItem {
                    color,
                    palette,
                    priority: 0,
                    bg_priority,
                };
            }
        } else {
            for i in 0..8 {
                let color = ((data_low >> (7 - i)) & 1) | (((data_high >> (7 - i)) & 1) << 1);
                self.fifo[i] = FifoItem {
                    color,
                    palette,
                    priority: 0,
                    bg_priority,
                };
            }
        }
    }

    /// Pops a single pixel from the FIFO.
    ///
    /// Reference: SameBoy display.c fifo_pop()
    ///
    /// Returns `None` if the FIFO is empty.
    #[inline]
    pub fn pop(&mut self) -> FifoItem {
        assert!(self.size > 0, "Cannot pop from empty FIFO");
        assert!(self.size <= FIFO_LENGTH as u8, "FIFO size overflow");
        let pixel = self.fifo[self.read_end as usize];
        self.read_end = (self.read_end + 1) & (FIFO_LENGTH as u8 - 1);
        self.size -= 1;
        pixel
    }

    /// Overlays sprite pixels onto the OAM FIFO.
    ///
    /// Reference: SameBoy display.c fifo_overlay_object_row()
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
    pub fn overlay_object_row(
        &mut self,
        mut data_low: u8,
        mut data_high: u8,
        palette: u8,
        bg_priority: bool,
        priority: u8,
        flip_x: bool,
    ) {
        // SameBoy: Ensure FIFO has GB_FIFO_LENGTH slots (pad with transparent pixels if needed)
        while self.size < FIFO_LENGTH as u8 {
            let idx = ((self.read_end + self.size) & (FIFO_LENGTH as u8 - 1)) as usize;
            self.fifo[idx] = FifoItem::default();
            self.size += 1;
        }

        let flip_xor = if flip_x { 0 } else { 0x7 };

        for i in (0..8_u8).rev() {
            // Extract color from tile data
            let color = (data_low >> 7) | ((data_high >> 7) << 1);
            let target = &mut self.fifo
                [((self.read_end + (i ^ flip_xor)) & (FIFO_LENGTH as u8 - 1)) as usize];

            // Sprite pixels only replace if:
            // - New pixel is non-transparent (color != 0)
            // - Target pixel is transparent OR new sprite has higher priority
            if color != 0 && (target.color == 0 || priority < target.priority) {
                target.color = color;
                target.palette = palette;
                target.bg_priority = bg_priority;
                target.priority = priority;
            }

            data_low <<= 1;
            data_high <<= 1;
        }
    }
}
