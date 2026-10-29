//! Kani proofs that ceres's line timing is the netlist's, on the DMG
//! (`cargo kani -p ceres-core --harness netlist_lcd`).
//!
//! `NetLcd` (`netlist.rs`) transcribes `hdl/rtl/lcd_timing.vhd` edge by
//! edge; ceres's `LcdTiming::unit` advances a half dot, which is an edge.
//! Ceres's half dot `h` names the netlist's dividers: hclk rises at
//! `h = 4` (mod 8), where LY = LYC is sampled and nype follows vclk, and
//! falls at `h = 0` (mod 8), where vclk follows LX; the line's end (vclk,
//! ceres's `line_end`) rises at 904, so that LY changes there and VBlank
//! at 908. The proof is a bisimulation: from any pair of states that
//! `related` matches, a half dot raises the STAT interrupt's line on both
//! sides or on neither, and leaves them related.
//!
//! What it does not cover yet: the CPU's reads and writes, the LCD being
//! turned on and off, the HBlank source (render.vhd's mode 0: its enable is
//! held off here), and the resync of ceres's clock by the pixel pipeline at
//! each line's end (`Ppu::timing_line_end`).

use super::{
    DMG_PHASES, LAST_LINE, LAST_VISIBLE_LINE, LINE_END_H, LINE_H, LcdTiming, STAT_ENABLES,
    VBLANK_H, netlist::NetLcd,
};
use crate::ppu::STAT_IF_HBLANK_B;

/// LY as the comparator sees it, at half dot `h` of `line`: the next line's
/// from the line's end, 0 through line 153 from its fourth half dot.
const fn ly_at(line: u8, h: i32) -> u8 {
    if h >= LINE_END_H {
        if line == LAST_LINE { 0 } else { line + 1 }
    } else if line == LAST_LINE && h >= 4 {
        0
    } else {
        line
    }
}

/// VBlank (POPU), at half dot `h` of `line`.
const fn vblank_at(line: u8, h: i32) -> bool {
    match line {
        LAST_VISIBLE_LINE => h >= VBLANK_H,
        LAST_LINE => h < VBLANK_H,
        _ => line > LAST_VISIBLE_LINE,
    }
}

/// A DMG's line timing with the LCD on, at any half dot of any line, with
/// any enables but HBlank's, any LYC and any LY = LYC flag: the states that
/// a running LCD with no access in flight can be in.
fn any_ceres() -> LcdTiming {
    let line: u8 = kani::any();
    let h: i32 = kani::any();
    kani::assume(line <= LAST_LINE && (0..LINE_H).contains(&h));
    let enables = kani::any::<u8>() & STAT_ENABLES & !STAT_IF_HBLANK_B;
    let mut t = LcdTiming {
        phases: DMG_PHASES,
        on: true,
        first_line: kani::any(),
        line,
        h,
        ly: ly_at(line, h),
        line_end: h >= LINE_END_H,
        vblank: vblank_at(line, h),
        hblank: kani::any(),
        hblank_pending: false,
        ly_match: kani::any(),
        enables,
        lyc: kani::any(),
        stat_line: false,
        lyc_pending: None,
        stat_pending: None,
        dirty: false,
    };
    t.stat_line = t.line_for(t.enables);
    t
}

/// The netlist's flip-flops for ceres's state; the LCD's pins' flip-flops,
/// which nothing here reads, are left free.
fn net_for(t: &LcdTiming) -> NetLcd {
    let (line, h) = (t.line, t.h);
    let phase = h & 7;
    NetLcd {
        clk: h & 1 != 0,
        wuvu: h & 2 != 0,
        vena: phase >= 4,
        wosu: (h + 1) & 2 != 0,
        lx: if (4..LINE_END_H).contains(&h) {
            ((h - 4) / 8 + 1) as u8
        } else {
            0
        },
        vclk: h >= LINE_END_H,
        nype: h >= VBLANK_H || h < 4,
        ly: t.ly,
        line_153: (line == LAST_LINE && h >= 4) || (line == 0 && h < 4),
        first_line: (line == 0 && h >= 4) || (line == 1 && h < 4),
        mode1: t.vblank,
        frame: kani::any(),
        line_parity: kani::any(),
        cpg_lx: kani::any(),
        lyc: t.lyc,
        lyc_match: t.ly_match,
        stat_enable: t.enables,
    }
}

/// The states match: the netlist's flip-flops are what ceres's half dot
/// says, and ceres's line, LY, VBlank and interrupt line are consistent.
fn related(net: &NetLcd, t: &LcdTiming) -> bool {
    let want = net_for(t);
    t.on && !t.dirty
        && t.lyc_pending.is_none()
        && t.stat_pending.is_none()
        && t.ly == ly_at(t.line, t.h)
        && t.line_end == (t.h >= LINE_END_H)
        && t.vblank == vblank_at(t.line, t.h)
        && t.stat_line == t.line_for(t.enables)
        && net.clk == want.clk
        && net.wuvu == want.wuvu
        && net.vena == want.vena
        && net.wosu == want.wosu
        && net.lx == want.lx
        && net.vclk == want.vclk
        && net.nype == want.nype
        && net.ly == want.ly
        && net.line_153 == want.line_153
        && net.first_line == want.first_line
        && net.mode1 == want.mode1
        && net.lyc == want.lyc
        && net.lyc_match == want.lyc_match
        && net.stat_enable == want.stat_enable
}

/// A half dot of a running LCD: the same rise of the STAT interrupt's line
/// on both sides, and related states after it.
#[kani::proof]
fn netlist_lcd_half_dot() {
    let mut t = any_ceres();
    let mut net = net_for(&t);
    kani::assume(related(&net, &t));
    let ceres_rose = t.unit();
    let net_rose = net.edge(false);
    assert!(
        ceres_rose == net_rose,
        "the interrupt's line rises on one side only"
    );
    assert!(related(&net, &t), "the states part");
    kani::cover!(
        ceres_rose && t.h == VBLANK_H,
        "the line rises as VBlank starts"
    );
    kani::cover!(
        ceres_rose && t.h == LINE_END_H,
        "the line rises at the line's end"
    );
    kani::cover!(
        ceres_rose && t.h & 7 == 4 && t.h != VBLANK_H,
        "LY = LYC raises the line"
    );
}
