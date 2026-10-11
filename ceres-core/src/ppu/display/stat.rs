//! The STAT and LYC registers. The interrupt and the LY=LYC flag they feed
//! are the timing unit's (`timing.rs`).

use {
    super::state::State,
    crate::{
        interrupts::Interrupts,
        ppu::Ppu,
    },
};

impl Ppu {
    /// The DMA's last cycle: during the OAM scan edge it raises the mode 2
    /// bits (SameBoy `GB_dma_run`).
    pub(crate) fn dma_finished(&mut self) {
        if self.d.state == State::OamScanObject {
            self.stat |= 2;
        }
    }

    pub(in crate::ppu) const fn write_lyc_reg(&mut self, val: u8) {
        self.timing_write_lyc(val);
        self.lyc = val;
    }

    pub(in crate::ppu) const fn write_stat_reg(&mut self, val: u8, ints: &mut Interrupts) {
        self.timing_write_stat(val, ints);
        self.stat = (self.stat & 7) | (val & !7) | 0x80;
    }
}
