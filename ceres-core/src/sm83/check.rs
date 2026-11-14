//! What ceres's CPU and the netlist's core (`ceres_netlist::sm83`) are
//! compared by: the core's M-cycle on a memory, and the relation between
//! the two where an instruction ends.
//!
//! The core does an instruction's last work while it fetches the next
//! opcode, so an instruction ends, for the comparison, with the M-cycle
//! that fetches the next one: ceres executes the opcode it holds and then
//! reads the next (`Sm83::exec`, then the read `Sm83::step` starts with),
//! and the core runs as many M-cycles from the one after the first opcode's
//! fetch. Each M-cycle's access must be the same, and `boundary` must hold
//! again after the last.
//!
//! The interrupt requests (IF's lines) are an input, which changes only as
//! phase E starts (`Net::int`). While the core holds `halt_n` low, the
//! chip stops its clocks, as `NetChip` does; the core is neither stopped
//! nor in reset.

#![cfg_attr(
    not(any(kani, test)),
    expect(dead_code, reason = "only the proofs and the co-simulation run these")
)]
#![expect(
    clippy::needless_bitwise_bool,
    reason = "a relation for Kani: a lazy && or || is a branch for the solver"
)]

use {
    super::Sm83,
    ceres_netlist::sm83::{CoreInputs, CoreOutputs, NetCore, Register},
};

/// The phases of an M-cycle (A to H), and those from which the clocks are
/// up: `data_phase` from E, `write_phase` from G; `pch_phase` in B to F.
pub(super) const PHASES: u8 = 8;
const PHASE_E: u8 = 4;
const PHASE_G: u8 = 6;

/// What an M-cycle does on the bus.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Access {
    Idle,
    Read(u16, u8),
    Write(u16, u8),
}

/// The core's inputs in a phase, with `d` on the chip's data bus, `int` on
/// IF's lines and the core's clocks running if `clk_ena` (the core's
/// `halt_n` as the phase before left it): the oscillator stable, no reset,
/// and an address the chip does not decode.
// @hw die: the clocks' BUVU BYXO BEDO, BELU BYRY BUDE BEVA, BAPY BERU BUFA BYLY, BATE BASU BUKE
pub(super) const fn inputs(phase: u8, d: u8, int: u8, clk_ena: bool) -> CoreInputs {
    let g_h_or_a = (phase == 0) | (phase >= PHASE_G);
    CoreInputs {
        clk: phase == 0,
        exec_phase: clk_ena & (phase != 0),
        data_phase: clk_ena & (phase >= PHASE_E),
        write_phase: clk_ena & (phase >= PHASE_G),
        pch_phase: !clk_ena | !g_h_or_a,
        sys_reset: false,
        pwron_reset: false,
        clk_ready: true,
        wake: false,
        oe_n: false,
        internal_access: false,
        shadow_access: false,
        shadow_override: false,
        int,
        d,
    }
}

/// The phases whose clocks are the phase before's: C and D (B's), F (E's)
/// and H (G's). The clocks change only as A, B, E and G start.
pub(super) const fn repeats_clocks(phase: u8) -> bool {
    matches!(phase, 2 | 3 | 5 | 7)
}

/// The netlist's core, with what the memory left on the data bus.
pub(super) struct Net {
    pub core: NetCore,
    /// The bits the memory pulled low in the phase before: they stay a
    /// while after the edge (`NetChip::data_low`).
    pub low_before: u8,
    /// Each settle is confirmed by a second pass, and each phase passed
    /// over is settled after all (the co-simulation). The proofs settle in
    /// one pass, and pass over the phases with nothing new, which the
    /// `netlist_sm83_settles_*` harnesses show is enough: each pass costs
    /// Kani about as much as the next.
    pub confirm: bool,
    /// IF's lines from phase E of the next M-cycle on, and before it (A to
    /// D, what they were from the E before).
    pub int: u8,
    pub int_before: u8,
    /// The interrupts the core acknowledged in the last M-cycle.
    pub acks: u8,
    /// The core's `halt_n` as the last phase left it: the chip runs the
    /// core's clocks while it is high. Unless `may_halt`, the core must keep
    /// it high, and the clocks run (which the proofs then fold).
    pub halt_n: bool,
    pub may_halt: bool,
}

impl Net {
    /// One pass with `i`, and, if confirmed, a second, which must change
    /// neither the core nor what it gives out.
    fn settle(&mut self, i: &CoreInputs, kept: u8, settled: &mut bool) -> CoreOutputs {
        let out = self.core.settle_once(i, kept);
        if self.confirm {
            let mut again = self.core;
            let out_again = again.settle_once(i, kept);
            *settled &= again.settled(&self.core) & (out_again == out);
        }
        out
    }

    /// An M-cycle, and its access: the read as phase H has it (the model's
    /// `BUS_TRACE`), the write as its last phase has it. `read` is the
    /// memory, which drives the bus while the core reads; the core is
    /// settled twice a phase while it reads, as `NetChip` does, the second
    /// time with what the memory drives. A phase whose clocks are the phase before's
    /// (`repeats_clocks`) finds the core settled for them, with the bus the
    /// phase before left, and gives out what that one did. `None` if the
    /// bus is not the phase before's there, or a confirmed settle took more
    /// than one pass.
    pub(super) fn mcycle(&mut self, mut read: impl FnMut(u16) -> u8) -> Option<Access> {
        let mut access = Access::Idle;
        let mut settled = true;
        let mut last = (0xFF, CoreOutputs::default(), self.halt_n);
        self.acks = 0;
        for phase in 0..PHASES {
            let kept = self.core.db;
            let int = if phase < PHASE_E {
                self.int_before
            } else {
                self.int
            };
            let clk_ena = self.halt_n;
            let stale = inputs(phase, !self.low_before, int, clk_ena);
            self.core.edges(&stale);
            if repeats_clocks(phase) & (clk_ena == last.2) {
                settled &= stale.d == last.0;
                let out = last.1;
                if self.confirm {
                    let mut again = self.core;
                    let out_again = again.settle_once(&stale, kept);
                    settled &= again.settled(&self.core) & (out_again == out);
                }
                self.acks |= out.inta;
                if out.wr {
                    access = Access::Write(out.addr, out.db);
                } else if (phase == PHASES - 1) & out.rd {
                    access = Access::Read(out.addr, out.db);
                } else {
                    // Nothing on the bus in this phase.
                }
                continue;
            }
            let first = self.settle(&stale, kept, &mut settled);
            let now_low = if first.rd { !read(first.addr) } else { 0 };
            let driven = inputs(phase, !(now_low | self.low_before), int, clk_ena);
            // While the core reads, settled again with what the memory
            // drives (a pass that changes nothing if the bus had it
            // already); otherwise the bus is as it was.
            let out = if first.rd {
                self.settle(&driven, kept, &mut settled)
            } else {
                first
            };
            self.low_before = now_low;
            self.halt_n = if self.may_halt {
                out.halt_n
            } else {
                assert!(out.halt_n, "the core does not halt");
                true
            };
            last = (driven.d, out, clk_ena);
            self.acks |= out.inta;
            if out.wr {
                access = Access::Write(out.addr, out.db);
            } else if (phase == PHASES - 1) & out.rd {
                access = Access::Read(out.addr, out.db);
            } else {
                // Nothing on the bus in this phase.
            }
        }
        self.int_before = self.int;
        settled.then_some(access)
    }
}

/// A pair of registers holds `value`: the high one's master and `low`, the
/// low one's master (or the flags', for F). Their slaves show them once
/// the write enables fall, in the next M-cycle's first phase.
const fn pair_holds(high: Register, low: u8, value: u16) -> bool {
    u16::from_le_bytes([low, high.master]) == value
}

/// The core holds ceres's registers and IME.
///
/// IME is compared as its latch holds it: EI's is set already, though it
/// counts an instruction later (ceres's `ime_toggle`). What the core shows
/// of it (`seq.ime`) is the latch as the M-cycle before found it.
pub(super) const fn registers_agree(core: &NetCore, cpu: &Sm83) -> bool {
    registers_agree_at(core, cpu, cpu.pc)
}

/// The same, with PC `pc`.
const fn registers_agree_at(core: &NetCore, cpu: &Sm83, pc: u16) -> bool {
    let regs = &core.regs;
    pair_holds(regs.a, core.alu.masters.bits(), cpu.af)
        & pair_holds(regs.b, regs.c.master, cpu.bc)
        & pair_holds(regs.d, regs.e.master, cpu.de)
        & pair_holds(regs.h, regs.l.master, cpu.hl)
        & pair_holds(regs.sph, regs.spl.master, cpu.sp)
        & pair_holds(regs.pch, regs.pcl.master, pc)
        & (core.seq.ime_state == (cpu.ime ^ cpu.ime_toggle))
}

/// The core's control state where a fetch's M-cycle ends, whether it
/// fetched an opcode, began an interrupt's entry or ended HALT's: nothing
/// else under way, no NMI, IE showing its master. The interrupts' request
/// latches are free: they take IF's lines again in the next M-cycle; so is
/// HALT's delayed step, which the next A edge overwrites before it is read,
/// and the HALT step sampled, which the callers pin.
const fn after_fetch(core: &NetCore) -> bool {
    let s = &core.seq;
    let sampled = &core.sampled;
    (s.mcyc <= 7)
        & !s.cb_next
        & !s.halt
        & !s.stop
        & !s.stop_delayed
        & !s.not_ready_1
        & !s.not_ready_2
        & s.no_nmi
        & s.no_nmi_1
        & !s.int_entry
        & s.nmi_low_1
        & s.nmi_low_2
        & !s.nmi_req
        & !s.nmi_entering
        & !s.nmi_entry
        & !core.alu.cond_fail
        & (sampled.int_pending == (core.ints.pending != 0))
        & (core.ints.ie == core.ints.ie_next)
        & !core.clk
        & core.exec_phase
        & core.fetch
        & (sampled.next_mcyc == 0)
        & !sampled.mread
        & !sampled.stop
        & !sampled.not_ready
        & !sampled.nmi
}

/// HALT's step was sampled with a request latched, which will reset the
/// halt it sets (the HALT bug), or not at all.
const fn no_halt(core: &NetCore) -> bool {
    !core.sampled.halt_step | core.sampled.int_pending
}

/// A request latched with IME set, and shown set: the core enters.
const fn takes_interrupt(core: &NetCore) -> bool {
    core.sampled.int_pending & core.seq.ime_state & core.seq.ime
}

/// The core's control state where an instruction ends: phase H of an
/// M-cycle that fetched `op`, which its opcode register shows next, and no
/// interrupt taken.
pub(super) const fn core_between(core: &NetCore, op: u8) -> bool {
    after_fetch(core)
        & no_halt(core)
        & (core.seq.ir_next == op)
        & !core.seq.entering
        & !takes_interrupt(core)
}

/// The core where an interrupt's entry begins: phase H of the M-cycle
/// that fetched, as ceres's dispatch reads, at PC, and the core took the
/// request.
pub(super) const fn core_entering(core: &NetCore) -> bool {
    after_fetch(core) & no_halt(core) & core.seq.entering & takes_interrupt(core)
}

/// The core where HALT's M-cycle ends and it halts: its step sampled, no
/// request latched to reset the halt it sets at the next A edge, and IME
/// shown as its latch holds it (`exec_phase` fell at the M-cycle's end).
pub(super) const fn core_halting(core: &NetCore) -> bool {
    after_fetch(core)
        & core.sampled.halt_step
        & !core.sampled.int_pending
        & !core.seq.entering
        & (core.seq.ime == core.seq.ime_state)
}

/// The core with its halt set, where a phase H ends: its clocks stopped
/// (`exec_phase` low, no fetch), its M-cycle counter at 7 (at 0 in the
/// first halted M-cycle, HALT's step still delayed) and going to 7,
/// nothing else under way, IME shown as its latch holds it (as HALT's
/// M-cycle ended; nothing changes them while halted). With the clocks
/// stopped `data_phase` stays low, so the request latch is open the whole
/// M-cycle and holds what ceres's halted step sampled.
const fn halted_shape(core: &NetCore) -> bool {
    let s = &core.seq;
    let sampled = &core.sampled;
    s.halt
        & ((s.mcyc == 7) | ((s.mcyc == 0) & s.halt_delayed))
        & (s.ime == s.ime_state)
        & !s.cb_next
        & !s.stop
        & !s.stop_delayed
        & !s.not_ready_1
        & !s.not_ready_2
        & s.no_nmi
        & s.no_nmi_1
        & !s.entering
        & !s.int_entry
        & s.nmi_low_1
        & s.nmi_low_2
        & !s.nmi_req
        & !s.nmi_entering
        & !s.nmi_entry
        & !core.alu.cond_fail
        & (sampled.int_pending == (core.ints.pending != 0))
        & (core.ints.ie == core.ints.ie_next)
        & !core.clk
        & !core.exec_phase
        & !core.fetch
        & (sampled.next_mcyc == 7)
        & !sampled.mread
        & !sampled.halt_step
        & !sampled.stop
        & !sampled.not_ready
        & !sampled.nmi
}

/// The core halted, no request latched.
pub(super) const fn core_halted(core: &NetCore) -> bool {
    halted_shape(core) & !core.sampled.int_pending
}

/// The core halted with a request latched, which resets the halt at the
/// next A edge: it wakes.
pub(super) const fn core_waking(core: &NetCore) -> bool {
    halted_shape(core) & core.sampled.int_pending
}

/// The two between instructions: ceres is about to execute `op`, which the
/// core has just fetched.
pub(super) const fn boundary(core: &NetCore, cpu: &Sm83, op: u8) -> bool {
    core_between(core, op) & registers_agree(core, cpu)
}

/// The two where an interrupt's entry begins: ceres has done its
/// dispatch's first read, at PC, and the core its fetch there, which moved
/// its PC on: `cpu` is ceres with PC one on (as ceres has PC after the HALT
/// bug, whose fetch does not move it).
pub(super) const fn entry_begins(core: &NetCore, cpu: &Sm83) -> bool {
    core_entering(core) & registers_agree(core, cpu)
}

/// The same where ceres is about to dispatch (its step's `begin` chose
/// to): PC is where its dispatch reads, one before the core's but after the
/// HALT bug.
pub(super) const fn entry_from(core: &NetCore, cpu: &Sm83) -> bool {
    let pc = cpu.pc.wrapping_add(!cpu.halt_bug as u16);
    core_entering(core) & registers_agree_at(core, cpu, pc)
}

/// The two where HALT's M-cycle ends and both halt: ceres has executed
/// HALT, PC past it.
pub(super) const fn halt_begins(core: &NetCore, cpu: &Sm83) -> bool {
    core_halting(core) & registers_agree(core, cpu)
}

/// The two halted, the core halting (where HALT's M-cycle ended) or
/// halted.
pub(super) const fn in_halt(core: &NetCore, cpu: &Sm83) -> bool {
    (core_halting(core) | core_halted(core)) & registers_agree(core, cpu)
}

/// The two where a halted M-cycle ends with a request: ceres's halted step
/// sampled it, and the core latched it.
pub(super) const fn waking(core: &NetCore, cpu: &Sm83) -> bool {
    core_waking(core) & registers_agree(core, cpu)
}
