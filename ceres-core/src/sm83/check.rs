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
//! phase E starts (`Net::int`); the core is neither halted, stopped nor in
//! reset.

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

/// The core's inputs in a phase, with `d` on the chip's data bus and `int`
/// on IF's lines: the oscillator stable, no reset, and an address the chip
/// does not decode.
pub(super) const fn inputs(phase: u8, d: u8, int: u8) -> CoreInputs {
    CoreInputs {
        clk: phase == 0,
        exec_phase: phase != 0,
        data_phase: phase >= PHASE_E,
        write_phase: phase >= PHASE_G,
        pch_phase: (phase != 0) & (phase < PHASE_G),
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
        let mut last = (0xFF, CoreOutputs::default());
        self.acks = 0;
        for phase in 0..PHASES {
            let kept = self.core.db;
            let int = if phase < PHASE_E {
                self.int_before
            } else {
                self.int
            };
            let stale = inputs(phase, !self.low_before, int);
            self.core.edges(&stale);
            if repeats_clocks(phase) {
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
            let driven = inputs(phase, !(now_low | self.low_before), int);
            // While the core reads, settled again with what the memory
            // drives (a pass that changes nothing if the bus had it
            // already); otherwise the bus is as it was.
            let out = if first.rd {
                self.settle(&driven, kept, &mut settled)
            } else {
                first
            };
            self.low_before = now_low;
            last = (driven.d, out);
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
    let regs = &core.regs;
    pair_holds(regs.a, core.alu.masters.bits(), cpu.af)
        & pair_holds(regs.b, regs.c.master, cpu.bc)
        & pair_holds(regs.d, regs.e.master, cpu.de)
        & pair_holds(regs.h, regs.l.master, cpu.hl)
        & pair_holds(regs.sph, regs.spl.master, cpu.sp)
        & pair_holds(regs.pch, regs.pcl.master, cpu.pc)
        & (core.seq.ime_state == (cpu.ime ^ cpu.ime_toggle))
}

/// The core's control state where an instruction ends: phase H of an
/// M-cycle that fetched `op`, which its opcode register shows next, with no
/// interrupt taken or pending and nothing else under way.
pub(super) const fn core_between(core: &NetCore, op: u8) -> bool {
    let s = &core.seq;
    let sampled = &core.sampled;
    (s.ir_next == op)
        & (s.mcyc <= 7)
        & !s.cb_next
        & !s.halt
        & !s.stop
        & !s.halt_delayed
        & !s.stop_delayed
        & !s.pending_delayed
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
        & (core.ints.pending == 0)
        & !core.clk
        & core.exec_phase
        & core.fetch
        & (sampled.next_mcyc == 0)
        & !sampled.mread
        & !sampled.halt_step
        & !sampled.stop
        & !sampled.int_pending
        & !sampled.not_ready
        & !sampled.nmi
}

/// The two between instructions: ceres is about to execute `op`, which the
/// core has just fetched.
pub(super) const fn boundary(core: &NetCore, cpu: &Sm83, op: u8) -> bool {
    core_between(core, op) & registers_agree(core, cpu)
}
