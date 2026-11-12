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
//! No interrupt is requested here, and the core is neither halted, stopped
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
    ceres_netlist::sm83::{CoreInputs, NetCore, Register},
};

/// The phases of an M-cycle (A to H), and those from which the clocks are
/// up: `data_phase` from E, `write_phase` from G; `pch_phase` in B to F.
const PHASES: u8 = 8;
const PHASE_E: u8 = 4;
const PHASE_G: u8 = 6;
/// The passes in which the core settles in a phase, the one that finds
/// nothing changed among them: with the data bus as the phase before left
/// it, then with what the memory drives.
const PASSES: u32 = 3;
const PASSES_DRIVEN: u32 = 2;

/// What an M-cycle does on the bus.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Access {
    Idle,
    Read(u16, u8),
    Write(u16, u8),
}

/// The core's inputs in a phase, with `d` on the chip's data bus: the
/// oscillator stable, no reset, no interrupt requested, and an address the
/// chip does not decode.
const fn inputs(phase: u8, d: u8) -> CoreInputs {
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
        int: 0,
        d,
    }
}

/// The netlist's core, with what the memory left on the data bus.
pub(super) struct Net {
    pub core: NetCore,
    /// The bits the memory pulled low in the phase before: they stay a
    /// while after the edge (`NetChip::data_low`).
    pub low_before: u8,
}

impl Net {
    /// An M-cycle, and its access: the read as phase H has it (the model's
    /// `BUS_TRACE`), the write as its last phase has it. `read` is the
    /// memory, which drives the bus while the core reads; the core is
    /// settled twice a phase, as `NetChip` does, the second time with what
    /// the memory drives (if that is what the bus had already, the core is
    /// settled as it is). `None` if the core does not settle in `PASSES`
    /// passes, or in `PASSES_DRIVEN` more with the memory's byte.
    pub(super) fn mcycle(&mut self, mut read: impl FnMut(u16) -> u8) -> Option<Access> {
        let mut access = Access::Idle;
        let mut settled = true;
        for phase in 0..PHASES {
            let kept = self.core.db;
            let stale = inputs(phase, !self.low_before);
            self.core.edges(&stale);
            let Some(first) = self.core.settle_within(&stale, kept, PASSES) else {
                settled = false;
                continue;
            };
            let now_low = if first.rd { !read(first.addr) } else { 0 };
            let driven = inputs(phase, !(now_low | self.low_before));
            let out = if driven.d == stale.d {
                first
            } else if let Some(out) = self.core.settle_within(&driven, kept, PASSES_DRIVEN) {
                out
            } else {
                settled = false;
                continue;
            };
            self.low_before = now_low;
            if out.wr {
                access = Access::Write(out.addr, out.db);
            } else if (phase == PHASES - 1) & out.rd {
                access = Access::Read(out.addr, out.db);
            } else {
                // Nothing on the bus in this phase.
            }
        }
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
