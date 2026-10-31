//! The netlist's OAM DMA, phase by phase.
//!
//! Transcribed from `hdl/rtl/dma.vhd`, for ceres-core's proofs against
//! ceres and for the replay of the model's traces (`tests/replay.rs`),
//! which checks the transcription.
//!
//! The DMA's clock (`dma_phi`) is high in phases A to D: its flip-flops take
//! their inputs as it rises, at the start of A, or as it falls, at the start
//! of E. Its latches follow at once. The CPU writes FF46 in E to G, and the
//! DMA writes OAM, while it runs, at the counter's index, while `dma_phi` and
//! `maka` (`clk_t4` half a clock later) are low: E, F and half of G
//! (`hdl/rtl/oam.vhd`).

use crate::Phase;

/// The netlist's OAM DMA, between phases.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[expect(clippy::struct_excessive_bools, reason = "the netlist's flip-flops")]
pub struct NetDma {
    /// FF46: a latch, open while the CPU writes it.
    pub source: u8,
    /// LYXE: a write asked for a transfer.
    pub requested: bool,
    /// LUVY, LENE: the start, through a flip-flop on each edge of `dma_phi`.
    pub start_1: bool,
    pub start_2: bool,
    /// LOKY LARA: the transfer is on, from its start to its last byte.
    pub enabled: bool,
    /// The low byte of the address.
    pub count: u8,
    /// MYTE: the last byte's address showed.
    pub done: bool,
    /// MATU: a byte is copied in this M-cycle.
    pub run: bool,
}

impl NetDma {
    /// The count that ends the transfer: bit 7 and bits 0 to 4 set (9F).
    const fn last(count: u8) -> bool {
        count & 0x9F == 0x9F
    }

    /// The latches, after the flip-flops changed and while the CPU writes
    /// FF46 (`write`) or not.
    pub const fn settle(&mut self, write: Option<u8>) {
        if let Some(val) = write {
            self.source = val;
        }
        // The restart (start_2) clears the count and done, and the request
        // unless the CPU writes again.
        if self.start_2 {
            self.count = 0;
            self.done = false;
        }
        self.requested = write.is_some() | (self.requested & !self.start_2);
        // The start wins over the end.
        self.enabled = self.start_2 | (self.enabled & !self.done);
    }

    /// One phase, with what the CPU writes to FF46 in it, while the CPU
    /// runs: a rising `dma_phi` starts A, a falling one starts E.
    pub const fn phase(&mut self, phase: Phase, write: Option<u8>) {
        match phase {
            Phase::A => self.rise(write.is_some()),
            Phase::E => self.fall(),
            Phase::B | Phase::C | Phase::D | Phase::F | Phase::G | Phase::H => {}
        }
        self.settle(write);
    }

    /// `dma_phi` rises, while the CPU writes FF46 (`writing`) or not: the
    /// flip-flops clocked by it take their inputs.
    pub const fn rise(&mut self, writing: bool) {
        let was = *self;
        self.start_1 = was.requested & !writing;
        if was.enabled & !was.start_2 {
            self.count = was.count.wrapping_add(1);
        }
        self.run = was.enabled;
    }

    /// `dma_phi` falls.
    pub const fn fall(&mut self) {
        let was = *self;
        self.start_2 = was.start_1;
        self.done = !was.start_2 & Self::last(was.count);
    }

    /// The OAM byte the DMA writes in this M-cycle, and the address it
    /// copies from, as the write ends (in G): the state after phase G.
    #[must_use]
    pub const fn oam_write(self) -> Option<(u16, u8)> {
        if self.run {
            Some((u16::from_le_bytes([self.count, self.source]), self.count))
        } else {
            None
        }
    }
}
