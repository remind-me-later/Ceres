//! The CPU's writes and the quiet inputs the relation takes.

#[cfg_attr(
    not(test),
    expect(clippy::wildcard_imports, reason = "the check's parts share its items")
)]
use super::*;

/// The netlist's inputs in mode 3: no reset, no line start, the scan
/// stopped (CENO low), the objects and the window on or off as ceres has
/// them; with the window on, not in VBlank and WX not written.
pub(super) const fn quiet(i: &EdgeInputs, l: &Mode3Loop) -> bool {
    !i.ppu_reset
        & !i.hard_reset
        & !i.line_start
        & !i.stop_scan
        & !i.ceno
        & ((i.lcdc & LCDC_OBJ_EN != 0) == l.objects_enabled)
        & ((i.lcdc & LCDC_WIN_EN != 0) == l.window_enabled)
        & (!l.window_enabled | !i.mode1 & !(i.cpu_wr_sync & (i.addr == WX)))
}

/// The CPU strobes SCX's address in this edge.
pub(super) const fn strobes_scx(i: &EdgeInputs) -> bool {
    i.cpu_wr_sync & (i.addr == SCX)
}

/// A write strobe that rises while the bus is on SCX's address keeps it there
/// (the edge before the strobe carries SCX's address, and 0xFF).
pub(super) const fn keeps_address(before: &EdgeInputs, now: &EdgeInputs) -> bool {
    !(now.cpu_wr_sync & !before.cpu_wr_sync & (before.addr == SCX)) | (now.addr == SCX)
}

/// The write strobe does not rise with the dot's first edge where the
/// netlist's dot clock starts (POKY rises with PYGO, in ceres's third dot of
/// the line, at h = 167): the strobes rise at h = 1 (mod 8).
pub(super) const fn strobe_misses_start(net: &NetPipeline, rising: &EdgeInputs) -> bool {
    !(net.mode3
        & net.fetcher.pory
        & !net.fetcher.started
        & rising.cpu_wr_sync
        & !net.inputs.cpu_wr_sync)
}

/// The CPU writes SCX's low bits with `scx`'s in a dot's edges, if at all:
/// while it strobes SCX's address the data bus carries them, and if they are
/// not the latch's, the strobe comes with the dot's first edge.
pub(super) const fn writes(
    net: &NetPipeline,
    rising: &EdgeInputs,
    falling: &EdgeInputs,
    scx: u8,
) -> bool {
    strobe_misses_start(net, rising)
        & (!strobes_scx(rising) | (rising.data & 7 == scx & 7))
        & (!strobes_scx(falling) | (falling.data & 7 == scx & 7))
        & keeps_address(&net.inputs, rising)
        & keeps_address(rising, falling)
        & ((net.scx & 7 == scx & 7) | strobes_scx(rising))
}

/// SCX's latch holds what the last edge strobed into it.
pub(super) const fn latch_holds(net: &NetPipeline) -> bool {
    !strobes_scx(&net.inputs) | (net.inputs.data == net.scx)
}
