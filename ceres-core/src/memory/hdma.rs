//! CGB general-purpose and `HBlank` DMA.
//!
//! Port of SameBoy's `GB_hdma_run`: a transfer is started by the CPU at an
//! opcode fetch (so it steals time from the instruction being executed), the
//! whole burst is one uninterrupted loop of 2 dots per byte (4 in double
//! speed) framed by a 2-dot lead-in and, in single speed, a 2-dot tail.

use crate::{AudioCallback, Gb, Model};

// HDMA5 bits
/// Write: an `HBlank` transfer. Read: no transfer is running.
const HDMA5_HBLANK_B: u8 = 0x80;
const HDMA5_IDLE_B: u8 = 0x80;
/// The length in blocks, minus 1.
const HDMA5_BLOCKS: u16 = 0x7F;
/// A transfer copies 16-byte blocks: the low bits of the addresses are 0.
const HDMA_BLOCK_SIZE: u16 = 0x10;

#[expect(
    clippy::struct_excessive_bools,
    reason = "Independent flags of the transfer state machine"
)]
#[derive(Default)]
pub(crate) struct Hdma {
    /// gambatte's `haltHdmaState_` for a speed switch on the CGB-C.
    switch_state: SwitchHdma,
    /// STAT mode was non-zero when the CPU last halted/stopped; an `HBlank`
    /// transfer only starts on wake-up if so.
    allow_on_wake: bool,
    cpu_halted: bool,
    dst: u16,
    in_progress: bool,
    on: bool,
    on_hblank: bool,
    src: u16,
    steps_left: u16,
}

/// What a speed switch's halt does with the `HBlank` transfer: request it
/// at the wake if the wake is in an `HBlank` that did not request it yet
/// (`Low`), not (`High`), or in any case (`Requested`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum SwitchHdma {
    #[default]
    Low,
    High,
    Requested,
}

impl Hdma {
    #[must_use]
    pub(crate) const fn switch_state(&self) -> SwitchHdma {
        self.switch_state
    }

    pub(crate) const fn set_switch_state(&mut self, state: SwitchHdma) {
        self.switch_state = state;
    }

    /// An `HBlank` transfer is requested and has not run yet.
    #[must_use]
    pub(crate) const fn hblank_requested(&self) -> bool {
        self.on && self.on_hblank
    }

    #[must_use]
    pub(crate) const fn hblank_enabled(&self) -> bool {
        self.on_hblank
    }

    /// The request of an `HBlank` transfer is dropped.
    pub(crate) const fn ack_hblank_request(&mut self) {
        if self.on_hblank {
            self.on = false;
        }
    }

    /// An `HBlank` transfer is requested.
    pub(crate) const fn request_hblank(&mut self) {
        if self.on_hblank {
            self.on = true;
        }
    }

    #[must_use]
    pub(crate) const fn is_on(&self) -> bool {
        self.on
    }

    #[must_use]
    pub(crate) const fn read_hdma5(&self) -> u8 {
        // active on low
        (if self.on || self.on_hblank {
            0
        } else {
            HDMA5_IDLE_B
        }) | (self.steps_left.wrapping_sub(1) & HDMA5_BLOCKS) as u8
    }

    #[must_use]
    pub(crate) const fn is_transferring(&self) -> bool {
        self.in_progress
    }

    #[must_use]
    pub(crate) const fn has_multiple_steps_left(&self) -> bool {
        self.steps_left > 1
    }

    #[must_use]
    pub(crate) const fn is_at_block_end(&self) -> bool {
        (self.dst & (HDMA_BLOCK_SIZE - 1)) == HDMA_BLOCK_SIZE - 1
    }

    #[must_use]
    pub(crate) const fn cpu_halted(&self) -> bool {
        self.cpu_halted
    }

    pub(crate) const fn set_cpu_halted(&mut self, halted: bool, mode_is_hblank: bool) {
        self.cpu_halted = halted;
        if halted {
            self.allow_on_wake = !mode_is_hblank;
        }
    }

    pub(crate) const fn note_stop(&mut self, mode_is_hblank: bool) {
        self.allow_on_wake = !mode_is_hblank;
    }

    /// Wake-up from HALT/STOP or interrupt dispatch.
    pub(crate) const fn wake(&mut self, mode_is_hblank: bool) {
        if self.on_hblank && mode_is_hblank && self.allow_on_wake {
            self.on = true;
        }
    }

    /// The PPU reached the start of `HBlank`.
    pub(crate) const fn hblank_edge(&mut self, stopped: bool) {
        if self.on_hblank && !self.cpu_halted && !stopped {
            self.on = true;
        }
    }

    /// The LCD was switched off while STAT reported a non-zero mode.
    pub(crate) const fn lcd_off_edge(&mut self) {
        if self.on_hblank {
            self.on = true;
        }
    }

    pub(crate) fn write_hdma1(&mut self, val: u8) {
        self.src = (self.src & 0xF0) | (u16::from(val) << 8);
        // Range 0xE*** acts like 0xF*** and can't overflow to anything
        // meaningful.
        if self.src >= 0xE000 {
            self.src |= 0xF000;
        }
    }

    pub(crate) const fn write_hdma2(&mut self, val: u8) {
        self.src = (self.src & 0xFF00) | (val as u16 & !(HDMA_BLOCK_SIZE - 1));
    }

    pub(crate) fn write_hdma3(&mut self, val: u8) {
        self.dst = (self.dst & 0xF0) | (u16::from(val) << 8);
    }

    pub(crate) const fn write_hdma4(&mut self, val: u8) {
        self.dst = (self.dst & 0xFF00) | (val as u16 & !(HDMA_BLOCK_SIZE - 1));
    }

    /// `in_hblank`: STAT mode is 0 and the PPU is not at the HBlank/OAM edge.
    pub(crate) const fn write_hdma5(&mut self, val: u8, in_hblank: bool) {
        self.steps_left = (val as u16 & HDMA5_BLOCKS) + 1;
        if val & HDMA5_HBLANK_B == 0 && self.on_hblank {
            // Cancel the running HBlank transfer.
            self.on_hblank = false;
            return;
        }
        self.on = val & HDMA5_HBLANK_B == 0;
        self.on_hblank = val & HDMA5_HBLANK_B != 0;
        if self.on_hblank && in_hblank {
            self.on = true;
        }
    }
}

impl<A: AudioCallback> Gb<A> {
    /// Runs a pending transfer burst. Called right after an opcode fetch,
    /// while that fetch's M-cycle is still pending.
    pub(crate) fn run_hdma(&mut self) {
        let cycles = if self.key1.is_enabled() { 4 } else { 2 };

        self.hdma.in_progress = true;
        self.ppu.set_hdma_state(true, self.hdma.src);
        self.advance_t_cycles(cycles);

        while self.hdma.on {
            let src = self.hdma.src;
            self.ppu.set_hdma_state(true, src);
            // Valid sources: ROM, cart RAM and WRAM. Anything else
            // (VRAM, echo RAM, OAM, I/O) yields the open bus.
            let byte = match src {
                0x0000..=0x7FFF | 0xA000..=0xDFFF => self.read_mem(src),
                _ => 0xFF,
            };
            if self.dma.hdma_can_write_oam(self.key1.is_enabled()) {
                self.hdma_write_oam(src.to_le_bytes()[0], byte);
            }
            self.hdma.src = src.wrapping_add(1);

            self.advance_t_cycles(cycles);

            // The destination is always VRAM, written directly: the PPU's
            // access blocking does not apply, but a write during a blocked
            // phase lands in both banks. If the PPU read VRAM meanwhile the
            // bus was busy and the byte may go astray.
            let mirror = self.ppu.vram_write_blocked();
            if let Some(conflict) = self.ppu.take_hdma_conflict_addr() {
                if self.model == Model::CgbE || self.key1.is_enabled() {
                    let addr = self.hdma.dst & conflict & 0x1FFF;
                    self.ppu.vram_mut().write_hdma(addr, byte, mirror);
                }
                self.hdma.dst = self.hdma.dst.wrapping_add(1);
            } else {
                let addr = self.hdma.dst & 0x1FFF;
                self.hdma.dst = self.hdma.dst.wrapping_add(1);
                self.ppu.vram_mut().write_hdma(addr, byte, mirror);
            }

            if self.hdma.dst & (HDMA_BLOCK_SIZE - 1) == 0
                && self.hdma.cpu_halted()
                && self.key1.is_enabled()
            {
                // A block that ends with the CPU halted by a switch to double
                // speed leaves the length as it was and ends the transfer.
                self.hdma.on = false;
                self.hdma.on_hblank = false;
            } else if self.hdma.dst & (HDMA_BLOCK_SIZE - 1) == 0 {
                self.hdma.steps_left = self.hdma.steps_left.wrapping_sub(1);
                if self.hdma.steps_left == 0 || self.hdma.dst == 0 {
                    self.hdma.on = false;
                    self.hdma.on_hblank = false;
                } else if self.hdma.on_hblank {
                    self.hdma.on = false;
                } else {
                    // A general purpose transfer goes on with the next block.
                }
            } else {
                // The block goes on.
            }
        }

        self.hdma.in_progress = false;
        self.ppu.set_hdma_state(false, self.hdma.src);
        if !self.key1.is_enabled() {
            self.advance_t_cycles(2);
        }
    }
}
