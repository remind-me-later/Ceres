//! The netlist's line timing, transcribed from the DMG's part of
//! `hdl/rtl/lcd_timing.vhd` edge by edge, for the proofs against ceres
//! (`netlist_proofs.rs`).
//!
//! The PPU's clock is the chip's, a dot a period: an edge is a half dot. Its
//! falling edges clock `wuvu` and `vena`, which halve it twice into hclk (an
//! M-cycle); its rising edges clock `wosu`. The flip-flops clocked by hclk,
//! and those clocked by them, change a flip-flop's delay after one another
//! within the edge: `stage` runs them in that order, and the STAT
//! interrupt's line (`int_stat`, the OR of its sources) is looked at after
//! each, since IF takes any of its rises, even one that falls again later
//! in the edge.
//!
//! Left out for now: the CPU's reads and writes (LY, LYC, STAT), the LCD's
//! being turned on and off (`ppu_reset`), and the HBlank source, which is
//! render.vhd's mode 0.

// Some of it only the proofs use, which other builds leave out.
#![cfg_attr(not(kani), allow(dead_code))]
#![expect(
    clippy::field_scoped_visibility_modifiers,
    reason = "the proofs set and compare the flip-flops one by one"
)]

use crate::ppu::{STAT_IF_HBLANK_B, STAT_IF_LYC_B, STAT_IF_OAM_B, STAT_IF_VBLANK_B};

/// The netlist's line timing, between edges of the PPU's clock.
#[derive(Clone, Copy, PartialEq, Eq)]
#[expect(clippy::struct_excessive_bools, reason = "the netlist's flip-flops")]
pub(super) struct NetLcd {
    /// The PPU's clock, as its last edge left it.
    pub(super) clk: bool,
    /// WUVU, VENA (hclk), WOSU: the dividers.
    pub(super) wuvu: bool,
    pub(super) vena: bool,
    pub(super) wosu: bool,
    /// SAXO..TYRY: LX, the M-cycle in the line.
    pub(super) lx: u8,
    /// RUTU: the line's end (vclk).
    pub(super) vclk: bool,
    /// NYPE: vclk, half an hclk period later.
    pub(super) nype: bool,
    /// MUWY..LAFO: LY.
    pub(super) ly: u8,
    /// MYTA, NOKO: line 153 (LY held at 0), the first line.
    pub(super) line_153: bool,
    pub(super) first_line: bool,
    /// POPU: VBlank.
    pub(super) mode1: bool,
    /// The LCD's pins' flip-flops: the frame's and the line's polarity, CPG.
    pub(super) frame: bool,
    pub(super) line_parity: bool,
    pub(super) cpg_lx: bool,
    /// LYC's latches and ROPO, LY = LYC as last sampled.
    pub(super) lyc: u8,
    pub(super) lyc_match: bool,
    /// STAT's enables (its bits 3 to 6).
    pub(super) stat_enable: u8,
}

impl NetLcd {
    /// SUKO: the STAT interrupt's line, with the HBlank source `mode0`.
    pub(super) const fn int_stat(&self, mode0: bool) -> bool {
        (self.stat_enable & STAT_IF_LYC_B != 0 && self.lyc_match)
            || (self.stat_enable & STAT_IF_OAM_B != 0 && self.vclk && !self.mode1)
            || (self.stat_enable & STAT_IF_VBLANK_B != 0 && self.mode1)
            || (self.stat_enable & STAT_IF_HBLANK_B != 0 && mode0)
    }

    /// The next edge of the PPU's clock: whether the STAT interrupt's line
    /// rose in it.
    pub(super) const fn edge(&mut self, mode0: bool) -> bool {
        let mut level = self.int_stat(mode0);
        let mut rose = false;
        self.clk = !self.clk;
        if self.clk {
            // @hw die: WOSU (dffr), XYFY
            self.wosu = !self.wuvu;
            return false;
        }

        // The falling edge: the dividers.
        // @hw die: WUVU VENA (dffr), XYVA XOTA, TALU
        let old_vena = self.vena;
        if self.wuvu {
            self.vena = !self.vena;
        }
        self.wuvu = !self.wuvu;

        // hclk's edge: what it clocks.
        let hclk_rose = !old_vena && self.vena;
        let hclk_fell = old_vena && !self.vena;
        let (old_vclk, old_nype) = (self.vclk, self.nype);
        if hclk_rose {
            // @hw die: SAXO..TYRY (dffr); NYPE (dffr); ROPO (dffr)
            if !self.vclk {
                self.lx += 1;
            }
            self.nype = self.vclk;
            self.lyc_match = self.ly == self.lyc;
        }
        if hclk_fell {
            // @hw die: RUTU (dffr), SANU; TEGY (dffr)
            self.vclk = self.lx == 113;
            self.cpg_lx = matches!(self.lx, 0 | 7 | 45 | 83);
        }
        Self::stage(self.int_stat(mode0), &mut level, &mut rose);

        // What vclk and nype clock.
        let old_mode1 = self.mode1;
        if self.vclk {
            // vclk holds LX at 0.
            self.lx = 0;
        }
        if !old_vclk && self.vclk && !self.line_153 {
            // @hw die: MUWY..LAFO (dffr)
            self.ly = self.ly.wrapping_add(1);
        }
        if old_vclk && !self.vclk {
            self.line_parity = !self.line_parity;
        }
        if !old_nype && self.nype {
            // @hw die: XYVO, POPU (dffr)
            self.mode1 = self.ly & 0x90 == 0x90;
        }
        if old_nype && !self.nype {
            // @hw die: NOKO MYTA (dffr)
            self.line_153 = self.ly == 153;
            self.first_line = self.ly == 0;
        }
        Self::stage(self.int_stat(mode0), &mut level, &mut rose);

        // What mode 1 and line 153 clock and reset.
        if !old_mode1 && self.mode1 {
            self.frame = !self.frame;
        }
        if self.line_153 {
            // @hw die: LAMA: LY's reset
            self.ly = 0;
        }
        Self::stage(self.int_stat(mode0), &mut level, &mut rose);
        rose
    }

    /// The line after a stage of an edge: a rise counts.
    const fn stage(now: bool, level: &mut bool, rose: &mut bool) {
        *rose |= now && !*level;
        *level = now;
    }
}
