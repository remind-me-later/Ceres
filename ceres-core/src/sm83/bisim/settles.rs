//! That the netlist's core settles in one pass of each phase
//! (`sm83-settles-in-one-pass`): after the edges, repeating, and driven.

use super::*;

/// The NMI's latches at rest: the DMG's core has no NMI.
pub(super) const fn nmi_at_rest(core: &NetCore) -> bool {
    let s = &core.seq;
    s.no_nmi & s.no_nmi_1 & s.nmi_low_1 & s.nmi_low_2 & !s.nmi_req & !s.nmi_entering & !s.nmi_entry
}
/// What the one-pass lemmas take of the core besides: the NMI at rest;
/// not stopped (`sm83-not-stopped`: no STOP runs); and HALT's step sampled
/// only with no CB prefix pending. That one holds where it matters: the
/// step is sampled in HALT's M-cycle only, which ends where the relations
/// (`relation::in_halt`, `relation::boundary`) pin the prefix clear. (The halt
/// it sets at the next A edge clears the prefix: a second pass there.)
pub(super) const fn at_rest(core: &NetCore) -> bool {
    let s = &core.seq;
    nmi_at_rest(core)
        & !s.stop
        & !s.stop_delayed
        & !core.sampled.stop
        & !(core.sampled.halt_step & s.cb_next)
}
/// From `core`, one pass with `i`: a second changes neither the core nor
/// what it gives out, and the NMI's latches stay at rest.
pub(super) fn one_pass_settles(mut core: NetCore, i: &CoreInputs, kept: u8) {
    let out = core.settle_once(i, kept);
    let mut again = core;
    let out_again = again.settle_once(i, kept);
    assert!(again.settled(&core), "a second pass changes the core");
    assert!(out_again == out, "a second pass changes what it gives out");
    assert!(nmi_at_rest(&core), "the NMI's latches stay at rest");
}
/// From any core settled in the phase before `phase` (any bus, any keeper,
/// any lines from IF, its clocks running or not), the NMI's latches at
/// rest: `phase`'s edges, then one pass with any bus, any lines from IF if
/// `phase` is E (where they change), else the same, and its clocks running
/// or not if `phase` is B, else as they were: the core's `halt_n` changes
/// only in phase A's settle (its latch's set and reset change at the A
/// edge), and the chip's clocks take it from the phase after.
pub(super) fn settles_after_edges(phase: u8) {
    let core = any_core();
    let before = if phase == 0 { PHASES - 1 } else { phase - 1 };
    let (int, clk_ena): (u8, bool) = kani::any();
    kani::assume(settled_in(
        &core,
        before,
        kani::any(),
        kani::any(),
        int,
        clk_ena,
    ));
    kani::assume(at_rest(&core));
    let int = if phase == PHASE_E { kani::any() } else { int };
    let clk_ena = if phase == PHASE_B {
        kani::any()
    } else {
        clk_ena
    };
    let i = inputs(phase, kani::any(), int, clk_ena);
    let kept = core.db;
    let mut core = core;
    core.edges(&i);
    one_pass_settles(core, &i, kept);
}
/// From any core settled in the phase before `phase` with a bus and a
/// keeper, lines from IF and clocks running or not, the NMI's latches at
/// rest, where `phase`'s clocks are that phase's (`relation::repeats_clocks`,
/// and the clocks as they ran): `phase`'s edges (none) and a pass with
/// the same bus, the keeper holding the core's bus, change nothing, and it
/// gives out what the phase before did.
pub(super) fn settles_repeating(phase: u8) {
    let core = any_core();
    let (d, kept, int, clk_ena): (u8, u8, u8, bool) = kani::any();
    kani::assume(settled_in(&core, phase - 1, d, kept, int, clk_ena));
    kani::assume(at_rest(&core));
    let mut before = core;
    let out_before = before.settle_once(&inputs(phase - 1, d, int, clk_ena), kept);
    let i = inputs(phase, d, int, clk_ena);
    let mut again = core;
    again.edges(&i);
    let out = again.settle_once(&i, core.db);
    assert!(again.settled(&core), "the pass changes the core");
    assert!(out == out_before, "it gives out something else");
}
/// From any core settled in `phase` with a bus, a keeper, lines from IF and
/// clocks running or not, the NMI's latches at rest: one pass with another
/// bus (the memory's), the rest the same.
pub(super) fn settles_driven(phase: u8) {
    let core = any_core();
    let (kept, int, clk_ena): (u8, u8, bool) = kani::any();
    kani::assume(settled_in(&core, phase, kani::any(), kept, int, clk_ena));
    kani::assume(at_rest(&core));
    one_pass_settles(core, &inputs(phase, kani::any(), int, clk_ena), kept);
}
/// Phase A's edges, then one pass, settle the core.
// @assumes transcription-sm83
// @discharges sm83-settles-in-one-pass
#[kani::proof]
fn netlist_sm83_settles_after_edges_a() {
    settles_after_edges(0);
}
/// Phase B's edges, then one pass, settle the core.
// @assumes transcription-sm83
// @discharges sm83-settles-in-one-pass
#[kani::proof]
fn netlist_sm83_settles_after_edges_b() {
    settles_after_edges(1);
}
/// Phase C's edges, then one pass, settle the core.
// @assumes transcription-sm83
// @discharges sm83-settles-in-one-pass
#[kani::proof]
fn netlist_sm83_settles_after_edges_c() {
    settles_after_edges(2);
}
/// Phase D's edges, then one pass, settle the core.
// @assumes transcription-sm83
// @discharges sm83-settles-in-one-pass
#[kani::proof]
fn netlist_sm83_settles_after_edges_d() {
    settles_after_edges(3);
}
/// Phase E's edges, then one pass, settle the core.
// @assumes transcription-sm83
// @discharges sm83-settles-in-one-pass
#[kani::proof]
fn netlist_sm83_settles_after_edges_e() {
    settles_after_edges(4);
}
/// Phase F's edges, then one pass, settle the core.
// @assumes transcription-sm83
// @discharges sm83-settles-in-one-pass
#[kani::proof]
fn netlist_sm83_settles_after_edges_f() {
    settles_after_edges(5);
}
/// Phase G's edges, then one pass, settle the core.
// @assumes transcription-sm83
// @discharges sm83-settles-in-one-pass
#[kani::proof]
fn netlist_sm83_settles_after_edges_g() {
    settles_after_edges(6);
}
/// Phase H's edges, then one pass, settle the core.
// @assumes transcription-sm83
// @discharges sm83-settles-in-one-pass
#[kani::proof]
fn netlist_sm83_settles_after_edges_h() {
    settles_after_edges(7);
}
/// Phase C, with the clocks and the bus of the phase before, changes nothing.
// @assumes transcription-sm83
// @discharges sm83-settles-in-one-pass
#[kani::proof]
fn netlist_sm83_settles_repeating_c() {
    settles_repeating(2);
}
/// Phase D, with the clocks and the bus of the phase before, changes nothing.
// @assumes transcription-sm83
// @discharges sm83-settles-in-one-pass
#[kani::proof]
fn netlist_sm83_settles_repeating_d() {
    settles_repeating(3);
}
/// Phase F, with the clocks and the bus of the phase before, changes nothing.
// @assumes transcription-sm83
// @discharges sm83-settles-in-one-pass
#[kani::proof]
fn netlist_sm83_settles_repeating_f() {
    settles_repeating(5);
}
/// Phase H, with the clocks and the bus of the phase before, changes nothing.
// @assumes transcription-sm83
// @discharges sm83-settles-in-one-pass
#[kani::proof]
fn netlist_sm83_settles_repeating_h() {
    settles_repeating(7);
}
/// One pass settles the core in phase A once the memory drives the bus.
// @assumes transcription-sm83
// @discharges sm83-settles-in-one-pass
#[kani::proof]
fn netlist_sm83_settles_driven_a() {
    settles_driven(0);
}
/// One pass settles the core in phase B once the memory drives the bus.
// @assumes transcription-sm83
// @discharges sm83-settles-in-one-pass
#[kani::proof]
fn netlist_sm83_settles_driven_b() {
    settles_driven(1);
}
/// One pass settles the core in phase C once the memory drives the bus.
// @assumes transcription-sm83
// @discharges sm83-settles-in-one-pass
#[kani::proof]
fn netlist_sm83_settles_driven_c() {
    settles_driven(2);
}
/// One pass settles the core in phase D once the memory drives the bus.
// @assumes transcription-sm83
// @discharges sm83-settles-in-one-pass
#[kani::proof]
fn netlist_sm83_settles_driven_d() {
    settles_driven(3);
}
/// One pass settles the core in phase E once the memory drives the bus.
// @assumes transcription-sm83
// @discharges sm83-settles-in-one-pass
#[kani::proof]
fn netlist_sm83_settles_driven_e() {
    settles_driven(4);
}
/// One pass settles the core in phase F once the memory drives the bus.
// @assumes transcription-sm83
// @discharges sm83-settles-in-one-pass
#[kani::proof]
fn netlist_sm83_settles_driven_f() {
    settles_driven(5);
}
/// One pass settles the core in phase G once the memory drives the bus.
// @assumes transcription-sm83
// @discharges sm83-settles-in-one-pass
#[kani::proof]
fn netlist_sm83_settles_driven_g() {
    settles_driven(6);
}
/// One pass settles the core in phase H once the memory drives the bus.
// @assumes transcription-sm83
// @discharges sm83-settles-in-one-pass
#[kani::proof]
fn netlist_sm83_settles_driven_h() {
    settles_driven(7);
}
