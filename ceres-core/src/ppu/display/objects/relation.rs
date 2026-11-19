//! The OAM scan's bisimulation with the netlist, cut into one sprite's
//! window, as functions that Kani proves for every argument and Aeneas takes
//! to Lean (`lean/`), as `memory::dma::relation` does for the OAM DMA.
//!
//! `NetScan` (`ceres_netlist::oam_scan`) and OAM's latches
//! (`ceres_netlist::oam::NetOamLatches`) step edge by edge on the line
//! timing's clocks (`bisim.rs` relates them to ceres's half dot
//! `h`): `wuvu` rises at `h = 4j - 2`, and the scan's index goes to sprite
//! `j + 1`; OAM's second latches take the Y the first read in the window
//! before (sprite `j`'s), and the scan saves sprite `j` at `4j - 1` and `4j`
//! if it is on the line and the store has room; the count rises as the next
//! window starts. Ceres's search reads sprite `j + 1` at `4j + 1`
//! (`mapping-scan-reads`), the window's last half dot. So a step is the
//! netlist's window `j`, its edges `4j - 2` to `4j + 1`, then ceres's read of
//! sprite `j + 1`; and between steps ceres is a sprite ahead of the saves.
//!
//! Windows 0 and 1 carry the scan's start (`start_scan`, `anel`), and the
//! last its stop, so a scan is `lead_in` (from the line's end, `h = -8`, to
//! the boundary of window 2), `step` for windows 2 to 38, which leave the
//! pair `related` again, and `tail` (windows 39 and 40, to mode 3). Kani
//! proves the three; Lean carries `step` over the windows
//! (`lean/Ceres/Bisim/Scan.lean`), and cuts the scan into dots, the pairs
//! between them reached from those the three relate
//! (`lean/Ceres/Bisim/ScanDots.lean`).
//!
//! As in `bisim::netlist_scan_selection`: a visible line, LCDC bit
//! 2 the same throughout, no OAM DMA, the CPU not addressing OAM. Ceres's
//! side is its search's latches and the first ten sprites it finds on the
//! line, in index order, which `ObjectSearch::consider` keeps
//! (`search-keeps-first-ten`).
//!
//! Written as Aeneas takes it, as ceres-netlist is (see its crate
//! documentation).
#![cfg_attr(
    not(any(kani, test)),
    expect(
        dead_code,
        reason = "outside Kani and the tests, only Charon reads the steps (`lean/translate.sh`)"
    )
)]
#![expect(
    clippy::needless_bitwise_bool,
    reason = "Aeneas (Lean) takes this module: a lazy && or || is a branch, and its symbolic execution forks on every one"
)]

use super::{MAX_OBJECTS_PER_LINE, OamLatches, on_line};
use ceres_netlist::{
    oam::{NetOamLatches, OamInputs},
    oam_scan::{Clocks, NetScan, SLOTS},
};

/// What the scan sees throughout: the line, LCDC bit 2.
#[derive(Clone, Copy)]
pub(crate) struct Line {
    pub ly: u8,
    pub tall: bool,
}

/// The netlist's side: the scan, OAM's latches, the index the sprite store
/// latches as `wuvu` rises, and the slots, as the save strobes fill them.
// Copied only by the tests: Aeneas would leave `Option`'s clone an axiom.
#[cfg_attr(test, derive(Clone, Copy))]
pub(crate) struct NetSide {
    pub scan: NetScan,
    pub latches: NetOamLatches,
    pub scanned: u8,
    pub slots: [Option<u8>; 10],
}

/// Ceres's side: its search's latches, the sprites it has read, and the
/// first ten on the line, in index order.
#[cfg_attr(test, derive(Clone, Copy))]
pub(crate) struct CeresSide {
    pub latches: OamLatches,
    pub read: u8,
    pub first: [Option<u8>; 10],
    pub found: usize,
}

/// The line timing's dividers after the edge at half dot `h` of the line
/// (`bisim.rs`: `wuvu` is `t & 2`, `wosu` `(t + 1) & 2`, `t`
/// counted from the line before's start, 912 half dots earlier).
const fn wuvu(h: i32) -> bool {
    (h + 912) & 2 != 0
}

const fn wosu(h: i32) -> bool {
    (h + 913) & 2 != 0
}

/// The netlist's edge at half dot `h`, with OAM giving `y` at the scan's
/// index after it.
const fn net_edge(net: &mut NetSide, h: i32, line: Line, y: u8) {
    let clocks = Clocks {
        rising: h & 1 != 0,
        wuvu_before: wuvu(h - 1),
        wuvu_after: wuvu(h),
        wosu_before: wosu(h - 1),
        // vclk is high over the 8 half dots before the line.
        vclk: (h >= -7) & (h <= 0),
        ly: line.ly,
    };
    let index = net.scan.index;
    net.scan.edge(clocks, net.latches.second, line.tall);
    if !clocks.wuvu_before & clocks.wuvu_after {
        net.scanned = index;
    }
    let inputs = OamInputs {
        scanning: net.scan.scanning,
        run: false,
        dma_phi: false,
        wuvu: wuvu(h),
        wosu: wosu(h),
        fetch_read: false,
        fetch_hold: false,
    };
    net.latches.settle(inputs, y);
    if let Some(slot) = net
        .scan
        .save(wosu(h), net.latches.second, line.ly, line.tall)
    {
        net.slots[slot as usize] = Some(net.scanned);
    }
}

/// Ceres's read of the next sprite, whose Y is `y`.
pub(crate) const fn ceres_read(ceres: &mut CeresSide, line: Line, y: u8) {
    let (seen, _) = ceres.latches.latch(Some((y, 0)));
    if on_line(seen, line.ly, line.tall) & (ceres.found < MAX_OBJECTS_PER_LINE) {
        ceres.first[ceres.found] = Some(ceres.read);
        ceres.found += 1;
    }
    ceres.read += 1;
}

/// The netlist's window `j`: its edges `4j - 2` to `4j + 1`, OAM giving
/// sprite `j + 1`'s Y.
pub(crate) const fn net_window(net: &mut NetSide, line: Line, j: u8, y: u8) {
    let h = 4 * j as i32 - 2;
    net_edge(net, h, line, y);
    net_edge(net, h + 1, line, y);
    net_edge(net, h + 2, line, y);
    net_edge(net, h + 3, line, y);
}

/// The netlist's window `j`, then ceres's read of sprite `j + 1`: the two
/// sides' steps, which Lean also takes apart (`Ceres/Bisim/Scan.lean`).
pub(crate) const fn step(net: &mut NetSide, ceres: &mut CeresSide, line: Line, j: u8, y: u8) {
    net_window(net, line, j, y);
    ceres_read(ceres, line, y);
}

/// Whether two slots hold the same sprite (Aeneas does not model
/// `Option`'s equality).
const fn same_slot(a: Option<u8>, b: Option<u8>) -> bool {
    match (a, b) {
        (Some(x), Some(y)) => x == y,
        (None, None) => true,
        _ => false,
    }
}

/// The slots below `n` hold ceres's first `n` sprites, and those from `n`
/// on are empty.
const fn slots_are(net: &NetSide, ceres: &CeresSide, n: usize) -> bool {
    let mut ok = true;
    let mut i = 0;
    while i < 10 {
        let want = if i < n { ceres.first[i] } else { None };
        ok &= same_slot(net.slots[i], want);
        i += 1;
    }
    ok
}

/// Ceres's first `found` sprites are in index order, below `read`, and the
/// rest of its list empty.
const fn list_is_sound(ceres: &CeresSide) -> bool {
    let mut ok = ceres.found <= MAX_OBJECTS_PER_LINE;
    let mut last: i32 = -1;
    let mut i = 0;
    while i < 10 {
        let (present, index) = unpack(ceres.first[i]);
        // One Boolean expression: an `if` between comparisons comes out of
        // Aeneas as a proposition, where Lean needs a Bool.
        let here = (present & (i < ceres.found) & ((index as i32) > last) & (index < ceres.read))
            | (!present & (i >= ceres.found));
        ok &= here;
        if present {
            last = index as i32;
        }
        i += 1;
    }
    ok
}

/// Whether a slot holds a sprite, and its index (Aeneas does not take a
/// match on an array's element).
const fn unpack(slot: Option<u8>) -> (bool, u8) {
    match slot {
        Some(index) => (true, index),
        None => (false, 0),
    }
}

/// The pair at the boundary of window `j` (2 to 39): ceres has read
/// sprites 0 to `j`; the netlist's scan runs at index `j`, its store has
/// latched `j - 1`, OAM's first latches hold sprite `j`'s Y (ceres's) and
/// the second sprite `j - 1`'s, and its slots hold ceres's sprites but `j`:
/// those up to the count, and sprite `j - 1` if its save is pending (the
/// save went through in window `j - 1`: `not_saving` low, the count rising
/// at the next edge).
pub(crate) const fn related(net: &NetSide, ceres: &CeresSide, line: Line, j: u8) -> bool {
    let scan = &net.scan;
    let state = scan.scanning
        & scan.ceno
        & !scan.start_scan
        & !scan.anel
        & !scan.byba
        & !scan.doba
        & (scan.index == j)
        & (net.scanned == j.wrapping_sub(1))
        & (ceres.read == j.wrapping_add(1))
        & (net.latches.first == ceres.latches.y)
        & (line.ly < 144);
    let pending = !scan.not_saving & (scan.count < SLOTS);
    let saved = scan.count as usize + pending as usize;
    // Ceres's list: the netlist's saves, and sprite j if it was on the line.
    let j_kept = (ceres.found == saved + 1) & same_slot(ceres.first[saved % 10], Some(j));
    let lists = ((ceres.found == saved) | j_kept)
        & (scan.count <= SLOTS)
        & slots_are(net, ceres, saved)
        & list_is_sound(ceres);
    // Sprite j - 1's Y is the second latches': the netlist saved it iff it
    // is on the line and there was room, which ceres decided alike.
    let y_before = net.latches.second;
    let saves_it =
        on_line(y_before, line.ly, line.tall) & ((scan.count as usize) < MAX_OBJECTS_PER_LINE);
    let save_agrees = pending == saves_it;
    // Sprite j's: on the line, ceres kept it iff there was room.
    let keeps_j = on_line(ceres.latches.y, line.ly, line.tall) & (saved < MAX_OBJECTS_PER_LINE);
    let j_agrees = keeps_j == j_kept;
    state & lists & save_agrees & j_agrees
}

/// The step, for every argument: from a related pair at window `j` (2 to
/// 38) and any Y for sprite `j + 1`, the pair is related at `j + 1`.
pub(crate) const fn step_agrees(net: NetSide, ceres: CeresSide, line: Line, j: u8, y: u8) -> bool {
    if !((j >= 2) & (j <= 38) & related(&net, &ceres, line, j)) {
        return true;
    }
    let (mut net, mut ceres) = (net, ceres);
    step(&mut net, &mut ceres, line, j, y);
    related(&net, &ceres, line, j + 1)
}

/// Ten empty slots, written out (Aeneas takes `[None; 10]` through
/// `Option`'s clone, which it leaves an axiom).
const NO_SPRITES: [Option<u8>; 10] = [None, None, None, None, None, None, None, None, None, None];

/// The netlist at the end of the line before (`h = -8`): the scan of that
/// line ended (the index at the last sprite, the stop's flip-flops set),
/// with any count and any `not_saving`, and OAM's latches holding `held`.
pub(crate) const fn net_at_line_end(count: u8, not_saving: bool, held: u8) -> NetSide {
    NetSide {
        scan: NetScan {
            start_scan: false,
            anel: false,
            scanning: false,
            ceno: false,
            index: 39,
            byba: true,
            doba: true,
            not_saving,
            count,
        },
        latches: NetOamLatches {
            first: held,
            second: held,
        },
        scanned: 39,
        slots: NO_SPRITES,
    }
}

/// Ceres before its search: the latches holding `held`, nothing read.
pub(crate) const fn ceres_at_line_end(held: u8) -> CeresSide {
    CeresSide {
        latches: OamLatches { y: held, x: 0 },
        read: 0,
        first: NO_SPRITES,
        found: 0,
    }
}

/// The Ys OAM gives over the lead-in: sprite 39's (from the line before),
/// then 0's, 1's and 2's.
#[derive(Clone, Copy)]
#[cfg_attr(kani, derive(kani::Arbitrary))]
pub(crate) struct LeadYs {
    pub y39: u8,
    pub y0: u8,
    pub y1: u8,
    pub y2: u8,
}

/// The lead-in's Ys, of the line's 40.
const fn lead_ys(ys: &[u8; 40]) -> LeadYs {
    LeadYs {
        y39: ys[39],
        y0: ys[0],
        y1: ys[1],
        y2: ys[2],
    }
}

/// From the line's end to window 2: the netlist's edges `-7` to `5`, OAM
/// giving sprite 39's Y (from the line before), then 0's, 1's and 2's as
/// the index moves; ceres's reads of sprites 0, 1 and 2 (at `-3`, `1`,
/// `5`).
pub(crate) const fn lead_in(net: &mut NetSide, ceres: &mut CeresSide, line: Line, ys: LeadYs) {
    let LeadYs { y39, y0, y1, y2 } = ys;
    net_edge(net, -7, line, y39);
    net_edge(net, -6, line, y0);
    net_edge(net, -5, line, y0);
    net_edge(net, -4, line, y0);
    net_edge(net, -3, line, y0);
    net_edge(net, -2, line, y1);
    net_edge(net, -1, line, y1);
    net_edge(net, 0, line, y1);
    net_edge(net, 1, line, y1);
    net_edge(net, 2, line, y2);
    net_edge(net, 3, line, y2);
    net_edge(net, 4, line, y2);
    net_edge(net, 5, line, y2);
    // Ceres's reads, at `-3`, `1` and `5`: the two sides do not meet.
    ceres_read(ceres, line, y0);
    ceres_read(ceres, line, y1);
    ceres_read(ceres, line, y2);
}

/// From the line's end, with any Ys for the first sprites and the line
/// before's scan ended with any count: the pair is related at window 2.
pub(crate) const fn lead_in_agrees(
    count: u8,
    not_saving: bool,
    held: u8,
    line: Line,
    ys: LeadYs,
) -> bool {
    if !((count <= SLOTS) & (line.ly < 144)) {
        return true;
    }
    let mut net = net_at_line_end(count, not_saving, held);
    let mut ceres = ceres_at_line_end(held);
    lead_in(&mut net, &mut ceres, line, ys);
    related(&net, &ceres, line, 2)
}

/// From window 39 to mode 3: the netlist's edges `154` to `162`, windows
/// 39 and 40 and the falling edge that ends the scan's last dot (mode 3's
/// first dot is `163` and `164`), OAM still giving sprite 39's Y; ceres
/// has read all 40.
pub(crate) const fn tail(net: &mut NetSide, line: Line, y39: u8) {
    net_window(net, line, 39, y39);
    net_window(net, line, 40, y39);
    net_edge(net, 162, line, y39);
}

/// From a related pair at window 39: the netlist's scan stops, at the
/// last sprite with the stop's flip-flops set, and its slots hold ceres's
/// first ten sprites.
pub(crate) const fn tail_agrees(net: NetSide, ceres: &CeresSide, line: Line) -> bool {
    if !related(&net, ceres, line, 39) {
        return true;
    }
    let y39 = ceres.latches.y;
    let mut net = net;
    tail(&mut net, line, y39);
    let scan = &net.scan;
    !scan.scanning
        & (scan.index == 39)
        & (net.scanned == 39)
        & scan.byba
        & scan.doba
        & !scan.start_scan
        & !scan.anel
        & !scan.ceno
        & (scan.count <= SLOTS)
        & slots_are(&net, ceres, ceres.found)
}

/// A whole scan of `line`, sprite `i`'s Y `ys[i]`: the lead-in, windows 2
/// to 38, the tail. What Lean proves of it (`scan_agrees`): with Kani's
/// three claims, the netlist's slots hold ceres's first ten sprites.
pub(crate) const fn scan(
    count: u8,
    not_saving: bool,
    held: u8,
    line: Line,
    ys: [u8; 40],
) -> (NetSide, CeresSide) {
    let mut net = net_at_line_end(count, not_saving, held);
    let mut ceres = ceres_at_line_end(held);
    lead_in(&mut net, &mut ceres, line, lead_ys(&ys));
    let mut j: u8 = 2;
    while j <= 38 {
        step(&mut net, &mut ceres, line, j, ys[j as usize + 1]);
        j += 1;
    }
    // Sprite 39's Y, as ceres latched it in its last read (`tail_agrees`'s).
    let y39 = ceres.latches.y;
    tail(&mut net, line, y39);
    (net, ceres)
}

/// What the scan gives render after its edge `h` (`NetChip::step_render`,
/// with the LCD on): the line's start over `-6` and `-5`, the stop at
/// `154`, CENO from `-2` to `157` (`background::relation::rest::
/// scan_inputs`, which the pipeline's claims take).
pub(crate) const fn outputs_at(scan: &NetScan, h: i32) -> bool {
    let line_start = scan.line_start();
    (line_start == ((h == -6) | (h == -5)))
        & ((scan.stop_scan() & !line_start) == (h == 154))
        & (scan.ceno == ((h >= -2) & (h <= 157)))
}

/// The netlist's edge `h`, then whether the scan's outputs are as
/// `outputs_at` has them.
const fn edge_out(net: &mut NetSide, h: i32, line: Line, y: u8) -> bool {
    net_edge(net, h, line, y);
    outputs_at(&net.scan, h)
}

/// The lead-in's outputs: from the line's end (`lead_in_agrees`'s), at
/// each of its edges.
pub(crate) const fn lead_in_outputs(
    count: u8,
    not_saving: bool,
    held: u8,
    line: Line,
    ys: LeadYs,
) -> bool {
    if !((count <= SLOTS) & (line.ly < 144)) {
        return true;
    }
    let LeadYs { y39, y0, y1, y2 } = ys;
    let mut net = net_at_line_end(count, not_saving, held);
    let n = &mut net;
    outputs_at(&n.scan, -8)
        & edge_out(n, -7, line, y39)
        & edge_out(n, -6, line, y0)
        & edge_out(n, -5, line, y0)
        & edge_out(n, -4, line, y0)
        & edge_out(n, -3, line, y0)
        & edge_out(n, -2, line, y1)
        & edge_out(n, -1, line, y1)
        & edge_out(n, 0, line, y1)
        & edge_out(n, 1, line, y1)
        & edge_out(n, 2, line, y2)
        & edge_out(n, 3, line, y2)
        & edge_out(n, 4, line, y2)
        & edge_out(n, 5, line, y2)
}

/// A window's outputs, `2` to `38`: from a related pair, at each of its
/// edges.
pub(crate) const fn window_outputs(
    net: NetSide,
    ceres: &CeresSide,
    line: Line,
    j: u8,
    y: u8,
) -> bool {
    if !(related(&net, ceres, line, j) & (j >= 2) & (j <= 38)) {
        return true;
    }
    let h = 4 * j as i32 - 2;
    let mut net = net;
    let n = &mut net;
    edge_out(n, h, line, y)
        & edge_out(n, h + 1, line, y)
        & edge_out(n, h + 2, line, y)
        & edge_out(n, h + 3, line, y)
}

/// The tail's outputs: from a pair related at window 39, at each of its
/// edges.
pub(crate) const fn tail_outputs(net: NetSide, ceres: &CeresSide, line: Line) -> bool {
    if !related(&net, ceres, line, 39) {
        return true;
    }
    let y = ceres.latches.y;
    let mut net = net;
    let n = &mut net;
    edge_out(n, 154, line, y)
        & edge_out(n, 155, line, y)
        & edge_out(n, 156, line, y)
        & edge_out(n, 157, line, y)
        & edge_out(n, 158, line, y)
        & edge_out(n, 159, line, y)
        & edge_out(n, 160, line, y)
        & edge_out(n, 161, line, y)
        & edge_out(n, 162, line, y)
}

/// The scan after its stop (`tail_agrees`'s): at the last sprite, the
/// stop's flip-flops set, nothing scanning.
pub(crate) const fn scan_ended(scan: &NetScan) -> bool {
    !scan.scanning
        & (scan.index == 39)
        & scan.byba
        & scan.doba
        & !scan.start_scan
        & !scan.anel
        & !scan.ceno
        & (scan.count <= SLOTS)
}

/// The scan's edge at half dot `h` alone, OAM giving `y` (`net_edge`'s
/// first part): what it does in mode 3 and HBlank, where OAM's latches are
/// the sprite fetch's.
pub(crate) const fn scan_edge_at(scan: &mut NetScan, h: i32, line: Line, y: u8) {
    let clocks = Clocks {
        rising: h & 1 != 0,
        wuvu_before: wuvu(h - 1),
        wuvu_after: wuvu(h),
        wosu_before: wosu(h - 1),
        vclk: (h >= -7) & (h <= 0),
        ly: line.ly,
    };
    scan.edge(clocks, y, line.tall);
}

/// An edge of mode 3 or HBlank, `163` to `904` (the line's end, the next
/// line's `-8`), from the scan ended, OAM giving any Y: the scan stays
/// ended, and gives render no line start, no stop and CENO low.
pub(crate) const fn ended_agrees(scan: NetScan, h: i32, line: Line, y: u8) -> bool {
    if !(scan_ended(&scan) & (h >= 163) & (h <= 904)) {
        return true;
    }
    let mut scan = scan;
    scan_edge_at(&mut scan, h, line, y);
    scan_ended(&scan) & !scan.line_start() & !scan.stop_scan() & !scan.ceno
}

/// The scan's flip-flops that make its outputs, after the edge `h` of a
/// visible line's scan (`-8` to `162`), as an exact shape: ended at the
/// line's end and from `158`; CATU over `-6` to `1` and ANEL over `-4` to
/// `3` (the line's start between them); scanning from `-6` to `153`, CENO
/// from `-2`; the index at sprite `(h + 6) / 4` (0 at the start, 39 from
/// `150`); BYBA from `154`, DOBA a half dot later; the count any of the
/// ten. Measured on `NetChip`'s runs, proved edge by edge (`flags_agrees`).
pub(crate) const fn flags_at(scan: &NetScan, h: i32) -> bool {
    if (h <= -7) | (h >= 158) {
        return scan_ended(scan);
    }
    let index = if h <= -3 {
        0
    } else if h >= 150 {
        39
    } else {
        index_at(h)
    };
    (scan.start_scan == (h <= 1))
        & (scan.anel == ((h >= -4) & (h <= 3)))
        & (scan.scanning == (h <= 153))
        & (scan.ceno == (h >= -2))
        & (scan.index == index)
        & (scan.byba == (h >= 154))
        & (scan.doba == (h >= 155))
        & (scan.count <= SLOTS)
}

/// The scan's index after the edge `h`, from `-2` to `149`.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "from -2 to 149, `h + 6` over 4 is 1 to 38"
)]
const fn index_at(h: i32) -> u8 {
    ((h + 6) >> 2) as u8
}

/// An edge of the scan, `h + 1` (`-7` to `162`), from its flip-flops at
/// `h`, OAM giving any Y: they are at `h + 1`, and its outputs to render
/// are `outputs_at`'s.
pub(crate) const fn flags_agrees(net: NetSide, h: i32, line: Line, y: u8) -> bool {
    if !((h >= -8) & (h <= 161) & (line.ly < 144) & flags_at(&net.scan, h)) {
        return true;
    }
    let mut net = net;
    net_edge(&mut net, h + 1, line, y);
    outputs_at(&net.scan, h + 1) & flags_at(&net.scan, h + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small xorshift.
    #[expect(clippy::cast_possible_truncation, reason = "the low byte")]
    fn next(x: &mut u32) -> u8 {
        *x ^= *x << 13;
        *x ^= *x >> 17;
        *x ^= *x << 5;
        *x as u8
    }

    /// Whole scans with random Ys, many on the line: the pair is related
    /// after the lead-in and after every step, and the tail ends it.
    /// The scan's flip-flops follow `flags_at` edge by edge, whatever OAM
    /// gives, from the line's end to its last edge.
    #[test]
    fn flags_follow_the_scan() {
        let mut x = 11_u32;
        for _ in 0..2_000_u32 {
            let line = Line {
                ly: next(&mut x) % 144,
                tall: next(&mut x) & 1 != 0,
            };
            let mut net = net_at_line_end(next(&mut x) % 11, next(&mut x) & 1 != 0, next(&mut x));
            for h in -8..162 {
                let y = next(&mut x);
                assert!(flags_agrees(net, h, line, y), "h {h}");
                net_edge(&mut net, h + 1, line, y);
            }
            assert!(flags_at(&net.scan, 162));
        }
    }

    #[test]
    fn scans_stay_related() {
        let mut x = 7_u32;
        for run in 0..20_000_u32 {
            let ly = next(&mut x) % 144;
            let tall = next(&mut x) & 1 != 0;
            let line = Line { ly, tall };
            let mut ys = [0_u8; 40];
            for y in &mut ys {
                // On the line about half the time.
                *y = if next(&mut x) & 1 != 0 {
                    ly.wrapping_add(16).wrapping_sub(next(&mut x) % 16)
                } else {
                    next(&mut x)
                };
            }
            let count = next(&mut x) % 11;
            let not_saving = next(&mut x) & 1 != 0;
            let held = next(&mut x);
            assert!(lead_in_agrees(count, not_saving, held, line, lead_ys(&ys)));
            assert!(lead_in_outputs(count, not_saving, held, line, lead_ys(&ys)));
            let mut net = net_at_line_end(count, not_saving, held);
            let mut ceres = ceres_at_line_end(held);
            lead_in(&mut net, &mut ceres, line, lead_ys(&ys));
            assert!(
                related(&net, &ceres, line, 2),
                "run {run}: after the lead-in"
            );
            for j in 2..39_u8 {
                assert!(step_agrees(net, ceres, line, j, ys[usize::from(j) + 1]));
                assert!(window_outputs(net, &ceres, line, j, ys[usize::from(j) + 1]));
                step(&mut net, &mut ceres, line, j, ys[usize::from(j) + 1]);
                assert!(
                    related(&net, &ceres, line, j + 1),
                    "run {run}: at window {}",
                    j + 1
                );
            }
            assert!(tail_agrees(net, &ceres, line), "run {run}: the tail");
            assert!(
                tail_outputs(net, &ceres, line),
                "run {run}: the tail's outputs"
            );
            // The whole scan is these steps.
            let (whole_net, whole_ceres) = scan(count, not_saving, held, line, ys);
            tail(&mut net, line, ys[39]);
            assert!(whole_net.slots == net.slots && whole_ceres.first == ceres.first);
            // Ended, the scan gives render nothing through mode 3 and HBlank.
            assert!(scan_ended(&net.scan));
            for h in 163..904 {
                assert!(ended_agrees(net.scan, h, line, ys[39]));
            }
            // And ceres's list is the whole-scan model's.
            let mut first = [None; 10];
            let mut found = 0;
            for (index, &y) in ys.iter().enumerate() {
                if on_line(y, ly, tall) && found < 10 {
                    #[expect(clippy::cast_possible_truncation, reason = "40 sprites")]
                    let index = index as u8;
                    first[found] = Some(index);
                    found += 1;
                }
            }
            assert_eq!((ceres.first, ceres.found), (first, found), "run {run}");
        }
    }
}
