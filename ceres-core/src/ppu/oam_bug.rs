//! CPU access to OAM and the unusable area behind it, including the DMG
//! OAM corruption bug.
//!
//! Port of SameBoy's `GB_trigger_oam_bug*` and the OAM paths of its
//! `read_high_memory`/`write_high_memory`. The PPU walks OAM two bytes at a
//! time during mode 2; a CPU access (or an address placed on the bus by
//! `inc rr`, `push`, ...) to the OAM address range while that happens
//! corrupts the row being read. The CGB is not affected.

#![expect(
    clippy::many_single_char_names,
    reason = "The glitch functions name the bus values a, b, c, ... like SameBoy's"
)]

use super::Ppu;
use crate::{
    Model,
    memory::{IO_START, OAM_START, UNUSABLE_START},
};

/// `accessed_oam_row` value used when the PPU is not walking OAM.
pub(super) const NO_ROW: u8 = 0xFF;

const fn glitch(a: u16, b: u16, c: u16) -> u16 {
    ((a ^ c) & (b ^ c)) ^ c
}

const fn glitch_read(a: u16, b: u16, c: u16) -> u16 {
    b | (a & c)
}

const fn glitch_read_secondary(a: u16, b: u16, c: u16, d: u16) -> u16 {
    (b & (a | c | d)) | (a & c & d)
}

const fn glitch_tertiary_read_1(a: u16, b: u16, c: u16, d: u16, e: u16) -> u16 {
    c | (a & b & d & e)
}

const fn glitch_tertiary_read_2(a: u16, b: u16, c: u16, d: u16, e: u16) -> u16 {
    (c & (a | b | d | e)) | (a & b & d & e)
}

const fn glitch_tertiary_read_3(a: u16, b: u16, c: u16, d: u16, e: u16) -> u16 {
    (c & (a | b | d | e)) | (b & d & e)
}

type Quaternary = fn(u16, u16, u16, u16, u16, u16, u16, u16) -> u16;

/// On some DMGs some of these cases are non-deterministic; like SameBoy this
/// models the ones that read back constant zeros.
#[expect(
    clippy::too_many_arguments,
    reason = "One argument per OAM word involved"
)]
const fn glitch_quaternary_read_dmg(
    _a: u16,
    b: u16,
    c: u16,
    d: u16,
    e: u16,
    f: u16,
    g: u16,
    h: u16,
) -> u16 {
    (e & (h | g | (!d & f) | c | b)) | (c & g & h)
}

#[expect(
    clippy::too_many_arguments,
    reason = "One argument per OAM word involved"
)]
const fn glitch_quaternary_read_sgb2(
    a: u16,
    b: u16,
    c: u16,
    _d: u16,
    e: u16,
    f: u16,
    g: u16,
    h: u16,
) -> u16 {
    (e & (h | g | c | (a & b))) | ((c & g & h) & (b | a | !f))
}

impl Ppu {
    /// OAM byte; the corruption can touch the row right after OAM (row
    /// 0xA0), which is memory outside OAM (unobservable on the DMG): reads
    /// there yield 0 and writes are dropped.
    fn oam_byte(&self, index: usize) -> u8 {
        self.oam.bytes().get(index).copied().unwrap_or(0)
    }

    fn set_oam_byte(&mut self, index: usize, value: u8) {
        if let Some(byte) = self.oam.bytes_mut().get_mut(index) {
            *byte = value;
        }
    }

    fn oam_word(&self, index: usize) -> u16 {
        u16::from_le_bytes([self.oam_byte(index * 2), self.oam_byte(index * 2 + 1)])
    }

    fn set_oam_word(&mut self, index: usize, value: u16) {
        let [lo, hi] = value.to_le_bytes();
        self.set_oam_byte(index * 2, lo);
        self.set_oam_byte(index * 2 + 1, hi);
    }

    /// The row the PPU is accessing, if the bug applies to it.
    fn bug_row(&self) -> Option<usize> {
        let row = self.d.accessed_oam_row();
        (row != NO_ROW && row >= 8).then_some(usize::from(row))
    }

    /// Copies `len` bytes inside OAM.
    fn copy_oam_row(&mut self, dst: usize, src: usize) {
        for i in 0..8 {
            let byte = self.oam_byte(src + i);
            self.set_oam_byte(dst + i, byte);
        }
    }

    /// The "write" corruption: an address in OAM range was put on the bus
    /// (writes, `inc rr`, `push`, ...).
    pub fn trigger_oam_bug(&mut self, addr: u16) {
        if self.hw_cgb() || !(OAM_START..IO_START).contains(&addr) {
            return;
        }
        let Some(row) = self.bug_row() else {
            return;
        };
        let base = row / 2;
        let value = glitch(
            self.oam_word(base),
            self.oam_word(base - 4),
            self.oam_word(base - 2),
        );
        self.set_oam_word(base, value);
        for i in 2..8 {
            let byte = self.oam_byte(row - 8 + i);
            self.set_oam_byte(row + i, byte);
        }
    }

    fn oam_bug_secondary_read(&mut self, row: usize) {
        if row < 0x98 {
            let base = row / 2;
            let value = glitch_read_secondary(
                self.oam_word(base - 8),
                self.oam_word(base - 4),
                self.oam_word(base),
                self.oam_word(base - 2),
            );
            self.set_oam_word(base - 4, value);
            self.copy_oam_row(row - 0x10, row - 0x08);
        }
    }

    fn oam_bug_quaternary_read(&mut self, row: usize, op: Quaternary) {
        if row < 0x98 {
            let base = row / 2;
            let value = op(
                self.oam_word(0),
                self.oam_word(base),
                self.oam_word(base - 2),
                self.oam_word(base - 3),
                self.oam_word(base - 4),
                self.oam_word(base - 7),
                self.oam_word(base - 8),
                self.oam_word(base - 16),
            );
            self.set_oam_word(base - 4, value);
            self.copy_oam_row(row - 0x10, row - 0x08);
            self.copy_oam_row(row - 0x20, row - 0x08);
        }
    }

    fn oam_bug_tertiary_read(&mut self, row: usize, op: fn(u16, u16, u16, u16, u16) -> u16) {
        if row < 0x98 {
            let base = row / 2;
            let value = op(
                self.oam_word(base),
                self.oam_word(base - 2),
                self.oam_word(base - 4),
                self.oam_word(base - 8),
                self.oam_word(base - 16),
            );
            self.set_oam_word(base - 4, value);
            self.copy_oam_row(row - 0x10, row - 0x08);
            self.copy_oam_row(row - 0x20, row - 0x08);
        }
    }

    /// The "read" corruption: a CPU read of OAM while the PPU owns it.
    pub fn trigger_oam_bug_read(&mut self, addr: u16) {
        if self.hw_cgb() || !(OAM_START..IO_START).contains(&addr) {
            return;
        }
        let Some(row) = self.bug_row() else {
            return;
        };

        if row & 0x18 == 0x10 {
            self.oam_bug_secondary_read(row);
        } else if row & 0x18 == 0x00 {
            // Everything in this case is extremely revision and instance
            // specific.
            if self.model == Model::Mgb {
                self.oam_bug_tertiary_read(row, glitch_tertiary_read_3);
            } else if row == 0x40 {
                let op: Quaternary = if self.model == Model::Sgb2 {
                    glitch_quaternary_read_sgb2
                } else {
                    glitch_quaternary_read_dmg
                };
                self.oam_bug_quaternary_read(row, op);
            } else if self.model != Model::Sgb2 {
                let op = match row {
                    0x20 => glitch_tertiary_read_2,
                    0x60 => glitch_tertiary_read_3,
                    _ => glitch_tertiary_read_1,
                };
                self.oam_bug_tertiary_read(row, op);
            } else {
                self.oam_bug_tertiary_read(row, glitch_tertiary_read_2);
            }
        } else {
            let base = row / 2;
            let value = glitch_read(
                self.oam_word(base),
                self.oam_word(base - 4),
                self.oam_word(base - 2),
            );
            self.set_oam_word(base - 4, value);
            self.set_oam_word(base, value);
        }

        self.copy_oam_row(row, row - 8);
        if row == 0x80 || (self.model == Model::Mgb && row == 0x40) {
            self.copy_oam_row(0, row);
        }
    }

    /// Corruption caused by reading OAM during the dots where only reads are
    /// blocked (the PPU is on the first or the last row).
    fn oam_read_row_corruption(&mut self, addr: u16) {
        if addr >= UNUSABLE_START {
            return;
        }
        let low = usize::from(addr & 0xFF);
        match self.d.accessed_oam_row() {
            0 => {
                let value = glitch_read(
                    self.oam_word(0),
                    self.oam_word((low & 0xF8) >> 1),
                    self.oam_word(low >> 1),
                );
                self.oam_word_pair(0, (low & 0xF8) >> 1, value);
                let base = low & 0xF8;
                self.oam.bytes_mut().copy_within(base + 2..base + 8, 2);
            }
            0xA0 => {
                let target = (low & 7) | 0x98;
                let a = self.oam_word(0x9C >> 1);
                let b = self.oam_word(target >> 1);
                let mut c = self.oam_word((low & 0xF8) >> 1);
                match low & 7 {
                    0 | 1 => {
                        // Probably instance specific.
                        let value = if matches!(self.model, Model::DmgB | Model::Dmg0 | Model::Sgb)
                        {
                            (a & b) | (a & c) | (b & c)
                        } else {
                            glitch_read(a, b, c)
                        };
                        self.set_oam_word(target >> 1, value);
                    }
                    2 | 3 => {
                        c = self.oam_word((low & 0xFE) >> 1);
                        self.set_oam_word(target >> 1, (a & b) | (a & c) | (b & c));
                    }
                    4 | 5 => {}
                    _ => self.set_oam_word(target >> 1, glitch_read(a, b, c)),
                }
                self.oam.bytes_mut().copy_within(0x98..0xA0, low & 0xF8);
            }
            _ => {}
        }
    }

    /// `oam[a] = oam[b] = value` on 16-bit words, in C's right-to-left order.
    fn oam_word_pair(&mut self, first: usize, second: usize, value: u16) {
        self.set_oam_word(first, value);
        self.set_oam_word(second, value);
    }

    /// The model-specific contents of the unusable area behind OAM.
    pub(super) fn read_unusable(&self, addr: u16) -> u8 {
        let low = (addr & 0xFF) as u8;
        match self.model {
            Model::CgbE | Model::Agb => (low & 0xF0) | (low >> 4),
            model => unusable_index(model, low).map_or(0, |i| self.oam.extra()[i]),
        }
    }

    const fn write_unusable(&mut self, addr: u16, val: u8) {
        if let Some(i) = unusable_index(self.model, (addr & 0xFF) as u8) {
            self.oam.extra_mut()[i] = val;
        }
    }

    /// Side-effect free view of the unusable area (for non-CPU readers).
    #[must_use]
    pub fn peek_unusable(&self, addr: u16) -> u8 {
        if self.d.cpu().oam_read_blocked || self.d.cpu().oam_write_blocked && !self.hw_cgb() {
            0xFF
        } else {
            self.read_unusable(addr)
        }
    }

    /// CPU read of `0xFE00..=0xFEFF`. `dma_blocked`: an OAM DMA owns the bus.
    pub fn cpu_read_oam_area(&mut self, addr: u16, dma_blocked: bool) -> u8 {
        if self.d.cpu().oam_write_blocked && !self.hw_cgb() {
            self.trigger_oam_bug_read(addr);
            return 0xFF;
        }
        if dma_blocked {
            return 0xFF;
        }
        if self
            .gstat_oam_lock(false)
            .unwrap_or_else(|| self.d.cpu().oam_read_blocked)
        {
            if !self.hw_cgb() {
                self.oam_read_row_corruption(addr);
            }
            return 0xFF;
        }
        if addr < UNUSABLE_START {
            self.oam.read(addr)
        } else {
            self.read_unusable(addr)
        }
    }

    /// CPU write to `0xFE00..=0xFEFF`.
    pub fn cpu_write_oam_area(&mut self, addr: u16, val: u8, dma_blocked: bool) {
        if self
            .gstat_oam_lock(true)
            .unwrap_or_else(|| self.d.cpu().oam_write_blocked)
        {
            self.trigger_oam_bug(addr);
            return;
        }
        if dma_blocked {
            return;
        }
        if self.hw_cgb() {
            if addr < UNUSABLE_START {
                self.oam.write(addr, val);
            } else {
                self.write_unusable(addr, val);
            }
            return;
        }

        let low = usize::from(addr & 0xFF);
        if addr < UNUSABLE_START {
            if self.d.accessed_oam_row() == 0xA0 {
                for i in 0..8 {
                    let dst = (low & 0xF8) + i;
                    let value = if (i & 6) == (low & 6) {
                        let a = u16::from(self.oam.bytes()[dst]);
                        let b = u16::from(self.oam.bytes()[0x9C]);
                        let c = u16::from(self.oam.bytes()[0x98 + i]);
                        glitch(a, b, c).to_le_bytes()[0]
                    } else {
                        self.oam.bytes()[0x98 + i]
                    };
                    self.oam.bytes_mut()[dst] = value;
                }
            }

            self.oam.bytes_mut()[low] = val;

            if self.d.accessed_oam_row() == 0 {
                for i in 0..2 {
                    let a = u16::from(self.oam.bytes()[i]);
                    let b = u16::from(self.oam.bytes()[(low & 0xF8) + i]);
                    let c = u16::from(self.oam.bytes()[(low & 0xFE) | i]);
                    self.oam.bytes_mut()[i] = glitch(a, b, c).to_le_bytes()[0];
                }
                let base = low & 0xF8;
                self.oam.bytes_mut().copy_within(base + 2..base + 8, 2);
            }
        } else if self.d.accessed_oam_row() == 0 {
            self.oam.bytes_mut()[low & 7] = val;
        } else {
            // The unusable area: the write is dropped.
        }
    }
}

/// Where a byte of the unusable area (`low` is 0xA0..=0xFF) is kept, on the
/// revisions that keep it in memory.
pub const fn unusable_index(model: Model, low: u8) -> Option<usize> {
    let low = match model {
        Model::CgbD if low >= 0xC0 => low | 0xF0,
        Model::CgbD => low,
        Model::Cgb0 | Model::CgbA | Model::CgbB | Model::CgbC => low & !0x18,
        _ => return None,
    };
    Some(low as usize - 0xA0)
}
