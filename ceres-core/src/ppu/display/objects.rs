//! Objects: the mode 2 search for the objects on the line and the state of
//! the object fetch in mode 3.

use {
    super::State,
    crate::{
        Model,
        ppu::{LCDC_OBJ_EN_B, LCDC_OBJ_SIZE_B, LCDC_ON_B, Ppu, oam_bug::NO_ROW},
    },
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
    /// A CGB-C OBJ_SIZE write during this line's search (see
    /// `cgb_obj_size_write`).
    pub size_change: Option<SizeChange>,
}

/// OBJ_SIZE changed while the CGB-C was searching `line`: an entry samples
/// the size at line cycles `2 * index` and `2 * index + 1` on gambatte's
/// clock and is large if either sample is; the samples after `lc` see `new`.
#[derive(Clone, Copy)]
pub struct SizeChange {
    pub line: u8,
    pub lc: i64,
    pub new: bool,
}

impl SizeChange {
    fn large(self, index: u8) -> bool {
        let first = 2 * i64::from(index);
        let sample = |p: i64| if p > self.lc { self.new } else { !self.new };
        sample(first) || sample(first + 1)
    }
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
            size_change: None,
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
        if (1..=0xA0).contains(&dest) {
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

        let height_16 = match self.d.objs.size_change {
            Some(c) if c.line == self.d.current_line => c.large(index),
            _ => self.lcdc & LCDC_OBJ_SIZE_B != 0,
        };
        if self.object_on_line(self.d.objs.y_bus, height_16) {
            self.insert_object(index, self.d.objs.x_bus, self.d.objs.y_bus);
        }
    }

    fn object_on_line(&self, y: u8, height_16: bool) -> bool {
        let y = i32::from(y) - 16;
        let line = i32::from(self.d.current_line);
        y <= line && y + if height_16 { 16 } else { 8 } > line
    }

    /// Inserts an object in the list, sorted by X (descending), then by OAM
    /// index (descending).
    fn insert_object(&mut self, index: u8, obj_x: u8, obj_y: u8) {
        let objs = &mut self.d.objs;
        let count = objs.count;
        let at = (0..count)
            .find(|&i| objs.x[i] < obj_x || (objs.x[i] == obj_x && objs.indices[i] < index))
            .unwrap_or(count);
        objs.indices.copy_within(at..count, at + 1);
        objs.x.copy_within(at..count, at + 1);
        objs.y.copy_within(at..count, at + 1);
        objs.indices[at] = index;
        objs.x[at] = obj_x;
        objs.y[at] = obj_y;
        objs.count += 1;
    }

    fn remove_object(&mut self, index: u8) {
        let n = self.d.objs.count;
        let Some(j) = self.d.objs.indices[..n].iter().position(|&i| i == index) else {
            return;
        };
        let objs = &mut self.d.objs;
        objs.indices.copy_within(j + 1..n, j);
        objs.x.copy_within(j + 1..n, j);
        objs.y.copy_within(j + 1..n, j);
        objs.count -= 1;
    }

    /// A CGB-C write that changes OBJ_SIZE, `units_early` before the point a
    /// read in the same cycle would see. The search samples the size later
    /// than this PPU looks at each entry (gambatte's `OamReader`), so the
    /// entries already searched are looked at again.
    pub fn cgb_obj_size_write(&mut self, val: u8, units_early: i64) {
        let new = val & LCDC_OBJ_SIZE_B != 0;
        if !self.gambatte_stat()
            || self.lcdc & LCDC_ON_B == 0
            || (self.lcdc & LCDC_OBJ_SIZE_B != 0) == new
        {
            return;
        }
        let searched = match self.d.state() {
            State::OamScanNext => self.d.objs.index,
            State::OamScanObject => self.d.objs.index + 1,
            State::Mode3PalettesLock | State::Mode3Start => 40,
            State::LineOamWriteLock | State::LineLy | State::OamScanStart => 0,
            _ => return,
        };
        let line = self.d.current_line;
        let Some(lc) = self.gstat_size_change_cycle(line, units_early) else {
            return;
        };
        let change = SizeChange { line, lc, new };
        self.d.objs.size_change = Some(change);
        let old = !new;
        for index in 0..searched {
            let large = change.large(index);
            if large == old {
                continue;
            }
            let base = u16::from(index) * 4;
            let y = self.oam_read(base);
            let x = self.oam_read(base + 1);
            match (self.object_on_line(y, old), self.object_on_line(y, large)) {
                (false, true) if self.d.objs.count < 10 => self.insert_object(index, x, y),
                (true, false) => self.remove_object(index),
                _ => {}
            }
        }
        if searched == 40 {
            self.d.objs.found = self.d.objs.count;
        }
    }

    pub(super) const fn x_for_object_match(&self) -> u8 {
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
    pub(in crate::ppu) const fn abort_object_fetch_on_obj_disable(&mut self, val: u8) {
        if !self.hw_cgb()
            && self.lcdc & LCDC_OBJ_EN_B != 0
            && val & LCDC_OBJ_EN_B == 0
            && self.d.obj_fetch.active
        {
            self.d.cfl -= self.d.wait - 1;
            self.d.wait = 1;
            self.d.obj_fetch.aborted = true;
        }
    }
}
