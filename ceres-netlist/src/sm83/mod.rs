//! The SM83, the DMG's CPU core (`hdl/rtl/sm83.vhd` and its parts), phase
//! by phase.
//!
//! An M-cycle is the chip's eight phases: the core's flip-flops change at
//! the rise of `clk` (phase A); the decoders and the buses are precharged
//! in A and work from B (`exec_phase`); reads take the data bus in the
//! second half (`data_phase`, E to H); the registers take their new values
//! in `write_phase` (G and H) and show them when their write enables fall,
//! at the M-cycle's end. A phase is settled: the edge that starts it clocks
//! the flip-flops, then the latches and the wires are evaluated until
//! nothing changes, as the VHDL's delta cycles would.
//!
//! The core's data bus (`db`) is precharged in phase A and from B only
//! pulled down: by the chip's data bus (through the bridge of wired ANDs),
//! the ALU, operand 2, PC, SP and IE; a keeper holds it. The chip's bus is
//! pulled down where `db` is 0, so the core's outputs show on it.
//!
//! The core's outputs leave it later than its flip-flops change (14 ns
//! for the control lines, 21 to 32 ns for the address), which the chip's
//! decoding of its request races (`hdl/rtl/dmg_cpu_b.vhd`); a settled
//! phase shows them as they end it.
//!
//! @hw die: dmg-schematics, the SM83 (sm83/)

mod alu;
mod control;
mod decoder;
mod idu;
mod interrupts;
mod registers;
mod sequencer;

pub use {
    alu::{Flags, NetAlu},
    control::{ControlInputs, Controls, control},
    decoder::{Steps, decode},
    interrupts::NetCoreInterrupts,
    registers::{NetRegisters, Register},
    sequencer::{NetSequencer, Sampled},
};

use {alu::AluInputs, idu::idu, registers::RegisterInputs, sequencer::SequencerInputs};

/// What the core takes in during a phase.
#[derive(Clone, Copy, Default)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "the clocks' and pins' levels"
)]
pub struct CoreInputs {
    /// The clocks: `clk_1mhz` (high in A), `exec_phase` (B to H),
    /// `data_phase` (E to H), `write_phase` (`clk_t4`, G and H),
    /// `pch_phase` (B to F).
    pub clk: bool,
    pub exec_phase: bool,
    pub data_phase: bool,
    pub write_phase: bool,
    pub pch_phase: bool,
    /// The resets, the oscillator's being stable, and the joypad's wake-up
    /// (STOP).
    pub sys_reset: bool,
    pub pwron_reset: bool,
    pub clk_ready: bool,
    pub wake: bool,
    /// The test mode T1 alone: the core lets go of the buses.
    pub oe_n: bool,
    /// The chip's decoding of the address for the memory request: FE00 to
    /// FFFF, the boot ROM's, unless in the test mode T2 alone.
    pub internal_access: bool,
    pub shadow_access: bool,
    pub shadow_override: bool,
    /// The interrupt requests (IF).
    pub int: u8,
    /// The chip's data bus: its bits that were 0 in the phase (strongly:
    /// the bridge passes neither a keeper's weak 0 nor an X), else 1.
    pub d: u8,
}

/// What the core gives out as a phase ends.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
#[expect(clippy::struct_excessive_bools, reason = "the control lines")]
pub struct CoreOutputs {
    pub addr: u16,
    /// The decoder's request, and the request to the chip.
    pub ctl_mreq: bool,
    pub mreq: bool,
    pub rd: bool,
    pub wr: bool,
    /// The opcode's fetch.
    pub m1: bool,
    pub halt_n: bool,
    pub stop_n: bool,
    /// The acknowledges.
    pub inta: u8,
    /// The core's data bus: the chip's bus is pulled down where it is 0.
    pub db: u8,
}

/// The core between phases.
#[derive(Clone, Copy)]
pub struct NetCore {
    pub seq: NetSequencer,
    pub regs: NetRegisters,
    pub alu: NetAlu,
    pub ints: NetCoreInterrupts,
    /// The core's data bus, which its keeper holds.
    pub db: u8,
    /// `clk`, `exec_phase` and the fetch in the phase before, and what the
    /// flip-flops clocked by `clk` take at its rise.
    pub clk: bool,
    pub exec_phase: bool,
    pub fetch: bool,
    pub sampled: Sampled,
    /// The decoder's steps, and its inputs they are for (`key`: the
    /// opcode, the M-cycle, the prefix, the entry, `exec_phase`): they
    /// change only at the M-cycle's edges, so the core decodes about once
    /// an M-cycle.
    pub decoded_key: u16,
    pub decoded: Steps,
}

impl NetCore {
    pub const POWER_ON: Self = Self {
        seq: NetSequencer::POWER_ON,
        regs: NetRegisters::POWER_ON,
        alu: NetAlu::POWER_ON,
        ints: NetCoreInterrupts {
            ie_next: 0,
            ie: 0,
            pending: 0,
        },
        db: 0xFF,
        clk: false,
        exec_phase: false,
        fetch: false,
        sampled: Sampled {
            next_mcyc: 0,
            mread: false,
            halt_step: false,
            stop: false,
            int_pending: false,
            not_ready: true,
            nmi: false,
        },
        decoded_key: 0,
        decoded: decode(0, 0, false, false, false),
    };

    /// A phase: the edges that start it (`clk` rising, `exec_phase`
    /// falling), then the phase settled.
    pub fn phase(&mut self, i: &CoreInputs) -> CoreOutputs {
        if i.clk & !self.clk {
            self.seq.clock(self.sampled);
        }
        if !i.exec_phase & self.exec_phase {
            self.seq.exec_falls();
        }
        self.settle(i)
    }

    /// The latches and wires with `i`, until nothing changes.
    pub fn settle(&mut self, i: &CoreInputs) -> CoreOutputs {
        self.clk = i.clk;
        self.exec_phase = i.exec_phase;
        // The keeper holds what the phase before left.
        let kept = self.db;
        loop {
            let was = *self;
            let out = self.evaluate(i, kept);
            if self.settled(&was) {
                return out;
            }
        }
    }

    /// Nothing changed in a pass (the decoded steps follow from their key).
    fn settled(&self, was: &Self) -> bool {
        (self.seq == was.seq)
            & (self.regs == was.regs)
            & (self.alu == was.alu)
            & (self.ints == was.ints)
            & (self.db == was.db)
            & (self.fetch == was.fetch)
            & (self.sampled == was.sampled)
            & (self.decoded_key == was.decoded_key)
    }

    /// One pass over the latches and wires.
    #[expect(
        clippy::too_many_lines,
        reason = "the core's parts in the order they settle"
    )]
    fn evaluate(&mut self, i: &CoreInputs, kept: u8) -> CoreOutputs {
        let (exec, data, write) = (i.exec_phase, i.data_phase, i.write_phase);
        let table_cb = self.seq.table_cb();
        let (op, m, int) = (
            self.seq.opcode,
            self.seq.mcyc,
            self.seq.in_intr(i.sys_reset),
        );
        let key = u16::from(op)
            | u16::from(m) << 8
            | u16::from(table_cb) << 11
            | u16::from(int) << 12
            | u16::from(exec) << 13;
        if key != self.decoded_key {
            self.decoded = decode(op, m, table_cb, int, exec);
            self.decoded_key = key;
        }
        let s = self.decoded;
        let c = control(
            &s,
            ControlInputs {
                op,
                table_cb,
                exec_phase: exec,
                data_phase: data,
                write_phase: write,
                sel_reg_w: self.seq.sel_reg_w,
                sel_reg_z: !self.seq.sel_reg_w,
                z7_delayed: self.alu.z7_delayed,
            },
        );
        // The opcode shows when the fetch ends.
        // @hw die: REG_IR
        if self.fetch & !c.fetch {
            self.seq.opcode = self.seq.ir_next;
        }
        self.fetch = c.fetch;

        self.alu.settle_sign(data, write, self.regs.z.q);
        self.regs.show(&c);
        self.alu.show(&c);
        let buses = self.regs.buses(&s, &c, op, self.alu.flags.bits());
        let alu = self.alu.compute(
            &s,
            &c,
            AluInputs {
                op,
                op1: buses.op1,
                op2: buses.op2,
                reg_a: self.regs.a.q,
                reg_z: self.regs.z.q,
                exec_phase: exec,
                write_phase: write,
            },
        );
        let idu_out = idu(
            &s,
            &c,
            data,
            self.regs.z.q & 0x80 != 0,
            alu.carry & 0x80 != 0,
            buses.idu_in,
        );
        let addr = buses.idu_in;
        self.ints.show(&c, addr);

        // The core's data bus: precharged in phase A, then pulled down by
        // the chip's bus, the ALU, operand 2, PC, SP and IE.
        // @hw die: DBUS_BRIDGE (b2b_wand_inj_a), DBUS_NAND, DBUS_NOT,
        // REG_BUS_PCH_A (a); ZLYZ (oa211)
        let oe = !i.oe_n;
        let rd = (c.fetch | c.mread) & data & oe;
        let all8 = |on: bool| if on { 0xFF_u8 } else { 0 };
        let pulled = (all8(oe) & !i.d)
            | (all8(c.alu_to_db_en) & !alu.res)
            | (all8(c.op2_to_db_en) & !buses.op2)
            | buses.db_pull
            | self.ints.db_pull(rd, addr);
        self.db = if exec { kept & !pulled } else { 0xFF };

        let selected = self.ints.settle(
            &s,
            &c,
            (data, write),
            i.sys_reset,
            addr,
            self.db,
            i.int,
            self.seq.nmi_entry,
        );
        self.regs.take(
            &s,
            &c,
            RegisterInputs {
                op,
                res: alu.res,
                idu_out,
                db: self.db,
                int_vector: selected.int_vector,
                write_phase: write,
                sys_reset: i.sys_reset,
            },
        );
        self.alu.take(&c, write, alu.new);
        self.alu.settle_condition(&c, op, i.pch_phase, write);
        self.seq.settle(
            &s,
            &c,
            SequencerInputs {
                clk: i.clk,
                data_phase: data,
                write_phase: write,
                sys_reset: i.sys_reset,
                pwron_reset: i.pwron_reset,
                wake: i.wake,
                nmi: false,
                db: self.db,
                int_take: selected.int_take,
            },
        );
        // @hw die: WUDZ WEEN WERF, WAGR
        let mcyc7 = self.seq.idle(i.sys_reset) | self.alu.cond_fail;
        self.sampled = Sampled {
            next_mcyc: c.next_mcyc | if mcyc7 { 7 } else { 0 },
            mread: c.mread,
            halt_step: s.halt,
            stop: c.stop,
            int_pending: selected.int_pending,
            not_ready: !i.clk_ready | i.sys_reset,
            nmi: false,
        };

        // The request (unless the address is FE00 to FFFF, or the boot
        // ROM's), the read, the write.
        // @hw die: ZLYZ (oa211); WAFR, WDIN (oa21)
        CoreOutputs {
            addr,
            ctl_mreq: c.mreq,
            mreq: (!i.shadow_access | i.shadow_override) & !i.internal_access & c.mreq,
            rd,
            wr: c.mwrite,
            m1: c.fetch,
            halt_n: !self.seq.halt,
            stop_n: !self.seq.stop,
            inta: selected.inta,
            db: self.db,
        }
    }
}
