//! The line timing's bisimulation with the netlist, as one function that
//! Kani proves for every argument and Aeneas takes to Lean (`lean/`), as
//! `timing::check` does for the timer.
//!
//! `NetLcd` (`ceres_netlist::lcd_timing`) transcribes `hdl/rtl/lcd_timing.vhd`
//! edge by edge; `netlist_proofs.rs` gives the mapping of ceres's half dots
//! onto the netlist's edges. Ceres's M-cycle boundary is `h = 0` (mod 8),
//! the netlist's D edge: the CPU accesses STAT, LY, LYC or LCDC there, and
//! the M-cycle's eight half dots are the edges E to D. What the step gives
//! IF is the STAT line's rises: the netlist's by edge, ceres's by T-cycle
//! (the one at the access counts in the first), which IF tells apart as
//! `interrupts::check::same_stat` says.
//!
//! The LCD turned off and on: the netlist's side holds LCDC's latches,
//! whose bit 7 holds the line timing in reset, as `NetChip` does. Ceres's
//! LCDC write lands 2 half dots before its access (`LCDC_EARLY`), which
//! the step's M-cycle cannot run before it: but what the write does is
//! the same from the access on. Turning the LCD off, ceres's clock runs on
//! `LCD_OFF_LATE` half dots from where the write lands, before the reset
//! reaches the sources: the last of them after the access. Turning it on,
//! ceres's clock starts where the write lands, 2 half dots ahead of the
//! netlist's from the reset, through line 0, until the pixel pipeline's
//! line end resyncs it (at 895, `SKEWED_LINE_END`): there ceres's accesses
//! fall at `h = 2` (mod 8), still the netlist's D edge.
//!
//! Written as Aeneas takes it, as ceres-netlist is (see its crate
//! documentation).
#![cfg_attr(
    not(kani),
    expect(
        dead_code,
        reason = "outside Kani, only Charon reads the steps (`lean/translate.sh`)"
    )
)]
#![expect(
    clippy::needless_bitwise_bool,
    reason = "Aeneas (Lean) takes this module: a lazy && or || is a branch, and its symbolic execution forks on every one"
)]

use super::{
    DMG_DELAYS, DMG_PHASES, LAST_LINE, LAST_VISIBLE_LINE, LCD_OFF_LATE, LINE_END_H, LINE_H,
    LcdTiming, Phases, STAT_ENABLES, VBLANK_H,
};
use crate::{
    interrupts::check::same_stat,
    ppu::{LCDC_ON_B, STAT_IF_HBLANK_B, STAT_IF_LYC_B, STAT_LYC_B},
    timing::check::same_read,
};
use ceres_netlist::{lcd_control::NetLcdControl, lcd_timing::NetLcd};

/// A CPU access to the line timing's registers in the M-cycle (a write to
/// LY is not this block's; of LCDC's bits, only the LCD's enable is).
#[derive(Clone, Copy)]
pub(crate) enum Access {
    None,
    ReadStat,
    ReadLy,
    ReadLyc,
    WriteStat(u8),
    WriteLyc(u8),
    WriteLcdc(u8),
}

/// The netlist's line timing and LCDC's latches, whose bit 7 holds it in
/// reset.
pub(crate) struct Net {
    pub lcd: NetLcd,
    pub control: NetLcdControl,
}

/// The half dots ceres's DMG LCDC write lands before its access
/// (`ConflictType::DmgLcdc` in `Gb::cpu_write`, `mapping-lcdc-write`).
const LCDC_EARLY: u8 = 2;

/// Where ceres's clock is in line 0 after the LCD is turned on as the pixel
/// pipeline's line end resyncs it (`resync-only-in-line-0`).
const SKEWED_LINE_END: i32 = LINE_H - 17;

/// The first and last of the netlist's M-cycle boundaries in line 0 after
/// the LCD is turned on with ceres's clock ahead: from the reset's end to
/// the M-cycle of the resync.
const SKEWED_FIRST: i32 = 8;
const SKEWED_LAST: i32 = SKEWED_LINE_END - 7;

/// STAT's bits that this block drives: the LY = LYC flag and the enables
/// (its mode bits are the scan's and the pixels').
const STAT_OWN: u8 = STAT_ENABLES | STAT_LYC_B;

/// LY as the comparator sees it, at half dot `h` of `line`: the next line's
/// from the line's end, 0 through line 153 from its fourth half dot.
pub(crate) const fn ly_at(line: u8, h: i32) -> u8 {
    if h >= LINE_END_H {
        if line == LAST_LINE { 0 } else { line + 1 }
    } else if (line == LAST_LINE) & (h >= 4) {
        0
    } else {
        line
    }
}

/// VBlank (POPU), at half dot `h` of `line`.
pub(crate) const fn vblank_at(line: u8, h: i32) -> bool {
    match line {
        LAST_VISIBLE_LINE => h >= VBLANK_H,
        LAST_LINE => h < VBLANK_H,
        _ => line > LAST_VISIBLE_LINE,
    }
}

/// The DMG's phases.
const fn dmg_phases(p: Phases) -> bool {
    let d = DMG_PHASES;
    (p.cgb == d.cgb)
        & (p.hblank_clear == d.hblank_clear)
        & (p.line153_ly_reset == d.line153_ly_reset)
        & (p.scan_start == d.scan_start)
        & (p.scan_end == d.scan_end)
        & (p.mode1_start == d.mode1_start)
        & (p.mode1_end == d.mode1_end)
        & (p.flag_holds == d.flag_holds)
}

/// The states match with the netlist at half dot `h` of `line`, which
/// ceres's clock may run ahead of (line 0 after the LCD is turned on): the
/// netlist's flip-flops are what that half dot says, with ceres's LY,
/// VBlank, LYC, LY = LYC flag and enables, and ceres's line, LY, VBlank and
/// interrupt line are consistent. The LCD's pins' flip-flops (`first_line`,
/// `frame`, `line_parity`, `cpg_lx`), which nothing here reads, are free.
/// HBlank's enable is held off (`hblank-source-off`).
#[expect(
    clippy::suspicious_operation_groupings,
    reason = "the netlist's mode 1 is ceres's VBlank"
)]
pub(crate) const fn related_at(net: NetLcd, t: &LcdTiming, line: u8, h: i32) -> bool {
    let in_range = (t.line <= LAST_LINE)
        & (t.h >= 0)
        & (t.h < LINE_H)
        & (line <= LAST_LINE)
        & (h >= 0)
        & (h < LINE_H);
    if !in_range {
        return false;
    }
    let ceres = dmg_phases(t.phases)
        & (t.enables & !(STAT_ENABLES & !STAT_IF_HBLANK_B) == 0)
        & !t.hblank_pending
        & t.on
        & !t.dirty
        & t.lyc_pending.is_none()
        & t.stat_pending.is_none()
        & (t.ly == ly_at(t.line, t.h))
        & (t.line_end == (t.h >= LINE_END_H))
        & (t.vblank == vblank_at(t.line, t.h))
        & (t.stat_line == t.line_for(t.enables));
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "from 1 to 113 in that range"
    )]
    let lx = if (h >= 4) & (h < LINE_END_H) {
        ((h - 4) / 8 + 1) as u8
    } else {
        0
    };
    let clocks = (net.clk == (h & 1 != 0))
        & (net.wuvu == (h & 2 != 0))
        & (net.vena == (h & 7 >= 4))
        & (net.wosu == ((h + 1) & 2 != 0))
        & (net.lx == lx)
        & (net.vclk == (h >= LINE_END_H))
        & (net.nype == ((h >= VBLANK_H) | (h < 4)))
        & (net.line_153 == (((line == LAST_LINE) & (h >= 4)) | ((line == 0) & (h < 4))));
    let regs = (net.ly == t.ly)
        & (net.mode1 == t.vblank)
        & (net.lyc == t.lyc)
        & (net.lyc_match == t.ly_match)
        & (net.stat_enable == t.enables);
    ceres & clocks & regs
}

/// Line 0 after the LCD was turned on, ceres's clock 2 half dots ahead of
/// the netlist's: from the reset's end to the resync.
const fn skewed(t: &LcdTiming) -> bool {
    t.on & t.first_line & (t.line == 0) & (t.h & 7 == 2)
}

/// Both LCDs are off and agree on what outlives it: LYC, the LY = LYC flag
/// and the enables (not HBlank's), the netlist's clock held at the
/// boundary's level. Ceres's clock, which runs on, is anywhere in the
/// frame: turning the LCD on restarts it.
#[expect(
    clippy::suspicious_operation_groupings,
    reason = "the netlist's LY = LYC flag is ceres's"
)]
pub(crate) const fn off_related(net: &NetLcd, t: &LcdTiming) -> bool {
    dmg_phases(t.phases)
        & (t.line <= LAST_LINE)
        & (t.h >= 0)
        & (t.h < LINE_H)
        & !t.on
        & (t.enables & !(STAT_ENABLES & !STAT_IF_HBLANK_B) == 0)
        & !t.hblank
        & !t.hblank_pending
        & !t.line_end
        & !t.vblank
        & (t.ly == 0)
        & (t.stat_line == t.line_for(t.enables))
        & t.lyc_pending.is_none()
        & t.stat_pending.is_none()
        & !net.clk
        & !net.wuvu
        & !net.vena
        & !net.wosu
        & (net.lx == 0)
        & !net.vclk
        & !net.nype
        & (net.ly == 0)
        & !net.line_153
        & !net.mode1
        & (net.lyc == t.lyc)
        & (net.lyc_match == t.ly_match)
        & (net.stat_enable == t.enables)
        & (net.int_stat(false) == t.stat_line)
}

/// The states match at an M-cycle's boundary, HBlank's gate low: with the
/// LCD on, both clocks at the same half dot (`h = 0`, mod 8), or in line
/// 0 after it was turned on, ceres's 2 half dots ahead, the flag LY 0's
/// comparison with LYC (which ceres's clock samples first, as LY stays 0;
/// a written LYC both compare by the boundary); or the LCD off.
#[expect(
    clippy::verbose_bit_mask,
    reason = "the phase in the M-cycle, as the netlist's dividers count it"
)]
pub(crate) const fn related(net: &Net, t: &LcdTiming) -> bool {
    if net.control.ppu_reset(false) {
        return off_related(&net.lcd, t);
    }
    let aligned = (t.h & 7 == 0) & related_at(net.lcd, t, t.line, t.h);
    let ahead = if skewed(t) & (t.h >= SKEWED_FIRST + 2) & (t.h <= SKEWED_LAST + 2) {
        (t.ly_match == (t.ly == t.lyc)) & related_at(net.lcd, t, 0, t.h - 2)
    } else {
        false
    };
    !t.hblank & (aligned | ahead)
}

/// What the proof leaves out: a STAT write that enables the HBlank source
/// (`hblank-source-off`), and turning the LCD on with LYC 0, its enable on
/// and the flag clear, when ceres raises the line at once, an M-cycle
/// before the netlist, at its first sample in H (`lcd-on-lyc0-invisible`).
pub(crate) const fn outside(net: &Net, access: Access) -> bool {
    match access {
        Access::WriteStat(data) => data & STAT_IF_HBLANK_B != 0,
        Access::WriteLcdc(data) => {
            (data & LCDC_ON_B != 0)
                & net.control.ppu_reset(false)
                & (net.lcd.stat_enable & STAT_IF_LYC_B != 0)
                & (net.lcd.lyc == 0)
                & !net.lcd.lyc_match
        }
        Access::None
        | Access::ReadStat
        | Access::ReadLy
        | Access::ReadLyc
        | Access::WriteLyc(_) => false,
    }
}

/// Ceres's LCDC write, from its access: whether the line rose. Turned off,
/// the clock runs the last of `LCD_OFF_LATE` half dots; turned on, it
/// catches up the `LCDC_EARLY` half dots since the write landed
/// (`Ppu::write_lcdc`, which calls these as the LCD's enable changes).
const fn ceres_lcdc(t: &mut LcdTiming, data: u8) -> bool {
    let on = data & LCDC_ON_B != 0;
    let mut rose = false;
    if on & !t.on {
        rose = t.turn_on(DMG_PHASES);
        let mut k = 0;
        while k < LCDC_EARLY {
            rose |= t.unit();
            k += 1;
        }
    } else if !on & t.on {
        let mut k = LCDC_EARLY;
        while k < LCD_OFF_LATE {
            rose |= t.unit();
            k += 1;
        }
        t.turn_off();
    } else {
        // The LCD stays as it is.
    }
    rose
}

/// A half dot, and the pixel pipeline's line end in line 0 after the LCD
/// was turned on (`Ppu::timing_line_end`), once: whether the line rose.
const fn ceres_unit(t: &mut LcdTiming, ahead: &mut bool) -> bool {
    let rose = t.unit();
    if *ahead & (t.h == SKEWED_LINE_END) {
        t.resync(t.line);
        *ahead = false;
    }
    rose
}

/// Ceres's M-cycle: the access, then eight half dots. What a read gives,
/// and the rises of the interrupt's line by T-cycle (two half dots each;
/// one at the access counts in the first).
pub(crate) const fn ceres_m_cycle(t: &mut LcdTiming, access: Access) -> (Option<u8>, [bool; 4]) {
    let mut ahead = skewed(t);
    let mut at_access = false;
    let read = match access {
        Access::None => None,
        Access::ReadStat => Some(t.read_stat(t.enables, DMG_DELAYS, false) & STAT_OWN),
        Access::ReadLy => Some(t.read_ly(DMG_DELAYS)),
        Access::ReadLyc => Some(t.lyc),
        Access::WriteStat(data) => {
            at_access = t.write_stat(data, DMG_DELAYS);
            None
        }
        Access::WriteLyc(data) => {
            t.write_lyc(data, DMG_DELAYS);
            None
        }
        Access::WriteLcdc(data) => {
            at_access = ceres_lcdc(t, data);
            None
        }
    };
    let mut rose = [false; 4];
    let mut k = 0;
    while k < 4 {
        let first = ceres_unit(t, &mut ahead);
        let second = ceres_unit(t, &mut ahead);
        rose[k] = first | second;
        k += 1;
    }
    rose[0] |= at_access;
    (read, rose)
}

/// An edge of the PPU's clock, with LCDC as the edge finds it, which holds
/// the line timing in reset while its bit 7 is clear, as `NetChip::step_lcd`
/// does: whether the line rose.
const fn net_edge(net: &mut Net) -> bool {
    let held = net.control.ppu_reset(false);
    let rose = net.lcd.held_edge(held, false);
    if held {
        net.lcd.reset();
    }
    rose
}

/// The register a write reaches takes `byte`: whether the line rose.
const fn net_write(net: &mut NetLcd, access: Access, byte: u8) -> bool {
    let level = net.int_stat(false);
    match access {
        Access::WriteStat(_) => net.write(0xFF41, byte),
        Access::WriteLyc(_) => net.write(0xFF45, byte),
        Access::None
        | Access::ReadStat
        | Access::ReadLy
        | Access::ReadLyc
        | Access::WriteLcdc(_) => {}
    }
    net.int_stat(false) & !level
}

/// What a read drives on the bus now (of STAT, this block's bits).
const fn net_shown(net: &NetLcd, access: Access) -> Option<u8> {
    match access {
        Access::ReadStat => match net.read(0xFF41, false, false) {
            Some(b) => Some(b & STAT_OWN),
            None => None,
        },
        Access::ReadLy => net.read(0xFF44, false, false),
        Access::ReadLyc => net.read(0xFF45, false, false),
        Access::None | Access::WriteStat(_) | Access::WriteLyc(_) | Access::WriteLcdc(_) => None,
    }
}

/// The precharged bus, pulled low by either of two drives.
const fn and_bus(a: Option<u8>, b: Option<u8>) -> Option<u8> {
    match (a, b) {
        (Some(x), Some(y)) => Some(x & y),
        _ => None,
    }
}

/// The netlist's M-cycle from D's edge: E's edge, where a write opens the
/// latch, which takes the precharged bus (FF) and then the byte (LCDC's
/// opens after the edge, which still finds the old one; then its bit 7
/// clear resets the line timing); then the edges F to D. A read gives the
/// AND of what the register shows after E's edge to H's. The rises of the
/// interrupt's line, by edge.
pub(crate) const fn net_m_cycle(net: &mut Net, access: Access) -> (Option<u8>, [bool; 8]) {
    let mut rose = [false; 8];
    rose[0] = net_edge(net);
    match access {
        Access::WriteStat(data) | Access::WriteLyc(data) => {
            let glitch = net_write(&mut net.lcd, access, 0xFF);
            let lands = net_write(&mut net.lcd, access, data);
            rose[0] = rose[0] | glitch | lands;
        }
        Access::WriteLcdc(data) => {
            net.control.settle(false, Some(data));
            if net.control.ppu_reset(false) {
                net.lcd.reset();
            }
        }
        Access::None | Access::ReadStat | Access::ReadLy | Access::ReadLyc => {}
    }
    let mut bus = net_shown(&net.lcd, access);
    rose[1] = net_edge(net);
    bus = and_bus(bus, net_shown(&net.lcd, access));
    rose[2] = net_edge(net);
    bus = and_bus(bus, net_shown(&net.lcd, access));
    rose[3] = net_edge(net);
    bus = and_bus(bus, net_shown(&net.lcd, access));
    let mut k = 4;
    while k < 8 {
        rose[k] = net_edge(net);
        k += 1;
    }
    (bus, rose)
}

/// The step: from a related pair, an M-cycle with any access to STAT, LY,
/// LYC or LCDC (but what the proof leaves out) gives the same read on both
/// sides and the same rises of the interrupt's line as IF tells them
/// apart, and leaves the pair related. True for every argument (Kani's
/// `netlist_lcd_access`).
#[cfg_attr(
    not(kani),
    expect(clippy::missing_const_for_fn, reason = "Kani's covers are not const")
)]
pub(crate) fn step_agrees(mut net: Net, mut t: LcdTiming, access: Access) -> bool {
    if !related(&net, &t) | outside(&net, access) {
        return true;
    }
    #[cfg(kani)]
    let (was_on, was_ahead) = (t.on, skewed(&t));
    let (net_read, net_rose) = net_m_cycle(&mut net, access);
    let (ceres_read, ceres_rose) = ceres_m_cycle(&mut t, access);
    #[cfg(kani)]
    {
        kani::cover!(
            matches!(access, Access::WriteStat(_)) && ceres_rose[0],
            "a STAT write's glitch raises the line"
        );
        kani::cover!(
            matches!(access, Access::WriteLyc(_)) && ceres_rose[1],
            "an LYC write raises the line"
        );
        kani::cover!(
            matches!(access, Access::ReadStat) && ceres_read.is_some_and(|b| b & STAT_LYC_B != 0),
            "a STAT read sees LY = LYC"
        );
        kani::cover!(
            matches!(access, Access::ReadLy) && ceres_read == Some(0) && t.line == LAST_LINE,
            "LY reads 0 in line 153"
        );
        kani::cover!(ceres_rose[3], "the line rises in the last T-cycle");
        kani::cover!(!was_on && t.on, "the LCD is turned on");
        kani::cover!(was_on && !t.on, "the LCD is turned off");
        kani::cover!(
            was_ahead && !skewed(&t) && t.on,
            "the pixel pipeline's line end resyncs the clock"
        );
        kani::cover!(
            was_ahead && matches!(access, Access::ReadStat | Access::WriteLyc(_)),
            "an access in line 0 after the LCD is turned on"
        );
        kani::cover!(
            !was_on && matches!(access, Access::WriteStat(_)) && ceres_rose[0],
            "a STAT write raises the line with the LCD off"
        );
    }
    same_read(net_read, ceres_read) & same_stat(net_rose, ceres_rose) & related(&net, &t)
}
