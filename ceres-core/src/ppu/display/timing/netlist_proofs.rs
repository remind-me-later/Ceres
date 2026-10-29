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
//! The LCD turned off and on: ceres's LCDC write lands 2 half dots before
//! its access (SameBoy's `cycle_write`), the netlist's latch opens in E.
//! `netlist_lcd_off` proves the off alike (ceres runs 3 half dots before its
//! reset, to E). `netlist_lcd_on` proves the on and the M-cycle after it
//! alike but for one case: with LYC 0, its enable on and the flag clear,
//! ceres raises the line in the M-cycle before the write's, at once, the
//! netlist at H, after its first sample. The CPU cannot see it: in that
//! M-cycle it is writing LCDC, and takes neither IF nor an interrupt
//! (`hdl/check/lcd_on_lyc0.asm` enters the handler at the same instruction
//! on both). Then ceres's clock runs 2 half dots ahead through line 0, as
//! the netlist's line began at D, until the pixel pipeline resyncs it
//! (`netlist_lcd_line0`).
//!
//! What it does not cover yet: STAT's mode bits, the HBlank source
//! (render.vhd's mode 0: its enable is held off here), the pixel pipeline's
//! resyncs on the other lines (at `h = 893`, where ceres's clock already
//! is, in the traces measured), and accesses in line 0 after the LCD is
//! turned on.

use super::{
    DMG_DELAYS, DMG_PHASES, LAST_LINE, LAST_VISIBLE_LINE, LCD_OFF_LATE, LINE_END_H, LINE_H,
    LcdTiming, STAT_ENABLES, VBLANK_H, netlist::NetLcd,
};
use crate::ppu::{STAT_IF_HBLANK_B, STAT_IF_LYC_B, STAT_LYC_B};

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
    net_at(t, t.line, t.h)
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

/// The states match: the netlist's flip-flops are what ceres's half dot
/// says, and ceres's line, LY, VBlank and interrupt line are consistent.
fn related(net: &NetLcd, t: &LcdTiming) -> bool {
    related_at(net, t, t.line, t.h)
}

/// The same with the netlist at half dot `h` of `line`, which ceres's clock
/// may run ahead of (line 0 after the LCD is turned on). `first_line` only
/// drives the LCD's S pin, which nothing here reads.
fn related_at(net: &NetLcd, t: &LcdTiming, line: u8, h: i32) -> bool {
    let want = net_at(t, line, h);
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

/// Both LCDs are off and agree on what outlives it.
fn off_related(net: &NetLcd, t: &LcdTiming) -> bool {
    !t.on
        && !t.line_end
        && !t.vblank
        && t.ly == 0
        && t.stat_line == t.line_for(t.enables)
        && t.lyc_pending.is_none()
        && t.stat_pending.is_none()
        && !net.wuvu
        && !net.vena
        && !net.wosu
        && net.lx == 0
        && !net.vclk
        && !net.nype
        && net.ly == 0
        && !net.line_153
        && !net.mode1
        && net.lyc == t.lyc
        && net.lyc_match == t.ly_match
        && net.stat_enable == t.enables
        && net.int_stat(false) == t.stat_line
}

/// An LCDC write that turns the LCD off. Ceres's lands 2 half dots before
/// its access (SameBoy's `cycle_write` for the DMG's LCDC) and runs
/// `LCD_OFF_LATE` = 3 more half dots before its reset; the netlist's latch
/// opens in E, 3 edges later, and resets the clock's flip-flops. The same
/// rises in those half dots, and two LCDs off that agree. (2 half dots
/// would do as well here: E's edge changes nothing in this block; 1 is
/// caught.)
#[kani::proof]
fn netlist_lcd_off() {
    let mut t = any_ceres();
    kani::assume(t.h & 7 == 6);
    let mut net = net_for(&t);
    kani::assume(related(&net, &t));
    let mut ceres_rose = [false; 3];
    for rose in ceres_rose.iter_mut().take(usize::from(LCD_OFF_LATE)) {
        *rose = t.unit();
    }
    t.turn_off();
    let mut net_rose = [false; 3];
    for rose in &mut net_rose {
        *rose = net.edge(false);
    }
    // E: the latch takes the precharged bus (FF: the LCD stays on), then
    // the byte, whose bit 7 resets the clock.
    net.reset();
    assert!(
        ceres_rose == net_rose,
        "the interrupt's line rises differently"
    );
    assert!(off_related(&net, &t), "the LCDs off part");
    kani::cover!(
        ceres_rose.contains(&true),
        "the line rises as the LCD goes off"
    );
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
    assert!(related_at(&net, &t, 0, 8), "the states part");
    kani::cover!(early, "LYC 0 raises ceres's line an M-cycle early");
}

/// A half dot of line 0 after the LCD was turned on, with ceres's clock 2
/// half dots ahead of the netlist's, until the pixel pipeline resyncs it at
/// the line's end (`Ppu::timing_line_end`, when ceres's clock reaches 895):
/// the same rises, and then the states related as on any other line.
#[kani::proof]
fn netlist_lcd_line0() {
    let h: i32 = kani::any();
    kani::assume((8..893).contains(&h));
    let mut t = any_ceres();
    t.line = 0;
    t.h = h + 2;
    t.ly = 0;
    t.line_end = false;
    t.vblank = false;
    // LYC was compared with LY 0 (at the LCD's turning on or at a sample
    // since; an LYC write in line 0 is the access proof's).
    t.ly_match = t.lyc == 0;
    t.stat_line = t.line_for(t.enables);
    let mut net = net_at(&t, 0, h);
    net.line_153 = false;
    net.first_line = false;
    kani::assume(related_at(&net, &t, 0, h));
    let ceres_rose = t.unit();
    if t.h == LINE_H - 17 {
        // The pipeline's line end.
        t.h = LINE_H - 19;
    }
    let net_rose = net.edge(false);
    assert!(
        ceres_rose == net_rose,
        "the interrupt's line rises differently"
    );
    if h + 1 == LINE_H - 19 {
        assert!(related(&net, &t), "the resync parts the clocks");
    } else {
        assert!(related_at(&net, &t, 0, h + 1), "the states part");
    }
    kani::cover!(h + 1 == LINE_H - 19, "the resync");
}
