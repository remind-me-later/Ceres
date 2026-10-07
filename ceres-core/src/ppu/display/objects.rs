//! Objects: the mode 2 search for the objects on the line and the state of
//! the object fetch in mode 3.

use crate::{
    Model,
    ppu::{Ppu, oam_bug::NO_ROW},
};

/// The mode 2 object search.
#[derive(Clone)]
pub struct ObjectSearch {
    pub count: usize,
    pub found: usize,
    /// OAM indices of the objects on the line, sorted by X (descending).
    pub indices: [u8; 10],
    pub x: [u8; 10],
    pub y: [u8; 10],
    pub index: u8,
    /// OAM row the PPU is reading (DMG OAM bug); `NO_ROW` when none.
    pub accessed_oam_row: u8,
    /// The last Y and X the search read from the OAM.
    pub y_bus: u8,
    pub x_bus: u8,
}

impl Default for ObjectSearch {
    fn default() -> Self {
        Self {
            count: 0,
            found: 0,
            indices: [0; 10],
            x: [0; 10],
            y: [0; 10],
            index: 0,
            accessed_oam_row: NO_ROW,
            y_bus: 0,
            x_bus: 0,
        }
    }
}

/// The fetch of an object's tile in mode 3.
#[derive(Clone, Default)]
pub struct ObjectFetch {
    pub flags: u8,
    pub line_address: u16,
    pub data: [u8; 2],
    pub active: bool,
    pub aborted: bool,
    /// OBJ_SIZE as the object fetch sees it (it can land a dot before the
    /// object search sees it).
    pub size_16: bool,
}

impl Ppu {
    /// OAM as the PPU reads it (SameBoy's `oam_read`): blocked in STOP mode,
    /// and while a DMA runs it sees the byte pair the DMA is writing.
    pub(super) fn oam_read(&self, addr: u16) -> u8 {
        if self.d.bus.oam_ppu_blocked {
            return 0xFF;
        }
        let dest = self.d.bus.dma_dest;
        if dest <= 0xA0 && dest > 0 {
            if self.d.bus.hdma_in_progress {
                return self
                    .oam_read_row(((self.d.bus.hdma_src & !1) | (addr & 1)).to_le_bytes()[0]);
            }
            if dest != 0xA0 {
                return self.oam.read(u16::from(dest & !1) | (addr & 1));
            }
        }
        self.oam.read(addr)
    }

    /// SameBoy's `GB_read_oam`: the 160 bytes, then the unusable area.
    pub(super) fn oam_read_row(&self, addr: u8) -> u8 {
        if addr < 0xA0 {
            self.oam.read(u16::from(addr))
        } else {
            self.read_unusable(0xFE00 | u16::from(addr))
        }
    }

    pub(super) fn add_object_from_index(&mut self, index: u8) {
        let base = u16::from(index) * 4;
        // On CGB the object search runs ahead of the DMA's state: by two
        // T-cycles in single speed, by a whole M-cycle in double speed.
        let lead = if self.double_speed { 3 } else { 2 };
        let dest = if self.hw_cgb() && self.d.bus.chunk_left <= lead {
            self.d.bus.dma_dest_next
        } else {
            self.d.bus.dma_dest
        };
        let dma_active = dest != 0xA1;
        if dma_active && !self.d.bus.cpu_idle && !matches!(dest, 0xFF | 0) {
            // Once a DMA has written its first byte the object search reads
            // 0xFF until the transfer ends.
            self.d.objs.y_bus = 0xFF;
            self.d.objs.x_bus = 0xFF;
        } else {
            self.d.objs.y_bus = self.oam_read(base);
            self.d.objs.x_bus = self.oam_read(base + 1);
        }

        if self.d.objs.count == 10 {
            return;
        }

        // A halted DMA blocks the object search on everything before CGB-E
        // (pre-CGB units vary; like SameBoy this reads 0xFF there).
        if dma_active && self.d.bus.cpu_idle && !matches!(self.model, Model::CgbE | Model::Agb) {
            return;
        }

        if self.d.bus.oam_ppu_blocked {
            return;
        }

        let height_16 = self.lcdc & 0x04 != 0;
        let y = i32::from(self.d.objs.y_bus) - 16;
        let line = i32::from(self.d.current_line);
        if y <= line && y + if height_16 { 16 } else { 8 } > line {
            // Reverse-sorted insertion by X (stable for equal X).
            let n = self.d.objs.count;
            let mut j = 0;
            while j < n {
                if self.d.objs.x[j] <= self.d.objs.x_bus {
                    break;
                }
                j += 1;
            }
            let mut k = n;
            while k > j {
                self.d.objs.indices[k] = self.d.objs.indices[k - 1];
                self.d.objs.x[k] = self.d.objs.x[k - 1];
                self.d.objs.y[k] = self.d.objs.y[k - 1];
                k -= 1;
            }
            self.d.objs.indices[j] = index;
            self.d.objs.x[j] = self.d.objs.x_bus;
            self.d.objs.y[j] = self.d.objs.y_bus;
            self.d.objs.count += 1;
        }
    }

    pub(super) fn x_for_object_match(&self) -> u8 {
        let ret = self.d.position_in_line.wrapping_add(8);
        if ret > 240 { 0 } else { ret }
    }

    pub(super) fn object_line_address(&self, y: u8, tile: u8, flags: u8) -> u16 {
        let height_16 = self.d.obj_fetch.size_16;
        let mut tile_y = self.d.current_line.wrapping_sub(y) & if height_16 { 0xF } else { 7 };
        if flags & 0x40 != 0 {
            tile_y ^= if height_16 { 0xF } else { 7 };
        }
        let mut address =
            u16::from(if height_16 { tile & 0xFE } else { tile }) * 0x10 + u16::from(tile_y) * 2;
        if self.cgb_mode_on() && flags & 0x8 != 0 {
            address += 0x2000;
        }
        address
    }

    /// Object priority: OAM-index priority on CGB hardware unless OPRI
    /// selects X-coordinate priority; DMG hardware always uses X.
    #[inline]
    pub(super) const fn opri_index_priority(&self) -> bool {
        self.hw_cgb() && !self.opri
    }

    pub const fn set_obj_size_fetch(&mut self, big: bool) {
        self.d.obj_fetch.size_16 = big;
    }

    /// DMG: disabling objects while an object is being fetched aborts it.
    pub(in crate::ppu) fn abort_object_fetch_on_obj_disable(&mut self, val: u8) {
        if !self.hw_cgb() && self.lcdc & 0x02 != 0 && val & 0x02 == 0 && self.d.obj_fetch.active {
            self.d.cfl -= self.d.wait - 1;
            self.d.wait = 1;
            self.d.obj_fetch.aborted = true;
        }
    }
}
