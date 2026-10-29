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
//! The CPU's phases are the clock's edges: D is `h = 0` (mod 8), E is
//! `h = 1`, H `h = 4`. Ceres makes its access after the half dot at
//! `h = 0` (from the end of the first line after the LCD is turned on, in
//! which its clock runs 2 half dots off until the pixel pipeline resyncs
//! it) and then runs its M-cycle's eight half dots; in the netlist the
//! write's latches open in E (taking the precharged bus first: STAT's
//! write glitch), and a read takes the core's data bus in the middle of H.
//! That bus is precharged and only pulled low while the read drives it,
//! from E on: a read gives the AND of what the register showed in E to H,
//! so that a bit rising at H's edge is not seen and one falling is. The
//! rises at ceres's access and in its first half dot are the netlist's in
//! E. `netlist_lcd_access` proves an M-cycle with any access to STAT, LY or
//! LYC alike on both sides: the byte a read gives (STAT's flag and enables;
//! its mode bits are the scan's and the pixels', not this block's), and the
//! T-cycle of each rise, as IF sees it (`interrupts/netlist_proofs.rs`'s
//! mapping). By the half dot, an LYC write raises ceres's line one early,
//! within the same T-cycle: ceres compares the new LYC as it lands, the
//! netlist at its next sample. Where the trace replay (`crate::netlist_replay`)
//! measured it: the access mapping and the reads' AND match the model's
//! traces of 169 gbmicrotest ROMs.
//!
//! What it does not cover yet: STAT's mode bits, the LCD being turned on
//! and off, the HBlank source (render.vhd's mode 0: its enable is held off
//! here), and the resync of ceres's clock by the pixel pipeline at each
//! line's end (`Ppu::timing_line_end`).

use super::{
    DMG_DELAYS, DMG_PHASES, LAST_LINE, LAST_VISIBLE_LINE, LINE_END_H, LINE_H, LcdTiming,
    STAT_ENABLES, VBLANK_H, netlist::NetLcd,
};
use crate::ppu::{STAT_IF_HBLANK_B, STAT_LYC_B};

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

/// A CPU access to the line timing's registers.
#[derive(Clone, Copy)]
enum Access {
    None,
    Read(u16),
    Write(u16, u8),
}

/// Any access to STAT, LY or LYC (a write to LY is not this block's), with
/// HBlank's enable held off.
fn any_access() -> Access {
    let addr = match kani::any::<u8>() % 3 {
        0 => 0xFF41,
        1 => 0xFF44,
        _ => 0xFF45,
    };
    match kani::any::<u8>() % 3 {
        0 => Access::None,
        1 => Access::Read(addr),
        _ => {
            let data: u8 = kani::any();
            kani::assume(addr != 0xFF44 && (addr != 0xFF41 || data & STAT_IF_HBLANK_B == 0));
            Access::Write(addr, data)
        }
    }
}

/// STAT's bits that this block drives: the LY = LYC flag and the enables.
const STAT_OWN: u8 = STAT_ENABLES | STAT_LYC_B;

/// Ceres's M-cycle: the access, then eight half dots. What a read gives,
/// and the rises of the interrupt's line in each half dot (one at the
/// access counts in the first).
fn ceres_m_cycle(t: &mut LcdTiming, access: Access) -> (Option<u8>, [bool; 8]) {
    let mut at_access = false;
    let read = match access {
        Access::None => None,
        Access::Read(0xFF41) => Some(t.read_stat(t.enables, DMG_DELAYS, false) & STAT_OWN),
        Access::Read(0xFF44) => Some(t.read_ly(DMG_DELAYS)),
        Access::Read(_) => Some(t.lyc),
        Access::Write(0xFF41, data) => {
            at_access = t.write_stat(data, DMG_DELAYS);
            None
        }
        Access::Write(_, data) => {
            t.write_lyc(data, DMG_DELAYS);
            None
        }
    };
    let mut rose = [false; 8];
    for slot in &mut rose {
        *slot = t.unit();
    }
    rose[0] |= at_access;
    (read, rose)
}

/// The netlist's M-cycle from D's edge: E's edge, where a write opens the
/// latch, which takes the precharged bus (FF) and then the byte; then the
/// edges F to D. A read gives the AND of what the register shows after E's
/// edge to H's.
fn net_m_cycle(net: &mut NetLcd, access: Access) -> (Option<u8>, [bool; 8]) {
    let mut rose = [false; 8];
    let mut bus: Option<u8> = None;
    for (k, slot) in rose.iter_mut().enumerate() {
        *slot = net.edge(false);
        if k == 0 {
            if let Access::Write(addr, data) = access {
                let mut level = net.int_stat(false);
                for byte in [0xFF, data] {
                    if addr == 0xFF41 {
                        net.stat_enable = byte & STAT_ENABLES;
                    } else {
                        net.lyc = byte;
                    }
                    let now = net.int_stat(false);
                    *slot |= now && !level;
                    level = now;
                }
            }
        }
        if k <= 3 {
            // E to H: the bus that the read drives.
            if let Access::Read(addr) = access {
                let shown = net
                    .read(addr, false, false)
                    .map(|b| if addr == 0xFF41 { b & STAT_OWN } else { b });
                bus = if k == 0 {
                    shown
                } else {
                    bus.zip(shown).map(|(a, b)| a & b)
                };
            }
        }
    }
    (bus, rose)
}

/// An M-cycle with any access to STAT, LY or LYC: the same byte read, the
/// same rises of the interrupt's line in the same half dots, and related
/// states after it.
#[kani::proof]
#[kani::unwind(10)]
fn netlist_lcd_access() {
    let mut t = any_ceres();
    kani::assume(t.h & 7 == 0);
    // STAT's write glitch enables HBlank's source for a moment: until it
    // joins the proof (render.vhd's mode 0), it is low on both sides.
    t.hblank = false;
    let mut net = net_for(&t);
    kani::assume(related(&net, &t));
    let access = any_access();
    let (ceres_read, ceres_rose) = ceres_m_cycle(&mut t, access);
    let (net_read, net_rose) = net_m_cycle(&mut net, access);
    assert!(ceres_read == net_read, "a read differs");
    // IF sees the rises by T-cycle, as `interrupts/netlist_proofs.rs` maps
    // them: ceres's first T-cycle (the access and its first two half dots)
    // is the netlist's E to G, its second H, its last two A to D.
    let ceres_by_t = [
        ceres_rose[0] || ceres_rose[1],
        ceres_rose[2] || ceres_rose[3],
        ceres_rose[4..].contains(&true),
    ];
    let net_by_t = [
        net_rose[0] || net_rose[1] || net_rose[2],
        net_rose[3],
        net_rose[4..].contains(&true),
    ];
    assert!(
        ceres_by_t == net_by_t,
        "the interrupt's line rises in another T-cycle"
    );
    // Within the second, an LYC write raises ceres's line a half dot before
    // the netlist's sample at H: ceres's comparison takes the new LYC at
    // once.
    kani::cover!(
        ceres_rose[2] && !net_rose[2] && net_rose[3],
        "an LYC write raises ceres's line a half dot early"
    );
    assert!(related(&net, &t), "the states part");
    kani::cover!(
        matches!(access, Access::Write(0xFF41, _)) && ceres_rose[0],
        "a STAT write's glitch raises the line"
    );
    kani::cover!(
        matches!(access, Access::Read(0xFF41)) && ceres_read.is_some_and(|b| b & STAT_LYC_B != 0),
        "a STAT read sees LY = LYC"
    );
}
