//! The timing of CPU writes to the I/O registers.

use crate::sm83::conflict::{self, ConflictType};
use crate::{
    AudioCallback, Gb, Model,
    memory::{IF, SCX, io_addr},
    ppu::{
        LCDC_BG_EN_B, LCDC_BG_MAP_B, LCDC_OBJ_EN_B, LCDC_OBJ_SIZE_B, LCDC_ON_B, LCDC_TILE_SEL_B,
        LCDC_WIN_EN_B, LCDC_WIN_MAP_B, STAT_IF_HBLANK_B, STAT_IF_LYC_B, STAT_IF_OAM_B,
        STAT_IF_VBLANK_B,
    },
};

impl<A: AudioCallback> Gb<A> {
    /// A CPU write: when the write lands relative to the PPU depends on the
    /// register (`ConflictType`), as in SameBoy's `cycle_write`.
    #[expect(
        clippy::too_many_lines,
        reason = "One arm per register class, like SameBoy's `cycle_write`"
    )]
    pub(super) fn cpu_write(&mut self, addr: u16, val: u8) {
        let conflict = conflict::get_conflict(self.model, self.key1.is_enabled(), addr);

        let pending = self.time_deferred;

        // Port of SameBoy's `cycle_write`: each class says when, relative to
        // the end of the pending M-cycles, the PPU sees the new value. The
        // write's own M-cycle is always 4 dots: what an arm advances past
        // `pending` plus what it leaves in `time_deferred` (a write landing a
        // dot early defers 5, one landing a dot late 3).
        match conflict {
            ConflictType::ReadOld => {
                self.flush_deferred_time();
                self.write_mem(addr, val);
                self.time_deferred = 4;
            }
            ConflictType::ReadNew => {
                self.advance_t_cycles(pending - 1);
                self.write_mem(addr, val);
                self.time_deferred = 5;
            }
            ConflictType::WriteCpu => {
                self.advance_t_cycles(pending + 1);
                // In double speed a write to IF lands after the LCD
                // interrupts of the next cycle (gambatte `updateIrqs(cc + 2)`).
                if addr == io_addr(IF) && self.key1.is_enabled() && self.ppu.gambatte_cgb_timing() {
                    self.ppu
                        .run_ahead(&mut self.ints, self.cgb_mode, true, 1, true);
                }
                self.write_mem(addr, val);
                self.time_deferred = 3;
            }
            // The DMG STAT-write bug is basically the STAT register being
            // read as FF for a single T-cycle.
            ConflictType::StatDmg if self.ppu.gambatte_stat_irq() => {
                // The STAT write bug is in the PPU's STAT interrupt events.
                self.flush_deferred_time();
                self.write_mem(addr, val);
                self.time_deferred = 4;
            }
            ConflictType::StatDmg => {
                self.flush_deferred_time();
                // The write glitches the register for a dot (all the enables are
                // on) before the real value lands. The glitch is a pulse, the
                // real value is in place for what the PPU does in the next dot,
                // except at the edge between HBlank and OAM mode, where the OAM
                // interrupt seems to be blocked by HBlank interrupts.
                let stat = self.ppu.read_stat();
                if self.ppu.at_oam_scan_edge()
                    && stat & (STAT_IF_OAM_B | STAT_IF_HBLANK_B) == STAT_IF_HBLANK_B
                {
                    self.write_mem(addr, !STAT_IF_OAM_B);
                    self.advance_t_cycles(1);
                    self.write_mem(addr, val);
                } else {
                    self.write_mem(addr, 0xFF);
                    self.write_mem(addr, val);
                    self.advance_t_cycles(1);
                }
                self.time_deferred = 3;
            }
            ConflictType::StatCgb => {
                // The LYC and the VBlank enables reach the PPU a dot after the
                // others (the HBlank one too when it is turned off).
                const LATE: u8 = STAT_IF_LYC_B | STAT_IF_VBLANK_B;

                let old = self.ppu.read_stat();
                self.flush_deferred_time();
                let mut early = (old & LATE) | (val & !LATE);
                early |= old & !val & STAT_IF_HBLANK_B;
                if val & !old & STAT_IF_LYC_B != 0 {
                    // Enabling the LYC source: the enables this write clears go
                    // with it, so that no source drops out for a dot in between.
                    early |= old & (STAT_IF_HBLANK_B | STAT_IF_VBLANK_B | STAT_IF_OAM_B);
                }
                self.write_mem(addr, early);
                self.advance_t_cycles(1);
                self.write_mem(addr, val);
                self.time_deferred = 3;
            }
            ConflictType::StatCgbDouble => {
                let old = self.ppu.read_stat();
                self.flush_deferred_time();
                self.write_mem(addr, (val & !STAT_IF_HBLANK_B) | (old & STAT_IF_HBLANK_B));
                self.advance_t_cycles(1);
                self.write_mem(addr, val);
                self.time_deferred = 3;
            }
            ConflictType::PaletteDmg => {
                self.advance_t_cycles(pending - 2);
                let old = self.read_mem(addr);
                self.write_mem(addr, val | old);
                self.advance_t_cycles(1);
                self.write_mem(addr, val);
                self.time_deferred = 5;
            }
            ConflictType::PaletteCgb => {
                if matches!(self.model, Model::CgbD | Model::CgbE | Model::Agb) {
                    self.advance_t_cycles(pending - 2);
                    self.write_mem(addr, val);
                    self.time_deferred = 6;
                } else {
                    self.advance_t_cycles(pending - 1);
                    self.write_mem(addr, val);
                    self.time_deferred = 5;
                }
            }
            // LCDC.1 is read both by the FIFO when popping pixels and by the
            // object-fetching state machine, and the two behave differently
            // when it comes to access conflicts.
            ConflictType::DmgLcdc => {
                // Bits the tile fetcher consumes (BG_MAP, TILE_SEL, WIN_MAP)
                // are seen by the PPU one dot before the ones the pixel mixer
                // consumes (BG_EN, WIN_EN, OBJ_EN): measured on DMG against
                // the mealybug LCDC tests. OBJ_SIZE reaches the object fetch
                // early too, but the object search only sees it with the rest.
                const FETCHER_BITS: u8 = LCDC_BG_MAP_B | LCDC_TILE_SEL_B | LCDC_WIN_MAP_B;

                let mut old = self.read_mem(addr);
                self.advance_t_cycles(pending - 2);
                if (self.model != Model::Mgb && self.ppu.fifo_position() == 0
                    || self.ppu.is_fetching_sprite())
                    && val & LCDC_OBJ_EN_B == 0
                {
                    old &= !LCDC_OBJ_EN_B;
                }

                self.write_mem(addr, (old & !FETCHER_BITS) | (val & FETCHER_BITS));
                // The object fetch (not the object search) sees OBJ_SIZE early.
                self.ppu.set_obj_size_fetch(val & LCDC_OBJ_SIZE_B != 0);
                self.advance_t_cycles(1);
                self.write_mem(addr, val);

                self.ppu.note_window_disable(old, val);
                self.time_deferred = 5;
            }
            ConflictType::SgbLcdc => {
                // Simplified version of the above.
                let old = self.read_mem(addr);
                self.advance_t_cycles(pending - 2);
                // Hack to force aborting an object fetch.
                self.write_mem(addr, val);
                self.write_mem(addr, old);
                self.advance_t_cycles(1);
                self.write_mem(addr, val);
                self.time_deferred = 5;
            }
            ConflictType::WxDmg => {
                self.advance_t_cycles(pending);
                self.write_mem(addr, val);
                self.ppu.set_wx_just_changed(true);
                self.advance_t_cycles(1);
                self.ppu.set_wx_just_changed(false);
                self.time_deferred = 3;
            }
            ConflictType::LcdcCgb => {
                // OBJ_SIZE reaches the object fetcher one dot after the other
                // bits reach the PPU (measured on CGB-C).
                let old = self.ppu.read_lcdc();
                self.advance_t_cycles(pending);
                self.ppu.cgb_obj_size_write(val, 0);
                if self.ppu.gambatte_cgb_timing() {
                    self.ppu.gstat_write_lcdc(val, 0);
                }
                let delay_obj_size = self.ppu.read_scx() & 7 != 0;
                self.write_mem(
                    addr,
                    if delay_obj_size {
                        (val & !LCDC_OBJ_SIZE_B) | (old & LCDC_OBJ_SIZE_B)
                    } else {
                        val
                    },
                );
                // Changing TILE_SEL on the dot after the write can corrupt a
                // bitplane read in flight (see the PPU). The window start sees
                // the window being turned on a dot late.
                self.ppu
                    .set_tile_sel_glitch((val ^ old) & LCDC_TILE_SEL_B != 0);
                self.ppu.set_window_enable_pending(
                    old & LCDC_WIN_EN_B == 0 && val & LCDC_WIN_EN_B != 0,
                );
                self.advance_t_cycles(1);
                self.ppu.set_tile_sel_glitch(false);
                self.ppu.set_window_enable_pending(false);
                self.write_mem(addr, val);
                self.ppu.gstat_lcdc_write_done();
                self.time_deferred = 3;
            }
            ConflictType::LcdcCgbDouble => {
                let old = self.ppu.read_lcdc();
                self.advance_t_cycles(pending - 2);
                self.ppu.cgb_obj_size_write(val, 2);
                if self.ppu.gambatte_cgb_timing() {
                    self.ppu.gstat_write_lcdc(val, 2);
                }
                // Turning the window on waits for the end of the write too.
                let late = LCDC_ON_B | LCDC_BG_EN_B | (!old & LCDC_WIN_EN_B);
                self.write_mem(addr, (val & !late) | (old & late));
                self.ppu
                    .set_tile_sel_glitch((val ^ old) & LCDC_TILE_SEL_B != 0);
                self.advance_t_cycles(2);
                self.ppu.set_tile_sel_glitch(false);
                self.write_mem(addr, val);
                self.ppu.gstat_lcdc_write_done();
                self.time_deferred = 4;
            }
            // Registers the tile fetcher consumes land a dot before the ones the
            // pixel mixer consumes (see `DmgLcdc`).
            ConflictType::ScxDmgAndCgbDouble | ConflictType::ScyDmg => {
                // The tile fetcher sees these two dots before the pixel mixer.
                // On the DMG the mixer, which discards SCX's low bits, sees them
                // a dot before the end of the write.
                let old = self.ppu.read_scx();
                self.advance_t_cycles(pending - 2);
                if self.model.is_cgb_hardware() || addr != io_addr(SCX) {
                    self.write_mem(addr, val);
                    self.time_deferred = 6;
                } else {
                    self.write_mem(addr, (old & 7) | (val & !7));
                    self.advance_t_cycles(1);
                    self.write_mem(addr, val);
                    self.time_deferred = 5;
                }
            }
            ConflictType::Nr10CgbDouble => {
                self.advance_t_cycles(pending - 1);
                self.advance_t_cycles(1);
                self.write_mem(addr, val);
                self.time_deferred = 4;
            }
        }
        self.address_bus = addr;
    }
}
