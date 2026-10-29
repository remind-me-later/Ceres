//! The netlist's OAM scan, transcribed from `hdl/rtl/oam_scan.vhd` edge by
//! edge, for the proofs against ceres's object search (`netlist_proofs.rs`)
//! and for the replay of the model's traces (`crate::netlist_replay`),
//! which checks the transcription.
//!
//! The scan runs on the line timing's clocks (`NetLcd`): `wuvu`, whose
//! period is two dots, counts the sprites, one a period, and the chip's
//! clock (`clk`) times the saves. A scan starts as `wuvu` rises at a line's end (vclk),
//! unless the next line is in VBlank; it reads each sprite's Y from OAM,
//! and saves the first ten on the line, in index order, in the sprite
//! store's slots; it stops a `wuvu` period after the last sprite, which
//! starts mode 3.

// Some of it only the proofs use, which other builds leave out.
#![cfg_attr(not(kani), allow(dead_code))]
#![expect(
    clippy::field_scoped_visibility_modifiers,
    reason = "the proofs and the replay set and compare the flip-flops one by one"
)]

/// The last sprite's index.
const LAST: u8 = 39;
/// The sprite store's slots.
pub(crate) const SLOTS: u8 = 10;

/// What the scan takes from the line timing in an edge: the dividers before
/// and after it, and vclk and LY before it.
#[derive(Clone, Copy)]
#[expect(clippy::struct_excessive_bools, reason = "the clocks' levels")]
pub(crate) struct Clocks {
    /// The edge is the chip clock's rise.
    pub(crate) rising: bool,
    pub(crate) wuvu_before: bool,
    pub(crate) wuvu_after: bool,
    pub(crate) wosu_before: bool,
    pub(crate) vclk: bool,
    pub(crate) ly: u8,
}

/// The netlist's OAM scan, between edges of the chip's clock.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[expect(clippy::struct_excessive_bools, reason = "the netlist's flip-flops")]
pub(crate) struct NetScan {
    /// CATU, ANEL: the scan starts; the start, a half `wuvu` period later.
    pub(crate) start_scan: bool,
    pub(crate) anel: bool,
    /// BESU: the scan runs (mode 2, also while OAM DMA hides it).
    pub(crate) scanning: bool,
    /// CENO: `scanning`, at `wuvu`'s rise.
    pub(crate) ceno: bool,
    /// YFEL..FONY: the sprite being read.
    pub(crate) index: u8,
    /// BYBA, DOBA: the scan is past the last sprite.
    pub(crate) byba: bool,
    pub(crate) doba: bool,
    /// DEZY: no save in the last half dot.
    pub(crate) not_saving: bool,
    /// BESE..DYBE: the sprites saved.
    pub(crate) count: u8,
}

impl NetScan {
    /// Whether a sprite whose Y is `y` is on line `ly`: `y - ly - 1` is 8 to
    /// 15 (0 to 15 if `tall`), the sprite's lines from the bottom.
    // @hw die: ERUC ENEF FECO GYKY GOPU FUWA GOJU WUHU (full_add), WOTA GESE
    pub(crate) const fn y_match(y: u8, ly: u8, tall: bool) -> bool {
        let difference = y as u16 + !ly as u16;
        difference & 0x100 != 0 && difference & 0xF0 == 0 && (tall || difference & 8 != 0)
    }

    /// From the start until `anel` follows, the line starts over: the
    /// index, the count and the stop are cleared.
    // @hw die: BYHA ATEJ, ANOM
    pub(crate) const fn line_start(&self) -> bool {
        self.start_scan && !self.anel
    }

    /// The scan is done: mode 3 starts.
    // @hw die: BEBU AVAP
    pub(crate) const fn stop_scan(&self) -> bool {
        self.byba && !self.doba && !self.line_start()
    }

    /// A sprite on the line is saved while `wosu` is low, with its `y`, LY
    /// and LCDC bit 2.
    // @hw die: CARE DYTY
    pub(crate) const fn save_enable(&self, wosu: bool, y: u8, ly: u8, tall: bool) -> bool {
        !wosu && self.ceno && Self::y_match(y, ly, tall)
    }

    /// The slot a save goes to, if the store has room.
    // @hw die: the decoders of the slots
    pub(crate) const fn save(&self, wosu: bool, y: u8, ly: u8, tall: bool) -> Option<u8> {
        if self.save_enable(wosu, y, ly, tall) && self.count < SLOTS {
            Some(self.count)
        } else {
            None
        }
    }

    /// The next edge of the chip's clock, with what the line timing's edge
    /// does (`clocks`), the Y read from OAM and LCDC bit 2 before it.
    pub(crate) const fn edge(&mut self, clocks: Clocks, y: u8, tall: bool) {
        if clocks.rising {
            // @hw die: DOBA (dffr)
            self.doba = self.byba;
        } else {
            // @hw die: DEZY (dffr): the save as the clock falls
            let saving = self.save_enable(clocks.wosu_before, y, clocks.ly, tall);
            let not_saving_rose = !saving && !self.not_saving;
            self.not_saving = !saving;
            let full = self.count & 0b1010 == 0b1010;
            if not_saving_rose && !full {
                // @hw die: BESE..DYBE (dffr)
                self.count += 1;
            }
            if !clocks.wuvu_before && clocks.wuvu_after {
                // @hw die: CATU (dffr), ABOV ALES XYVO; CENO (dffr); BYBA
                // (dffr); YFEL..FONY (dffr), GAVA
                let last = self.index == LAST;
                self.start_scan = clocks.vclk && clocks.ly & 0x90 != 0x90;
                self.ceno = self.scanning;
                self.byba = last;
                if !last {
                    self.index += 1;
                }
            }
            if clocks.wuvu_before && !clocks.wuvu_after {
                // @hw die: ANEL (dffr), AWOH
                self.anel = self.start_scan;
            }
        }
        if self.line_start() {
            self.index = 0;
            self.count = 0;
            self.byba = false;
            self.doba = false;
        }
        // @hw die: BESU (nor_latch), ASEN
        if self.stop_scan() {
            self.scanning = false;
        } else if self.start_scan {
            self.scanning = true;
        } else {
            // The latch holds.
        }
    }

    /// The LCD's reset (`ppu_reset`).
    #[cfg_attr(
        kani,
        expect(
            dead_code,
            reason = "the replay resets it, the proofs start after the reset"
        )
    )]
    pub(crate) const fn reset(&mut self) {
        self.start_scan = false;
        self.anel = false;
        self.scanning = false;
        self.ceno = false;
        self.not_saving = false;
        // `line_start` holds the counters.
        self.index = 0;
        self.count = 0;
        self.byba = false;
        self.doba = false;
    }
}
