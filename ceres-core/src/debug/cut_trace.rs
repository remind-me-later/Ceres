//! A trace of what the blocks of the machine show each other: the interrupt
//! flags, the divider, TIMA, the OAM DMA, LY, the mode STAT shows, and whether
//! the CPU's reads of OAM and VRAM are shut out. It is recorded whenever one
//! of them changes, for comparing with the readable VHDL model of the chip,
//! which writes the same (`hdl/compare-cuts.sh`).

use {
    super::OamDmaPhase,
    crate::{AudioCallback, Gb},
    alloc::vec::Vec,
    core::mem,
};

/// What the blocks show each other at one moment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct CutState {
    /// IF's five bits.
    pub interrupt_flags: u8,
    /// The divider's 16 bits (DIV is the high byte).
    pub div: u16,
    /// TIMA, as a read would see it.
    pub tima: u8,
    /// An OAM DMA is copying.
    pub dma: bool,
    /// LY and STAT's mode bits, as a read would see them.
    pub ly: u8,
    pub stat_mode: u8,
    /// A CPU read of OAM, or of VRAM, would read 0xFF: the PPU or a DMA
    /// has it.
    pub oam_blocked: bool,
    pub vram_blocked: bool,
}

/// The state, and when it began: in the step of time from `since_units` to
/// `clock_units`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct CutSample {
    /// `Gb::clock_units` when the state was seen, at the end of the step.
    pub clock_units: u64,
    /// `Gb::clock_units` when the step began.
    pub since_units: u64,
    pub state: CutState,
}

#[derive(Default)]
pub(crate) struct CutTrace {
    enabled: bool,
    /// The state seen last, recorded or not.
    last: Option<CutSample>,
    samples: Vec<CutSample>,
}

impl<A: AudioCallback> Gb<A> {
    /// Starts recording the state between the blocks, or stops and forgets
    /// it.
    #[inline]
    pub fn set_cut_trace(&mut self, enabled: bool) {
        self.cut_trace.enabled = enabled;
        self.cut_trace.last = None;
        self.cut_trace.samples.clear();
    }

    /// The changes recorded since the last call.
    #[inline]
    pub fn take_cut_trace(&mut self) -> Vec<CutSample> {
        mem::take(&mut self.cut_trace.samples)
    }

    /// Records the state if something but the divider's counting changed in
    /// the step of time that began at `since_units`.
    pub(crate) fn record_cut(&mut self, since_units: u64) {
        if !self.cut_trace.enabled {
            return;
        }
        let state = CutState {
            interrupt_flags: self.read_mem(0xFF0F) & 0x1F,
            div: self.clock.div,
            tima: self.read_mem(0xFF05),
            dma: matches!(
                self.dma.debug_state().phase,
                OamDmaPhase::Copy { .. } | OamDmaPhase::Last
            ),
            ly: self.read_mem(0xFF44),
            stat_mode: self.read_mem(0xFF41) & 3,
            oam_blocked: self.dma.blocks_oam_read()
                || (self.ppu.debug_state().lcd_on && self.ppu.oam_read_blocked()),
            vram_blocked: self.ppu.vram_read_blocked(),
        };
        let sample = CutSample {
            clock_units: self.clock_units,
            since_units,
            state,
        };
        // The divider counts T-cycles.
        let units_per_cycle = if self.key1.is_enabled() { 1 } else { 2 };
        let steady = self.cut_trace.last.is_some_and(|last| {
            let cycles = (sample.clock_units - last.clock_units) / units_per_cycle;
            let div = last
                .state
                .div
                .wrapping_add(u16::try_from(cycles & 0xFFFF).unwrap_or_default());
            CutState { div, ..last.state } == state
        });
        self.cut_trace.last = Some(sample);
        if !steady {
            self.cut_trace.samples.push(sample);
        }
    }
}
