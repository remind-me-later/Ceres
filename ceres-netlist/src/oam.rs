//! The netlist's OAM, as far as the OAM scan reads it.
//!
//! Its two stages of latches of the even bytes (the Y), transcribed from
//! `hdl/rtl/oam.vhd`, for ceres-core's proofs against ceres's object search
//! and for the replay of the model's traces (`tests/replay.rs`), which
//! checks the transcription.

/// What OAM's latches take in after an edge (`hdl/rtl/oam.vhd`), while the
/// CPU does not address OAM.
#[derive(Clone, Copy)]
#[expect(clippy::struct_excessive_bools, reason = "the strobes' levels")]
pub struct OamInputs {
    /// The scan runs (`crate::oam_scan::NetScan::scanning`).
    pub scanning: bool,
    /// OAM DMA runs, and its clock.
    pub run: bool,
    pub dma_phi: bool,
    pub wuvu: bool,
    pub wosu: bool,
    /// The sprite fetch reads OAM, and holds the second latches.
    pub fetch_read: bool,
    pub fetch_hold: bool,
}

impl OamInputs {
    /// Mode 2 for OAM: OAM DMA hides OAM from the scan.
    // @hw die: ACYL BOGE
    #[must_use]
    pub const fn mode2(self) -> bool {
        self.scanning && !self.run
    }

    /// The first latches are open: the scan reads, while `wosu` is low, or
    /// the sprite fetch reads.
    // @hw die: BOTA, AJEP XUJA ASYT BODE
    #[must_use]
    pub const fn latch_open(self) -> bool {
        (self.mode2() && !self.wosu) || self.fetch_read
    }

    /// The RAM is precharged, and the second latches take the first: out of
    /// mode 2 (while OAM DMA runs, only while `dma_phi` is high) unless the
    /// sprite fetch holds them, and in mode 2 as `wuvu` rises.
    // @hw die: CUFE XUJY AVER BYCU COTA
    #[must_use]
    pub const fn precharge(self) -> bool {
        (!self.run || self.dma_phi)
            && !self.fetch_hold
            && !(self.mode2() && !(self.wosu && self.wuvu))
    }
}

/// OAM's two stages of latches of the even bytes (Y, or a sprite's tile).
///
/// The first take the data lines while they are open, the second the first
/// while the RAM is precharged; the scan's comparator sees the second.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct NetOamLatches {
    pub first: u8,
    pub second: u8,
}

impl NetOamLatches {
    /// After an edge, with what the latches take in after it and the byte
    /// the data lines carry (the RAM's, while the scan reads it).
    // @hw die: YDYV..ECED (dlatch), XUSO..DEPO (dlatch_ee)
    pub const fn settle(&mut self, inputs: OamInputs, data: u8) {
        if inputs.latch_open() {
            self.first = data;
        }
        if inputs.precharge() {
            self.second = self.first;
        }
    }
}
