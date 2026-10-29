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
//! The CPU's writes reach LYC's and the enables' latches, which follow the
//! data bus while the write strobe is up (as it opens, the bus is still
//! precharged: FF, STAT's write glitch); `reset` is the LCD turned off,
//! `read` what a read of STAT, LY or LYC drives at a moment. The core's
//! data bus is precharged and only pulled low while a read drives it, from
//! E until the read takes it in the middle of H: a read takes the AND of
//! what `read` shows in E to H. The HBlank source is render.vhd's mode 0,
//! an input. `crate::netlist_replay` checks all of it against the model's
//! traces.

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
pub(crate) struct NetLcd {
    /// The PPU's clock, as its last edge left it.
    pub(crate) clk: bool,
    /// WUVU, VENA (hclk), WOSU: the dividers.
    pub(crate) wuvu: bool,
    pub(crate) vena: bool,
    pub(crate) wosu: bool,
    /// SAXO..TYRY: LX, the M-cycle in the line.
    pub(crate) lx: u8,
    /// RUTU: the line's end (vclk).
    pub(crate) vclk: bool,
    /// NYPE: vclk, half an hclk period later.
    pub(crate) nype: bool,
    /// MUWY..LAFO: LY.
    pub(crate) ly: u8,
    /// MYTA, NOKO: line 153 (LY held at 0), the first line.
    pub(crate) line_153: bool,
    pub(crate) first_line: bool,
    /// POPU: VBlank.
    pub(crate) mode1: bool,
    /// The LCD's pins' flip-flops: the frame's and the line's polarity, CPG.
    pub(crate) frame: bool,
    pub(crate) line_parity: bool,
    pub(crate) cpg_lx: bool,
    /// LYC's latches and ROPO, LY = LYC as last sampled.
    pub(crate) lyc: u8,
    pub(crate) lyc_match: bool,
    /// STAT's enables (its bits 3 to 6).
    pub(crate) stat_enable: u8,
}

impl NetLcd {
    /// SUKO: the STAT interrupt's line, with the HBlank source `mode0`.
    pub(crate) const fn int_stat(&self, mode0: bool) -> bool {
        (self.stat_enable & STAT_IF_LYC_B != 0 && self.lyc_match)
            || (self.stat_enable & STAT_IF_OAM_B != 0 && self.vclk && !self.mode1)
            || (self.stat_enable & STAT_IF_VBLANK_B != 0 && self.mode1)
            || (self.stat_enable & STAT_IF_HBLANK_B != 0 && mode0)
    }

    /// The next edge of the PPU's clock: whether the STAT interrupt's line
    /// rose in it.
    pub(crate) const fn edge(&mut self, mode0: bool) -> bool {
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

    /// The LCD's reset (`ppu_reset`, LCDC bit 7 clear) holds the clock's
    /// flip-flops at 0; LYC, the enables and LY = LYC's flag keep their
    /// values (only the chip's reset clears those).
    pub(crate) const fn reset(&mut self) {
        self.wuvu = false;
        self.vena = false;
        self.wosu = false;
        self.lx = 0;
        self.vclk = false;
        self.nype = false;
        self.ly = 0;
        self.line_153 = false;
        self.first_line = false;
        self.mode1 = false;
        self.frame = false;
        self.line_parity = false;
        self.cpg_lx = false;
    }

    /// What a read of STAT, LY or LYC drives, with the scan's and the
    /// pixels' modes (`mode2`, `mode3`, from `oam_scan.vhd` and `render.vhd`).
    /// STAT's bit 7 is not driven: the bus keeper holds it high.
    // @hw die: SADU TEBY, XATY WUGA, RUPO SEGO; PUZO POFO SASY POTE;
    // VEGA..WEZE; RETU..PUFY
    pub(crate) const fn read(&self, addr: u16, mode2: bool, mode3: bool) -> Option<u8> {
        match addr {
            0xFF41 => Some(
                0x80 | self.stat_enable
                    | if self.lyc_match { 4 } else { 0 }
                    | if mode2 || mode3 { 2 } else { 0 }
                    | if mode3 || self.mode1 { 1 } else { 0 },
            ),
            0xFF44 => Some(self.ly),
            0xFF45 => Some(self.lyc),
            _ => None,
        }
    }

    /// The line after a stage of an edge: a rise counts.
    const fn stage(now: bool, level: &mut bool, rose: &mut bool) {
        *rose |= now && !*level;
        *level = now;
    }
}
