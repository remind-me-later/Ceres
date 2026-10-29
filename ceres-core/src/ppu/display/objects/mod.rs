//! Objects: the mode 2 search for the objects on the line and the state of
//! the object fetch in mode 3.

#[cfg(any(kani, test))]
pub(super) mod netlist;
#[cfg(kani)]
mod netlist_proofs;

use {
    super::{State, TILE_BYTES, VRAM_BANK1},
    crate::{
        Model,
        memory::{DmaPhase, OAM_START},
        ppu::{
            ATTR_BANK_B, ATTR_Y_FLIP_B, LCDC_OBJ_EN_B, LCDC_OBJ_SIZE_B, LCDC_ON_B, Oam, Ppu,
            oam_bug::NO_ROW,
        },
    },
};

/// The most objects drawn on a line.
const MAX_OBJECTS_PER_LINE: usize = 10;

/// An object on the line.
#[derive(Clone, Copy, Default)]
pub(super) struct Entry {
    /// Its OAM index.
    pub index: u8,
    pub x: u8,
    pub y: u8,
}

/// The mode 2 object search.
#[derive(Clone)]
pub(super) struct ObjectSearch {
    /// The objects left to draw: mode 3 drops them from the end.
    pub count: usize,
    /// The objects the search found (`count` when mode 3 starts).
    pub found: usize,
    /// The objects on the line, sorted by X then OAM index, descending: the
    /// next one to draw is the last.
    pub entries: [Entry; MAX_OBJECTS_PER_LINE],
    pub index: u8,
    /// OAM row the PPU is reading (DMG OAM bug); `NO_ROW` when none.
    pub accessed_oam_row: u8,
    /// The OAM's two output latches: the last pair of bytes the PPU read,
    /// an entry's Y and X from the search, or an object's tile and
    /// attributes from the fetch. While a DMA runs the search does not read
    /// OAM and compares each entry with these.
    // @fact oam-scan-during-dma
    pub y_bus: u8,
    pub x_bus: u8,
    /// A CGB-C `OBJ_SIZE` write during this line's search (see
    /// `cgb_obj_size_write`).
    pub size_change: Option<SizeChange>,
}

/// `OBJ_SIZE` changed while the CGB-C was searching `line`: an entry samples
/// the size at line cycles `2 * index` and `2 * index + 1` on gambatte's
/// clock and is large if either sample is; the samples after `lc` see `new`.
#[derive(Clone, Copy)]
pub(super) struct SizeChange {
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

/// An object whose Y is `y` is on `line`: Y - 16 is the object's top line.
pub(super) const fn on_line(y: u8, line: u8, height_16: bool) -> bool {
    let top = y as i32 - 16;
    let line = line as i32;
    top <= line && top + if height_16 { 16 } else { 8 } > line
}

impl ObjectSearch {
    /// The search looks at the object `index` (X `x`, Y `y`), and keeps it
    /// if it is on the line and the list has room.
    pub(super) fn consider(&mut self, index: u8, x: u8, y: u8, on_line: bool) {
        if on_line && self.count < MAX_OBJECTS_PER_LINE {
            self.insert(index, x, y);
        }
    }

    /// Inserts an object in the list, sorted by X (descending), then by OAM
    /// index (descending).
    fn insert(&mut self, index: u8, obj_x: u8, obj_y: u8) {
        let count = self.count;
        let at = self.entries[..count]
            .iter()
            .position(|e| e.x < obj_x || (e.x == obj_x && e.index < index))
            .unwrap_or(count);
        self.entries.copy_within(at..count, at + 1);
        self.entries[at] = Entry {
            index,
            x: obj_x,
            y: obj_y,
        };
        self.count += 1;
    }

    /// The search reads an entry: the OAM's output latches take its Y and X
    /// (`read`), unless an OAM DMA hides OAM from the search (`None`), and
    /// the search compares what they hold.
    // @fact oam-scan-during-dma
    pub(super) const fn latch(&mut self, read: Option<(u8, u8)>) -> (u8, u8) {
        if let Some((y, x)) = read {
            self.y_bus = y;
            self.x_bus = x;
        }
        (self.y_bus, self.x_bus)
    }

    /// The next object to draw, if any.
    pub(super) fn next(&self) -> Option<Entry> {
        self.count.checked_sub(1).map(|i| self.entries[i])
    }

    /// The object being fetched (mode 3 only fetches with one left).
    pub(super) const fn fetched(&self) -> Entry {
        self.entries[self.count - 1]
    }
}

impl Default for ObjectSearch {
    fn default() -> Self {
        Self {
            count: 0,
            found: 0,
            entries: [Entry::default(); MAX_OBJECTS_PER_LINE],
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
pub(super) struct ObjectFetch {
    /// The tile number and the attributes read from the OAM.
    pub tile: u8,
    pub flags: u8,
    pub line_address: u16,
    pub data: [u8; 2],
    pub active: bool,
    pub aborted: bool,
    /// `OBJ_SIZE` as the object fetch sees it (it can land a dot before the
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
        if let Some(written) = self.d.bus.dma_phase.written() {
            if self.d.bus.hdma_in_progress {
                return self
                    .oam_read_row(((self.d.bus.hdma_src & !1) | (addr & 1)).to_le_bytes()[0]);
            }
            if written != Oam::SIZE {
                return self.oam.read(u16::from(written & !1) | (addr & 1));
            }
        }
        self.oam.read(addr)
    }

    /// OAM as the sprite fetch reads a sprite's tile and attributes. On the
    /// DMG, while a DMA runs, OAM's latches take the DMA's data lines, which
    /// carry the byte it copies, as the even byte and the odd one (the
    /// netlist's `oam.vhd`). They close two dots after this read, after the
    /// DMA's step in the chunk the PPU is running: the byte is the one the
    /// DMA has copied by the chunk's end.
    // @fact oam-fetch-during-dma
    pub(super) fn oam_read_fetch(&self, addr: u16) -> u8 {
        if !self.is_cgb_hardware() && !self.d.bus.oam_ppu_blocked && self.d.bus.dma_runs_next {
            return self.d.bus.dma_byte_next.unwrap_or(self.d.bus.dma_byte);
        }
        self.oam_read(addr)
    }

    /// SameBoy's `GB_read_oam`: the 160 bytes, then the unusable area.
    pub(super) fn oam_read_row(&self, addr: u8) -> u8 {
        if addr < Oam::SIZE {
            self.oam.read(u16::from(addr))
        } else {
            self.read_unusable(OAM_START | u16::from(addr))
        }
    }

    /// The DMA's state as the object search sees it: on CGB the search runs
    /// ahead of it, by two T-cycles in single speed, by a whole M-cycle in
    /// double speed.
    const fn search_dma_phase(&self) -> DmaPhase {
        if self.search_sees_dma_ahead() {
            self.d.bus.dma_phase_next
        } else {
            self.d.bus.dma_phase
        }
    }

    /// Whether the DMA runs, as the object search sees it (see
    /// `search_dma_phase`).
    const fn search_dma_runs(&self) -> bool {
        if self.search_sees_dma_ahead() {
            self.d.bus.dma_runs_next
        } else {
            self.d.bus.dma_runs
        }
    }

    const fn search_sees_dma_ahead(&self) -> bool {
        let lead = if self.double_speed { 3 } else { 2 };
        self.is_cgb_hardware() && self.d.bus.chunk_left <= lead
    }

    /// A DMA runs (it has written its first byte, or was restarted while it
    /// ran): the object search does not read OAM until the transfer ends,
    /// and every entry sees what the latches hold (on the DMG-CPU B's
    /// netlist, the scan's latches stay closed while the DMA runs). On the
    /// DMG, also while the CPU is halted (the DMA's clock stays high, and
    /// the transfer, paused, keeps OAM); the CGB's search is the CPU's then
    /// (see `add_object_from_index`).
    // @fact stat-during-oam-dma
    // @fact oam-scan-during-halted-dma
    // @fact oam-scan-during-dma-restart
    #[must_use]
    pub(in crate::ppu) const fn dma_hides_object_search(&self) -> bool {
        (!self.d.bus.cpu_idle || !self.is_cgb_hardware()) && self.search_dma_runs()
    }

    pub(super) fn add_object_from_index(&mut self, index: u8) {
        let base = u16::from(index) * Oam::ENTRY_SIZE;
        let dma_active = self.search_dma_phase() != DmaPhase::Idle;
        let read = (!self.dma_hides_object_search()).then(|| {
            (
                self.oam_read(base + Oam::ENTRY_Y),
                self.oam_read(base + Oam::ENTRY_X),
            )
        });
        let (y, x) = self.d.objs.latch(read);

        if self.d.objs.count == MAX_OBJECTS_PER_LINE {
            return;
        }

        // A halted DMA blocks the object search on the CGBs before the CGB-E
        // (like SameBoy, which notes that units before the CGB vary; the
        // DMG-CPU B's netlist compares the latches, above).
        // @fact oam-scan-during-halted-dma
        if dma_active
            && self.d.bus.cpu_idle
            && self.is_cgb_hardware()
            && !matches!(self.model, Model::CgbE | Model::Agb)
        {
            return;
        }

        if self.d.bus.oam_ppu_blocked {
            return;
        }

        let height_16 = match self.d.objs.size_change {
            Some(c) if c.line == self.d.current_line => c.large(index),
            _ => self.lcdc & LCDC_OBJ_SIZE_B != 0,
        };
        self.d
            .objs
            .consider(index, x, y, on_line(y, self.d.current_line, height_16));
    }

    fn remove_object(&mut self, index: u8) {
        let objs = &mut self.d.objs;
        let n = objs.count;
        let Some(j) = objs.entries[..n].iter().position(|e| e.index == index) else {
            return;
        };
        objs.entries.copy_within(j + 1..n, j);
        objs.count -= 1;
    }

    /// A CGB-C write that changes `OBJ_SIZE`, `units_early` before the point a
    /// read in the same cycle would see. The search samples the size later
    /// than this PPU looks at each entry (gambatte's `OamReader`), so the
    /// entries already searched are looked at again.
    pub(crate) fn cgb_obj_size_write(&mut self, val: u8, units_early: i64) {
        let new = val & LCDC_OBJ_SIZE_B != 0;
        if !self.cgb_c_timing()
            || self.lcdc & LCDC_ON_B == 0
            || (self.lcdc & LCDC_OBJ_SIZE_B != 0) == new
        {
            return;
        }
        let searched = match self.d.state() {
            State::OamScanNext => self.d.objs.index,
            State::OamScanObject => self.d.objs.index + 1,
            State::Mode3PalettesLock | State::Mode3Start => Oam::OBJECTS,
            State::LineOamWriteLock | State::LineLy | State::OamScanStart => 0,
            _ => return,
        };
        let line = self.d.current_line;
        let Some(lc) = self.cgb_c_size_change_cycle(units_early) else {
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
            let base = u16::from(index) * Oam::ENTRY_SIZE;
            let y = self.oam_read(base + Oam::ENTRY_Y);
            let x = self.oam_read(base + Oam::ENTRY_X);
            match (on_line(y, line, old), on_line(y, line, large)) {
                (false, true) => self.d.objs.consider(index, x, y, true),
                (true, false) => self.remove_object(index),
                _ => {}
            }
        }
        if searched == Oam::OBJECTS {
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
        if flags & ATTR_Y_FLIP_B != 0 {
            tile_y ^= if height_16 { 0xF } else { 7 };
        }
        // 8x16 objects ignore bit 0 of the tile number.
        let tile = if height_16 { tile & 0xFE } else { tile };
        let mut address = u16::from(tile) * TILE_BYTES + u16::from(tile_y) * 2;
        if self.cgb_mode_on() && flags & ATTR_BANK_B != 0 {
            address += VRAM_BANK1;
        }
        address
    }

    /// Object priority: OAM-index priority on CGB hardware unless OPRI
    /// selects X-coordinate priority; DMG hardware always uses X.
    #[inline]
    pub(super) const fn opri_index_priority(&self) -> bool {
        self.is_cgb_hardware() && !self.opri
    }

    pub(crate) const fn set_obj_size_fetch(&mut self, big: bool) {
        self.d.obj_fetch.size_16 = big;
    }

    /// DMG: disabling objects while an object is being fetched aborts it.
    pub(in crate::ppu) const fn abort_object_fetch_on_obj_disable(&mut self, val: u8) {
        if !self.is_cgb_hardware()
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
