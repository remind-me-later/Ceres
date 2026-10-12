//! The hardware's internal state, for a debugger to show: what the PPU, the
//! timer and the DMAs are doing, beyond what their registers say.

use crate::{AudioCallback, Gb};

/// Where the display is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct PpuState {
    pub lcd_on: bool,
    /// The mode STAT shows (0 to 3).
    pub mode: u8,
    /// The line and the half dot in it on the timing unit's clock
    /// (`ppu/display/timing.rs`), which drives LY reads and the STAT
    /// interrupt. Always counted while the LCD is on.
    pub line: u8,
    pub half_dot: i32,
    /// The pixel pipeline's line (it starts line 0 near the end of
    /// line 153).
    pub pipeline_line: u8,
    pub double_speed: bool,
}

/// The timer's internal state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct TimerState {
    /// The 16-bit counter DIV is the high byte of.
    pub div_counter: u16,
    /// TIMA itself (reads give 0 while a reload is pending).
    pub tima: u8,
    pub tma: u8,
    pub tac: u8,
    /// T-cycles until TIMA is reloaded from TMA (0: none pending).
    pub reload_pending: u8,
    /// T-cycles until the timer interrupt is requested (0: none pending).
    pub irq_countdown: u8,
}

/// Where an OAM DMA transfer is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum OamDmaPhase {
    Idle,
    /// The start-up delay after a write to DMA.
    StartUp,
    /// Copying: the next step writes OAM byte `next`.
    Copy {
        next: u8,
    },
    /// The step after the last byte, which hands OAM back.
    Last,
}

/// An OAM DMA transfer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct OamDmaState {
    pub phase: OamDmaPhase,
    /// The address the next byte comes from.
    pub source: u16,
}

/// What an HDMA transfer is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum HdmaKind {
    Idle,
    /// A general-purpose transfer: all the blocks in one burst.
    General,
    /// An `HBlank` transfer: a block per `HBlank`, once requested.
    HBlank {
        requested: bool,
    },
}

/// A CGB HDMA transfer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct HdmaState {
    pub kind: HdmaKind,
    /// The next source and destination addresses.
    pub source: u16,
    pub destination: u16,
    /// The 16-byte blocks left.
    pub blocks_left: u16,
}

impl<A: AudioCallback> Gb<A> {
    #[must_use]
    #[inline]
    pub const fn ppu_state(&self) -> PpuState {
        self.ppu.debug_state()
    }

    #[must_use]
    #[inline]
    pub const fn timer_state(&self) -> TimerState {
        let clock = &self.clock;
        TimerState {
            div_counter: clock.div,
            tima: clock.tima,
            tma: clock.tma,
            tac: clock.tac,
            reload_pending: clock.tima_reload_pending,
            irq_countdown: clock.tima_irq_countdown,
        }
    }

    #[must_use]
    #[inline]
    pub const fn oam_dma_state(&self) -> OamDmaState {
        self.dma.debug_state()
    }

    #[must_use]
    #[inline]
    pub const fn hdma_state(&self) -> HdmaState {
        self.hdma.debug_state()
    }

    /// 8 MHz units (half dots) the machine has run since it was made: a
    /// clock for traces. States don't hold it, so loading one doesn't
    /// change it.
    #[must_use]
    #[inline]
    pub const fn clock_units(&self) -> u64 {
        self.clock_units
    }
}
