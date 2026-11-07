//! Kani proofs that ceres's line timing is the netlist's, on the DMG
//! (`cargo kani -p ceres-core --harness netlist_lcd`).
//!
//! `NetLcd` (`ceres_netlist::lcd_timing`) transcribes `hdl/rtl/lcd_timing.vhd`
//! edge by edge; ceres's `LcdTiming::unit` advances a half dot, which is an
//! edge. Ceres's half dot `h` names the netlist's dividers: hclk rises at `h =
//! 4` (mod 8), where LY = LYC is sampled and nype follows vclk, and falls at `h
//! = 0` (mod 8), where vclk follows LX; the line's end (vclk, ceres's
//! `line_end`) rises at 904, so that LY changes there and VBlank at 908. The
//! proof is a bisimulation: from any pair of states that `related` matches,
//! an M-cycle raises the STAT interrupt's line in the same T-cycles, as IF
//! tells them apart, and leaves them related.
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
//! The HBlank source: render's mode 0, which rises as mode 3 ends and falls
//! at the scan's line start (`h = 906`), VBlank masking it, is ceres's
//! HBlank gate, which the pixel pipeline raises (`Ppu::timing_mode3_end`, a
//! half dot before) and the line timing clears at `h = 906`. Where mode 3
//! ends is an input of the step, the same on both sides
//! (`mapping-mode3-end`): an even half dot of a visible line, before the
//! line's end. A sprite at X 167 makes render's mode 0 pulse, which the
//! input does not give (the fact `sprite-at-167-mode-0-pulse`).
//!
//! What it does not cover yet: STAT's mode bits (the scan's and the
//! pixels'), the pixel pipeline itself, and its resyncs on the other lines
//! (at `h = 893`, where ceres's clock already is, in the traces measured).

use super::{
    DMG_PHASES, LAST_LINE, LINE_END_H, LINE_H, LcdTiming, PendingWrite, Phases,
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

/// Any access to STAT, LY, LYC or LCDC.
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
/// turned on, with ceres's clock ahead until the resync; and mode 3 ending
/// at any half dot, render's mode 0 the HBlank source. The same byte read,
/// the same rises of the interrupt's line in the T-cycles IF tells apart,
/// and related states after it. Within a T-cycle, an LYC write
/// raises ceres's line a half dot before the netlist's sample at H:
/// ceres's comparison takes the new LYC at once.
// @assumes transcription-lcd
// @assumes transcription-lcd-control
// @assumes mapping-half-dot
// @assumes mapping-access-at-h0
// @assumes mapping-lcdc-write
// @assumes resync-only-in-line-0
// @assumes if-sees-rises-by-t-cycle
// @assumes mapping-mode3-end
// @assumes lcd-on-lyc0-invisible
// @discharges lcd-step-agrees
#[kani::proof]
#[kani::unwind(10)]
fn netlist_lcd_access() {
    let net = Net {
        lcd: any_net_lcd(),
        control: NetLcdControl { lcdc: kani::any() },
        mode0: kani::any(),
    };
    let mode3_end = if kani::any() {
        Some(kani::any::<u8>() % 8)
    } else {
        None
    };
    assert!(step_agrees(net, any_lcd(), any_access(), mode3_end));
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
