//! The background and object pixel FIFOs.

#[derive(Clone, Copy, Default)]
pub(super) struct Item {
    pub pixel: u8,
    pub palette: u8,
    pub priority: u8,
    pub bg_priority: bool,
}

/// An 8-entry ring, like SameBoy's `GB_fifo_t`.
#[derive(Clone, Copy, Default)]
pub(super) struct Fifo {
    pub items: [Item; 8],
    pub read_end: u8,
    pub size: u8,
}

impl Fifo {
    pub(super) const fn clear(&mut self) {
        self.read_end = 0;
        self.size = 0;
    }

    pub(super) fn pop(&mut self) -> Item {
        let item = self.items[usize::from(self.read_end)];
        self.read_end = (self.read_end + 1) & 7;
        self.size -= 1;
        item
    }

    pub(super) fn push_bg_row(
        &mut self,
        mut lower: u8,
        mut upper: u8,
        palette: u8,
        bg_priority: bool,
        flip_x: bool,
    ) {
        self.size = 8;
        for i in 0..8 {
            let pixel = if flip_x {
                let p = (lower & 1) | ((upper & 1) << 1);
                lower >>= 1;
                upper >>= 1;
                p
            } else {
                let p = (lower >> 7) | ((upper >> 7) << 1);
                lower <<= 1;
                upper <<= 1;
                p
            };
            self.items[i] = Item {
                pixel,
                palette,
                priority: 0,
                bg_priority,
            };
        }
    }

    pub(super) fn overlay_object_row(
        &mut self,
        mut lower: u8,
        mut upper: u8,
        palette: u8,
        bg_priority: bool,
        priority: u8,
        flip_x: bool,
    ) {
        while self.size < 8 {
            self.items[usize::from((self.read_end + self.size) & 7)] = Item::default();
            self.size += 1;
        }
        let flip_xor: u8 = if flip_x { 0 } else { 7 };
        for i in (0..8_u8).rev() {
            let pixel = (lower >> 7) | ((upper >> 7) << 1);
            let target = &mut self.items[usize::from((self.read_end + (i ^ flip_xor)) & 7)];
            if pixel != 0 && (target.pixel == 0 || target.priority > priority) {
                *target = Item {
                    pixel,
                    palette,
                    priority,
                    bg_priority,
                };
            }
            lower <<= 1;
            upper <<= 1;
        }
    }
}

/// One pixel as it reaches the LCD, before palette lookup.
#[derive(Clone, Copy)]
pub(super) struct PixelOut {
    pub lx: u8,
    /// Background colour id (already 0 when the background is disabled).
    pub bg_pixel: u8,
    pub bg_palette: u8,
    /// Object pixel, if one is drawn over the background.
    pub obj: Option<(u8, u8)>,
}
