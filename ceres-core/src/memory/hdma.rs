//! CGB general-purpose and `HBlank` DMA.
//!
//! Port of SameBoy's `GB_hdma_run`: a transfer is started by the CPU at an
//! opcode fetch (so it steals time from the instruction being executed), the
//! whole burst is one uninterrupted loop of 2 dots per byte (4 in double
//! speed) framed by a 2-dot lead-in and, in single speed, a 2-dot tail.

use crate::{AudioCallback, Gb};

#[derive(Default)]
pub struct Hdma {
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

impl Hdma {
    #[must_use]
    pub const fn is_on(&self) -> bool {
        self.on
    }

    #[must_use]
    pub const fn read_hdma5(&self) -> u8 {
        // active on low
        (((!(self.on || self.on_hblank)) as u8) << 7)
            | (self.steps_left.wrapping_sub(1) & 0x7F) as u8
    }

    #[must_use]
    pub const fn is_transferring(&self) -> bool {
        self.in_progress
    }

    #[must_use]
    pub const fn has_multiple_steps_left(&self) -> bool {
        self.steps_left > 1
    }

    #[must_use]
    pub const fn is_at_block_end(&self) -> bool {
        (self.dst & 0xF) == 0xF
    }

    #[must_use]
    pub const fn cpu_halted(&self) -> bool {
        self.cpu_halted
    }

    pub const fn set_cpu_halted(&mut self, halted: bool, mode_is_hblank: bool) {
        self.cpu_halted = halted;
        if halted {
            self.allow_on_wake = !mode_is_hblank;
        }
    }

    pub const fn note_stop(&mut self, mode_is_hblank: bool) {
        self.allow_on_wake = !mode_is_hblank;
    }

    /// Wake-up from HALT/STOP or interrupt dispatch.
    pub const fn wake(&mut self, mode_is_hblank: bool) {
        if self.on_hblank && mode_is_hblank && self.allow_on_wake {
            self.on = true;
        }
    }

    /// The PPU reached the start of `HBlank`.
    pub const fn hblank_edge(&mut self, stopped: bool) {
        if self.on_hblank && !self.cpu_halted && !stopped {
            self.on = true;
        }
    }

    /// The LCD was switched off while STAT reported a non-zero mode.
    pub const fn lcd_off_edge(&mut self) {
        if self.on_hblank {
            self.on = true;
        }
    }

    pub fn write_hdma1(&mut self, val: u8) {
        self.src = (self.src & 0xF0) | (u16::from(val) << 8);
        // Range 0xE*** acts like 0xF*** and can't overflow to anything
        // meaningful.
        if self.src >= 0xE000 {
            self.src |= 0xF000;
        }
    }

    pub const fn write_hdma2(&mut self, val: u8) {
        self.src = (self.src & 0xFF00) | (val & 0xF0) as u16;
    }

    pub fn write_hdma3(&mut self, val: u8) {
        self.dst = (self.dst & 0xF0) | (u16::from(val) << 8);
    }

    pub const fn write_hdma4(&mut self, val: u8) {
        self.dst = (self.dst & 0xFF00) | (val & 0xF0) as u16;
    }

    /// `in_hblank`: STAT mode is 0 and the PPU is not at the HBlank/OAM edge.
    pub const fn write_hdma5(&mut self, val: u8, in_hblank: bool) {
        self.steps_left = (val & 0x7F) as u16 + 1;
        if val & 0x80 == 0 && self.on_hblank {
            // Cancel the running HBlank transfer.
            self.on_hblank = false;
            return;
        }
        self.on = val & 0x80 == 0;
        self.on_hblank = val & 0x80 != 0;
        if self.on_hblank && in_hblank {
            self.on = true;
        }
    }
}

impl<A: AudioCallback> Gb<A> {
    /// Runs a pending transfer burst. Called right after an opcode fetch,
    /// while that fetch's M-cycle is still pending.
    pub fn run_hdma(&mut self) {
        let cycles = if self.key1.is_enabled() { 4 } else { 2 };

        self.hdma.in_progress = true;
        self.advance_dots(cycles);

        while self.hdma.on {
            let src = self.hdma.src;
            // Valid sources: ROM, cart RAM and WRAM. Anything else
            // (VRAM, echo RAM, OAM, I/O) yields the open bus.
            let byte = match src {
                0x0000..=0x7FFF | 0xA000..=0xDFFF => self.read_mem(src),
                _ => 0xFF,
            };
            self.hdma.src = src.wrapping_add(1);

            self.advance_dots(cycles);

            // The destination is always VRAM, written directly: the PPU's
            // access blocking does not apply, but a write during a blocked
            // phase lands in both banks.
            let addr = self.hdma.dst & 0x1FFF;
            self.hdma.dst = self.hdma.dst.wrapping_add(1);
            let mirror = self.ppu.vram_write_blocked();
            self.ppu.vram_mut().write_hdma(addr, byte, mirror);

            if self.hdma.dst & 0xF == 0 {
                self.hdma.steps_left = self.hdma.steps_left.wrapping_sub(1);
                if self.hdma.steps_left == 0 || self.hdma.dst == 0 {
                    self.hdma.on = false;
                    self.hdma.on_hblank = false;
                } else if self.hdma.on_hblank {
                    self.hdma.on = false;
                }
            }
        }

        self.hdma.in_progress = false;
        if !self.key1.is_enabled() {
            self.advance_dots(2);
        }
    }
}
