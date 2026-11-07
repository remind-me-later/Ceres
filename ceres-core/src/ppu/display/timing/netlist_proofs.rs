//! Kani proofs that ceres's line timing is the netlist's, on the DMG
//! (`cargo kani -p ceres-core --harness netlist_lcd`).
//!
//! `NetLcd` (`ceres_netlist::lcd_timing`) transcribes `hdl/rtl/lcd_timing.vhd`
//! edge by edge; ceres's `LcdTiming::unit` advances a half dot, which is an
//! edge. Ceres's half dot `h` names the netlist's dividers: hclk rises at `h =
//! 4` (mod 8), where LY = LYC is sampled and nype follows vclk, and falls at `h
//! = 0` (mod 8), where vclk follows LX; the line's end (vclk, ceres's
//! `line_end`) rises at 904, so that LY changes there and VBlank at 908. The
//! proof is a bisimulation: from any pair of states that `related` matches, a
//! half dot raises the STAT interrupt's line on both sides or on neither, and
//! leaves them related.
//!
//! The CPU's phases are the clock's edges: D is `h = 0` (mod 8), E is `h = 1`,
//! H `h = 4`. Ceres makes its access after the half dot at `h = 0` (at
//! `h = 2` in the first line after the LCD is turned on, in which its clock
//! runs 2 half dots ahead until the pixel pipeline resyncs it) and then runs its
//! M-cycle's eight half dots; in the netlist the write's latches open in E
//! (taking the precharged bus first: STAT's write glitch), and a read takes the
//! core's data bus in the middle of H. That bus is precharged and only pulled
//! low while the read drives it, from E on: a read gives the AND of what the
//! register showed in E to H, so that a bit rising at H's edge is not seen and
//! one falling is. The rises at ceres's access and in its first half dot are
//! the netlist's in E. `netlist_lcd_access` proves an M-cycle with any access
//! to STAT, LY, LYC or LCDC alike on both sides, from every related pair, every
//! field of both states free (`check::step_agrees`, which Lean wires into
//! IF, `lean/`): the byte a read gives (STAT's flag and enables; its mode
//! bits are the scan's and the pixels', not this block's), and the T-cycle
//! of each rise, as IF sees it (`interrupts/netlist_proofs.rs`'s mapping).
//! By the half dot, an LYC write
//! raises ceres's line one early, within the same T-cycle: ceres compares the
//! new LYC as it lands, the netlist at its next sample. Where the trace replay
//! (ceres-netlist's `tests/replay.rs`) measured it: the access mapping and the
//! reads' AND match the model's traces of 169 gbmicrotest ROMs.
//!
//! The LCD turned off and on: ceres's LCDC write lands 2 half dots before
//! its access (SameBoy's `cycle_write`), the netlist's latch opens in E.
//! Turning it off, ceres runs 3 half dots before its reset, to E; turning
//! it on, ceres's clock runs 2 half dots ahead through line 0, as the
//! netlist's line began at D, until the pixel pipeline resyncs it.
//! `netlist_lcd_access` proves both, and the M-cycles with the LCD off and
//! in that line 0, but for one case: with LYC 0, its enable on and the
//! flag clear, ceres raises the line in the M-cycle before the write's, at
//! once, the netlist at H, after its first sample. `netlist_lcd_on` proves
//! that this is the only difference as the LCD is turned on. The CPU
//! cannot see it: in that M-cycle it is writing LCDC, and takes neither IF
//! nor an interrupt (`hdl/check/lcd_on_lyc0.asm` enters the handler at the
//! same instruction on both).
//!
//! What it does not cover yet: STAT's mode bits, the HBlank source
//! (render.vhd's mode 0: its enable is held off here), and the pixel
//! pipeline's resyncs on the other lines (at `h = 893`, where ceres's clock
//! already is, in the traces measured).

use super::{
    DMG_PHASES, LAST_LINE, LINE_END_H, LINE_H, LcdTiming, PendingWrite, Phases, VBLANK_H,
    check::{Access, Net, ly_at, off_related, related_at, step_agrees, vblank_at},
};
use crate::ppu::STAT_IF_LYC_B;
use ceres_netlist::{lcd_control::NetLcdControl, lcd_timing::NetLcd};

/// A DMG's line timing with the LCD on, at any half dot of any line: every
/// field that `related` does not fix is free, so that a proof from it holds
/// for every related state (the phases are the DMG's, which nothing changes).
fn any_ceres() -> LcdTiming {
    let line: u8 = kani::any();
    let h: i32 = kani::any();
    kani::assume(line <= LAST_LINE && (0..LINE_H).contains(&h));
    let enables: u8 = kani::any();
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
        hblank_pending: kani::any(),
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
    NetLcd {
        first_line: kani::any(),
        ..net_at(t, t.line, t.h)
    }
}

/// The netlist's flip-flops at half dot `h` of `line`, with ceres's LY,
/// VBlank, LYC, LY = LYC flag and enables.
fn net_at(t: &LcdTiming, line: u8, h: i32) -> NetLcd {
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

/// The states match at any half dot (`check::related_at`, both clocks at
/// ceres's).
fn related(net: &NetLcd, t: &LcdTiming) -> bool {
    related_at(*net, t, t.line, t.h)
}

/// A half dot of a running LCD: the same rise of the STAT interrupt's line
/// on both sides, and related states after it.
// @assumes transcription-lcd
// @assumes mapping-half-dot
// @assumes resync-only-in-line-0
// @assumes hblank-source-off
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

/// Any access to STAT, LY or LYC.
fn any_access() -> Access {
    match kani::any::<u8>() % 7 {
        0 => Access::None,
        1 => Access::ReadStat,
        2 => Access::ReadLy,
        3 => Access::ReadLyc,
        4 => Access::WriteStat(kani::any()),
        5 => Access::WriteLyc(kani::any()),
        _ => Access::WriteLcdc(kani::any()),
    }
}

/// Any write on its way.
fn any_pending() -> Option<PendingWrite> {
    if kani::any() {
        Some(PendingWrite {
            value: kani::any(),
            half_dots_left: kani::any(),
        })
    } else {
        None
    }
}

/// Any line timing, every field free.
fn any_lcd() -> LcdTiming {
    LcdTiming {
        phases: Phases {
            cgb: kani::any(),
            hblank_clear: kani::any(),
            line153_ly_reset: kani::any(),
            scan_start: kani::any(),
            scan_end: kani::any(),
            mode1_start: kani::any(),
            mode1_end: kani::any(),
            flag_holds: kani::any(),
        },
        on: kani::any(),
        first_line: kani::any(),
        line: kani::any(),
        h: kani::any(),
        ly: kani::any(),
        line_end: kani::any(),
        vblank: kani::any(),
        hblank: kani::any(),
        hblank_pending: kani::any(),
        ly_match: kani::any(),
        enables: kani::any(),
        lyc: kani::any(),
        stat_line: kani::any(),
        lyc_pending: any_pending(),
        stat_pending: any_pending(),
        dirty: kani::any(),
    }
}

/// Any netlist line timing, every flip-flop free.
fn any_net_lcd() -> NetLcd {
    NetLcd {
        clk: kani::any(),
        wuvu: kani::any(),
        vena: kani::any(),
        wosu: kani::any(),
        lx: kani::any(),
        vclk: kani::any(),
        nype: kani::any(),
        ly: kani::any(),
        line_153: kani::any(),
        first_line: kani::any(),
        mode1: kani::any(),
        frame: kani::any(),
        line_parity: kani::any(),
        cpg_lx: kani::any(),
        lyc: kani::any(),
        lyc_match: kani::any(),
        stat_enable: kani::any(),
    }
}

/// An M-cycle with any access to STAT, LY, LYC or LCDC, from every related
/// pair (`check::step_agrees`): the LCD on, off, or in line 0 after it was
/// turned on, with ceres's clock ahead until the resync. The same byte
/// read, the same rises of the interrupt's line in the T-cycles IF tells
/// apart, and related states after it. Within a T-cycle, an LYC write
/// raises ceres's line a half dot before the netlist's sample at H:
/// ceres's comparison takes the new LYC at once.
// @assumes transcription-lcd
// @assumes transcription-lcd-control
// @assumes mapping-half-dot
// @assumes mapping-access-at-h0
// @assumes mapping-lcdc-write
// @assumes resync-only-in-line-0
// @assumes if-sees-rises-by-t-cycle
// @assumes hblank-source-off
// @assumes lcd-on-lyc0-invisible
// @discharges lcd-step-agrees
#[kani::proof]
#[kani::unwind(10)]
fn netlist_lcd_access() {
    let net = Net {
        lcd: any_net_lcd(),
        control: NetLcdControl { lcdc: kani::any() },
    };
    assert!(step_agrees(net, any_lcd(), any_access()));
}

/// A DMG's LCD that is off, with any LYC, LY = LYC flag and enables but
/// HBlank's, in ceres and in the netlist (whose clock's flip-flops the
/// reset holds at 0).
fn any_off() -> (LcdTiming, NetLcd) {
    let mut t = any_ceres();
    t.turn_off();
    let net = NetLcd {
        clk: false,
        wuvu: false,
        vena: false,
        wosu: false,
        lx: 0,
        vclk: false,
        nype: false,
        ly: 0,
        line_153: false,
        first_line: false,
        mode1: false,
        frame: false,
        line_parity: false,
        cpg_lx: false,
        lyc: t.lyc,
        lyc_match: t.ly_match,
        stat_enable: t.enables,
    };
    (t, net)
}

/// An LCDC write that turns the LCD on, and the M-cycle after it. Ceres's
/// lands 2 half dots before its access, starts its clock there at `h = 0`
/// and compares LY 0 with LYC at once; the netlist's latch opens in E, 3
/// edges later, and its clock starts from the reset: F's edge sets `wuvu`,
/// H's raises hclk, where LY = LYC is first sampled. The rises, by the
/// T-cycles IF tells apart, are the same, except that with LYC 0, its
/// enable on and the flag clear from before, ceres raises the line at once,
/// in the M-cycle before, and the netlist at H. Then ceres's clock runs 2
/// half dots ahead of the netlist's, as the netlist's line 0 began at D.
// @assumes transcription-lcd
// @assumes mapping-half-dot
// @assumes mapping-lcdc-write
// @assumes if-sees-rises-by-t-cycle
// @assumes lcd-on-lyc0-invisible
#[kani::proof]
fn netlist_lcd_on() {
    let (mut t, mut net) = any_off();
    kani::assume(off_related(&net, &t));
    let flag_before = t.ly_match;
    // Half dots from 2 before the access (B, `h = 6`) to the end of the
    // M-cycle after it (D, `h = 0`): ceres's in [-2, 8], the netlist's
    // edges in [-1, 8].
    let mut ceres_rose = [false; 11];
    ceres_rose[0] = t.turn_on(DMG_PHASES);
    for rose in ceres_rose.iter_mut().skip(1) {
        *rose = t.unit();
    }
    let mut net_rose = [false; 11];
    for (k, rose) in net_rose.iter_mut().enumerate().skip(1) {
        if k <= 3 {
            // C, D and E's edges in reset; the latch opens in E.
            net.clk = !net.clk;
        } else {
            *rose = net.edge(false);
        }
    }
    // By T-cycle: the last two of the M-cycle before (A to D), then the
    // M-cycle's: ceres's first (E to G), second (H), last two (A to D).
    let ceres_by_t = [
        ceres_rose[..3].contains(&true),
        ceres_rose[3] || ceres_rose[4],
        ceres_rose[5] || ceres_rose[6],
        ceres_rose[7..].contains(&true),
    ];
    let net_by_t = [
        net_rose[..3].contains(&true),
        net_rose[3] || net_rose[4] || net_rose[5],
        net_rose[6],
        net_rose[7..].contains(&true),
    ];
    let early = t.enables & STAT_IF_LYC_B != 0 && t.lyc == 0 && !flag_before;
    if early {
        assert!(
            ceres_by_t == [true, false, false, false] && net_by_t == [false, false, true, false],
            "LYC 0: not the M-cycle before"
        );
    } else {
        assert!(
            ceres_by_t == net_by_t,
            "the interrupt's line rises differently"
        );
    }
    assert!(t.h == 10 && t.ly_match == (t.lyc == 0), "ceres's clock");
    assert!(
        net.lyc_match == (net.lyc == 0),
        "the netlist's first sample"
    );
    assert!(related_at(net, &t, 0, 8), "the states part");
    kani::cover!(early, "LYC 0 raises ceres's line an M-cycle early");
}
