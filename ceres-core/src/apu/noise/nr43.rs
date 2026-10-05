//! Changing the clock of the noise channel (NR43) while it runs: the counter
//! bit that steps the LFSR changes, which can step it, sometimes glitching
//! it in revision specific ways.

use super::{super::Ctx, Noise};
use crate::apu::revision::Revision;

impl Noise {
    /// Sets NR43 to `new`, stepping or glitching the LFSR when the selected
    /// counter bit changes (SameBoy's `nr43_write`).
    pub(super) fn switch_clock(&mut self, new: u8, c: &Ctx) {
        let rev = c.rev;
        let old_narrow = self.narrow;
        self.narrow = new & 8 != 0;
        let old = self.nr43;
        self.nr43 = new;

        if old & 0xF0 == new & 0xF0 {
            return;
        }

        let mut effective_counter = self.counter;
        if rev <= Revision::CgbC && self.countdown_reloaded {
            effective_counter |= effective_counter.wrapping_sub(1) & 0x3FFF;
        }
        let old_bit = Self::counter_bit(effective_counter, old);

        let mut glitch_value = (old & 0x7F) | (new & 0x80);
        let mut glitch_bit = Self::counter_bit(effective_counter, glitch_value);
        let new_bit = Self::counter_bit(effective_counter, new);
        let mut force_glitch =
            rev == Revision::CgbD && new_bit && glitch_bit && old_bit && (old ^ new) & 0x70 != 0;

        if rev.is_agb() {
            // AGB behaviour is very glitchy and inconsistent; this is a very
            // rough approximation.
            let glitch_value2 = if new >= 0x80 && old >= 0x80 {
                glitch_value = (old & 0xCF) | (new & 0x30);
                (old & 0x8F) | (new & 0x70)
            } else {
                glitch_value = (old & 0xDF) | (new & 0x20);
                (old & 0xCF) | (new & 0x30)
            };
            glitch_bit = Self::counter_bit(self.counter, glitch_value);
            let glitch_bit2 = Self::counter_bit(self.counter, glitch_value2);
            if glitch_bit != glitch_bit2 {
                if new_bit == old_bit {
                    glitch_bit = !new_bit;
                } else if !glitch_bit && old_bit {
                    force_glitch = true;
                }
            }
        }

        if (old_bit == new_bit && new_bit != glitch_bit) || force_glitch {
            // Glitching write, in two categories (both have non-deterministic
            // variants; these are the most common, deterministic ones).
            if new_bit {
                // Category 1.
                if rev >= Revision::CgbE {
                    if new & 0x80 == 0 {
                        self.step_lfsr(c);
                    } else {
                        self.category_1_cgb_e(old, new, old_narrow, c);
                    }
                } else if rev == Revision::CgbD {
                    self.category_1_cgb_d(old, new, force_glitch, c);
                }
            } else if rev >= Revision::CgbE {
                // Category 2.
                self.category_2_cgb_e(old, new, c);
            } else {
                self.step_lfsr(c);
            }
        } else if !old_bit && new_bit {
            if rev <= Revision::CgbC {
                let previous_narrow = self.narrow;
                self.narrow = true;
                self.step_lfsr(c);
                self.narrow = previous_narrow;
                if (new & 0xF0) <= 0x20 && glitch_bit && effective_counter & 8 == 0 {
                    // Non-deterministic, not fully tested for revision differences.
                    self.step_lfsr(c);
                    self.lfsr &= !self.high_bit_mask();
                    self.lfsr |= (self.lfsr & (self.high_bit_mask() >> 1)) << 1;
                }
            } else {
                self.step_lfsr(c);
            }
        } else if rev <= Revision::CgbC
            && (new & 0xF0) <= 0x20
            && !glitch_bit
            && !new_bit
            && !old_bit
            && effective_counter & 8 != 0
        {
            // Step twice?
            self.step_lfsr(c);
        }
    }

    /// Category 1 on the CGB-E (and AGB) with NR43 bit 7 set: only happens
    /// under an odd condition.
    fn category_1_cgb_e(&mut self, old: u8, new: u8, old_narrow: bool, c: &Ctx) {
        let t1 = (old >> 4) & 7;
        let t2 = (new >> 4) & 7;
        if u32::from(t1 ^ 7) + u32::from(t2) <= 7 && (t1 ^ 7) & t2 == 0 {
            return;
        }
        // Copy bit 8 to bit 7.
        self.lfsr &= !0x80;
        self.lfsr |= (self.lfsr >> 1) & 0x80;

        // All specific cases have non-deterministic behaviours.
        if (t1 == 0 || t1 == 4) && t2 == 3 {
            self.lfsr &= (self.lfsr >> 1) | 0x545;
            self.update_lfsr(c);
        } else if t1 == 2 && t2 == 3 {
            let mut mask: u16 = 0x555;
            if self.lfsr & 0xC == 0xC {
                mask |= 8;
            }
            if self.lfsr & 0xC00 == 0xC00 {
                mask |= 0x800;
            }
            self.lfsr &= (self.lfsr >> 1) | mask;
            self.update_lfsr(c);
        }
        if !self.narrow && old_narrow && self.stepped_in_narrow {
            if self.bit_7_before_step {
                self.lfsr |= 0x40;
            } else {
                self.lfsr &= !0x40;
            }
        }
        self.lfsr |= self.high_bit_mask();
        self.stepped_in_narrow = self.narrow;
    }

    #[expect(
        clippy::too_many_lines,
        reason = "Two lookup tables and a switch with fall-through, as in SameBoy"
    )]
    fn category_1_cgb_d(&mut self, old: u8, new: u8, force_glitch: bool, c: &Ctx) {
        const GLITCH_MAP_L2H: [u8; 64] = {
            let mut m = [0_u8; 64];
            let rows: [[u8; 6]; 8] = [
                [0x00, 0x01, 0x01, 0x21, 0x02, 0x21],
                [0x03, 0x00, 0x21, 0x01, 0x04, 0x04],
                [0x05, 0x01, 0x00, 0x01, 0x04, 0x21],
                [0x03, 0x05, 0x05, 0x00, 0x01, 0x01],
                [0x05, 0x01, 0x01, 0x21, 0x00, 0x01],
                [0x05, 0x05, 0x21, 0x01, 0x05, 0x00],
                [0x05, 0x01, 0x05, 0x01, 0x05, 0x01],
                [0x03, 0x05, 0x05, 0x05, 0x05, 0x05],
            ];
            let mut r = 0;
            while r < 8 {
                let mut c = 0;
                while c < 6 {
                    m[r * 8 + c] = rows[r][c];
                    c += 1;
                }
                r += 1;
            }
            m
        };
        const GLITCH_MAP_H2L: [u8; 64] = {
            let mut m = [0_u8; 64];
            let rows: [[u8; 8]; 6] = [
                [0x00, 0x27, 0x26, 0x37, 0x21, 0x38, 0x01, 0x01],
                [0x01, 0x00, 0x38, 0x21, 0x21, 0x21, 0x01, 0x01],
                [0x01, 0x27, 0x00, 0x28, 0x21, 0x38, 0x01, 0x01],
                [0x01, 0x02, 0x01, 0x00, 0x31, 0x21, 0x01, 0x01],
                [0x06, 0x28, 0x28, 0x38, 0x00, 0x27, 0x01, 0x01],
                [0x01, 0x03, 0x38, 0x21, 0x01, 0x00, 0x01, 0x01],
            ];
            let mut r = 0;
            while r < 6 {
                let mut c = 0;
                while c < 8 {
                    m[r * 8 + c] = rows[r][c];
                    c += 1;
                }
                r += 1;
            }
            m
        };

        let map = if old & 0x80 != 0 {
            &GLITCH_MAP_H2L
        } else {
            &GLITCH_MAP_L2H
        };
        let mut glitch = u32::from(map[usize::from(((old & 0x70) >> 1) | ((new & 0x70) >> 4))]);
        if force_glitch {
            if (new ^ old) & 0x80 == 0 {
                glitch = if glitch & 0x20 != 0 { 5 } else { 0 };
            } else if new & 0x80 == 0 {
                glitch = if glitch & 0x10 != 0 { 5 } else { 0 };
            } else if glitch & 0xF == 1 || glitch & 0xF == 4 {
                glitch = 5;
            } else {
                glitch = 0;
            }
        } else {
            glitch &= 0xF;
        }
        let old_lfsr = self.lfsr;
        let lfsr_mask = self.high_bit_mask();

        // Emulates the C `switch` with its deliberate fall-through chain:
        // 6/4 -> 2 -> 1/8 -> 5.
        let mut stage = match glitch {
            6 | 4 => 6,
            2 => 5,
            1 | 8 => 4,
            5 => 3,
            7 => 100,
            3 => 101,
            _ => 0,
        };
        if stage == 6 {
            let probe = if glitch == 4 { 0x60 } else { 0x40 };
            if self.lfsr & probe == 0x40 {
                stage = 4;
            } else {
                stage = 5;
            }
        }
        if stage == 5 {
            if self.lfsr & 1 == 0 {
                self.lfsr &= !2;
            }
            stage = 4;
        }
        if stage == 4 {
            self.step_lfsr(c);
            stage = 3;
        }
        if stage == 3 {
            if glitch != 8 || old_lfsr & 3 != 2 {
                self.lfsr |= lfsr_mask;
            } else {
                self.lfsr |= old_lfsr & lfsr_mask;
            }
        } else if stage == 100 {
            self.step_lfsr(c);
            self.lfsr |= old_lfsr & lfsr_mask;
        } else if stage == 101 {
            self.step_lfsr(c);
            self.lfsr &= old_lfsr;
            self.lfsr |= old_lfsr & 1;
            self.lfsr |= lfsr_mask;
            self.update_lfsr(c);
        }
    }

    fn category_2_cgb_e(&mut self, old: u8, new: u8, c: &Ctx) {
        const GLITCH_MAP: [u8; 64] = {
            let mut m = [0_u8; 64];
            // Indexed by (old & 0x70) >> 1 | (new & 0x70) >> 4, octal in the C source.
            m[0o02] = 4;
            m[0o03] = 2;
            m[0o04] = 2;
            m[0o05] = 2;
            m[0o12] = 2;
            m[0o13] = 4;
            m[0o14] = 2;
            m[0o15] = 2;
            m[0o20] = 1;
            m[0o21] = 2;
            m[0o23] = 1;
            m[0o24] = 5;
            m[0o25] = 3;
            m[0o34] = 2;
            m[0o35] = 2;
            m[0o41] = 2;
            m[0o42] = 2;
            m[0o43] = 2;
            m[0o50] = 6;
            m[0o52] = 2;
            m[0o53] = 2;
            m
        };

        let glitch = if new & 0x80 != 0 {
            GLITCH_MAP[usize::from(((old & 0x70) >> 1) | ((new & 0x70) >> 4))]
        } else {
            0
        };
        match glitch {
            // Step, followed by bit 1 &= bit 0 (6: a variant).
            1 | 6 => {
                self.step_lfsr(c);
                if glitch == 6 {
                    if (self.narrow && self.lfsr & 0x71 == 0x20) || self.lfsr & 0x71 == 0x61 {
                        self.lfsr &= !0x20;
                    }
                    if self.lfsr & 0x7001 == 0x2000 || self.lfsr & 0x7001 == 0x6001 {
                        self.lfsr &= !0x2000;
                    }
                }
                if self.lfsr & 0x3 == 2 {
                    self.lfsr &= !2;
                }
            }
            // Step, bitwise AND with the previous value, except for bit 0.
            2 => {
                let prev = self.lfsr;
                self.step_lfsr(c);
                self.lfsr &= prev | 1;
            }
            // 5: non-deterministic variant of 3 (falls through into it).
            3 | 5 => {
                if glitch == 5 {
                    if self.lfsr & 0x3 == 2 {
                        self.lfsr &= !self.high_bit_mask();
                    }
                    if self.lfsr & 0x19 == 8 {
                        self.lfsr &= !8;
                    }
                }
                // No step, bit 0 = bit 1.
                self.lfsr &= !1;
                self.lfsr |= (self.lfsr >> 1) & 1;
                self.update_lfsr(c);
                self.stepped_in_narrow = self.narrow;
            }
            // Step, bit 1 &= bit 0, LFSR bit -1 &= LFSR bit.
            4 => {
                let prev = self.lfsr;
                self.step_lfsr(c);
                self.lfsr &= prev | if self.narrow { !0x2022 } else { !0x2002 };
            }
            _ => self.step_lfsr(c),
        }
    }
}
