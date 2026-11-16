//! Kani proofs that ceres's instructions are the netlist's core's
//! (`cargo kani -p ceres-core`), an instruction class at a time.
//!
//! From every pair of states `relation::boundary` relates, with the core
//! having just fetched an opcode of the class: ceres executes it and reads
//! the next opcode, the core runs as many M-cycles, each M-cycle's access
//! is the same (the memory gives any bytes, the same to both), and
//! `boundary` holds again with the opcode read. IF's lines are anything in
//! each M-cycle (changing as phase E starts): where ceres's next step
//! (`Sm83::begin`) dispatches an interrupt, the core's fetch has begun its
//! entry instead (`relation::entry_from`), and `netlist_sm83_interrupt_entry`
//! takes the entry from there to the vector's opcode. `netlist_sm83_halt`
//! takes HALT and the HALT bug, `netlist_sm83_halted` and
//! `netlist_sm83_halted_from_halt` a halted M-cycle, and `netlist_sm83_wake`
//! the wake-up. The memory keeps ceres's time, as its bus does.
//!
//! The core settles each phase in one pass there (`NetCore::settle_once`).
//! The `netlist_sm83_settles_*` harnesses show that one is enough: from any
//! core settled in the phase before, with the NMI's latches at rest, the
//! edges of a phase and one pass leave a core that a second pass changes
//! in nothing, nor what it gives out, and the latches still at rest; and
//! so from a core settled in a phase once the memory drives the bus. The
//! core between instructions is settled in phase H (an instruction's last
//! phase, by those harnesses), so by induction every phase's one pass is
//! the settle. (A second pass to check each would cost Kani as much again:
//! with it, one M-cycle took 6.7 GB, which left no room for two.) And as
//! the clocks change only as A, B, E and G start, the phases C, D, F and H,
//! with the bus the phase before left, are passed over: from a core settled
//! in the phase before, a pass with the same clocks and bus changes
//! nothing and gives out the same (`netlist_sm83_settles_repeating_*`).

use {
    super::{
        Begin, Bus, Sm83,
        relation::{Access, Net, PHASES, boundary, entry_from, in_halt, inputs, waking},
    },
    ceres_netlist::sm83::{
        CoreInputs, Flags, NetAlu, NetCore, NetCoreInterrupts, NetRegisters, NetSequencer,
        Register, Sampled, decode,
    },
};

mod instructions;
mod interrupts;
mod settles;
/// The most M-cycles an instruction takes with the fetch that ends it.
const MCYCLES: usize = 7;
/// IE's and IF's addresses.
const IE_ADDR: u16 = 0xFFFF;
const IF_ADDR: u16 = 0xFF0F;
/// Phase B, where the core's clocks take a `halt_n` changed in A, and
/// phase E, where IF's lines change.
const PHASE_B: u8 = 1;
const PHASE_E: u8 = 4;
/// HALT.
const HALT: u8 = 0x76;

/// The address of an access, if it has one.
const fn address(access: Access) -> Option<u16> {
    match access {
        Access::Idle => None,
        Access::Read(addr, _) | Access::Write(addr, _) => Some(addr),
    }
}

/// A memory whose reads give any bytes, one an M-cycle, and which
/// remembers each M-cycle's access; with IE, and IF's lines, any in each
/// M-cycle from its phase E on (and before the first). It keeps time as
/// ceres's bus does (`cpu_bus`), as the co-simulation's does: an access
/// flushes the time deferred, happens at its M-cycle's start and defers
/// four dots; `tick` defers four more, `defer` flushes all but some,
/// `advance` moves time on, `drop_deferred` forgets what is deferred, and a
/// step's end flushes it. Each access is in the M-cycle its time falls in,
/// and two in one must be the same (HALT's read, whose time ceres drops,
/// and the fetch after it). At an M-cycle's start ceres finds IF as the
/// lines took it at the E edge before, and once into it, as they took it
/// at its own; it acknowledges in the M-cycle under way.
struct AnyMemory {
    bytes: [u8; MCYCLES],
    trace: [Access; MCYCLES],
    /// The M-cycles that had an access, or the last's index and one.
    count: usize,
    lines: [u8; MCYCLES],
    before: u8,
    ie: u8,
    acked: u8,
    acks: [u8; MCYCLES],
    /// The time, in dots, and the dots deferred.
    time: usize,
    deferred: usize,
}

impl AnyMemory {
    fn new() -> Self {
        Self {
            bytes: kani::any(),
            trace: [Access::Idle; MCYCLES],
            count: 0,
            lines: kani::any(),
            before: kani::any(),
            ie: kani::any(),
            acked: 0,
            acks: [0; MCYCLES],
            time: 0,
            deferred: 0,
        }
    }

    /// IF's lines from phase E of M-cycle `m` on (before the first if
    /// `None`).
    fn line(&self, m: Option<usize>) -> u8 {
        m.map_or(self.before, |m| self.lines.get(m).copied().unwrap_or(0)) & 0x1F
    }

    /// IF where the time is (see the struct).
    fn flags(&self) -> u8 {
        let now = self.time >> 2;
        let at = if self.time & 3 == 0 {
            now.checked_sub(1)
        } else {
            Some(now)
        };
        self.line(at) & !self.acked
    }

    /// `access` in M-cycle `m`.
    fn record(&mut self, m: usize, access: Access) {
        if m < self.count {
            let had = self.trace.get(m).copied().unwrap_or(Access::Idle);
            assert!(had == access, "the accesses in one M-cycle are the same");
        } else {
            if let Some(slot) = self.trace.get_mut(m) {
                *slot = access;
            }
            self.count = m + 1;
        }
    }

    /// An access starts: the time deferred flushed, four dots deferred; its
    /// M-cycle.
    const fn start(&mut self) -> usize {
        self.time += self.deferred;
        self.deferred = 4;
        self.time >> 2
    }

    /// A step's end: the time deferred flushed.
    const fn end_step(&mut self) {
        self.time += self.deferred;
        self.deferred = 0;
    }
}

impl Bus for AnyMemory {
    fn tick(&mut self) {
        let m = (self.time + self.deferred) >> 2;
        self.deferred += 4;
        self.record(m, Access::Idle);
    }

    fn read(&mut self, addr: u16) -> u8 {
        let m = self.start();
        // IE answers at its address, as the core's own register does.
        let value = if addr == IE_ADDR {
            self.ie
        } else {
            self.bytes.get(m).copied().unwrap_or(0)
        };
        self.record(m, Access::Read(addr, value));
        value
    }

    fn write(&mut self, addr: u16, val: u8) {
        if addr == IE_ADDR {
            self.ie = val;
        }
        let m = self.start();
        self.record(m, Access::Write(addr, val));
    }

    fn tick_oam_bug(&mut self, _: u16) {
        let m = self.start();
        self.record(m, Access::Idle);
    }

    fn peek(&self, _: u16) -> u8 {
        kani::any()
    }

    fn flush(&mut self) {}
    fn defer(&mut self, t_cycles: i32) {
        let keep = usize::try_from(t_cycles).unwrap_or(0);
        self.time += self.deferred.saturating_sub(keep);
        self.deferred = keep;
    }
    fn drop_deferred(&mut self) {
        self.deferred = 0;
    }
    fn advance(&mut self, t_cycles: i32) {
        self.time += usize::try_from(t_cycles).unwrap_or(0);
    }
    fn is_cgb_hardware(&self) -> bool {
        false
    }
    fn interrupts_pending(&self) -> bool {
        self.flags() & self.ie & 0x1F != 0
    }
    fn read_if(&self) -> u8 {
        0xE0 | self.flags()
    }
    fn read_ie(&self) -> u8 {
        self.ie
    }
    fn ack_interrupt(&mut self, bit: u8) {
        self.acked |= bit;
        if let Some(slot) = self.acks.get_mut(self.time >> 2) {
            *slot |= bit;
        }
    }
    fn clear_ie(&mut self) {}
    fn trigger_oam_bug(&mut self, _: u16) {}
    fn dma_run(&mut self, _: bool) {}
    fn dma_finish_before_halt(&mut self) {}
    fn tick_hdma(&mut self) {}
    fn hdma_request_pending(&self) -> bool {
        false
    }
    fn note_halt_prefetch(&mut self) {}
    fn set_halted(&mut self, _: bool) {}
    fn is_stopped(&self) -> bool {
        false
    }
    fn enter_stop(&mut self, _: bool) {}
    fn wake_from_stop(&mut self) {}
    fn leave_stop(&mut self) {}
    fn speed_switch_requested(&self) -> bool {
        false
    }
    fn begin_speed_switch(&mut self, _: bool) {}
    fn clear_speed_switch_halt(&mut self) {}
    fn take_unhalt(&mut self) -> bool {
        false
    }
}

/// Any CPU state with the low nibble of F zero, not halted.
fn any_cpu() -> Sm83 {
    Sm83 {
        af: kani::any::<u16>() & 0xFFF0,
        bc: kani::any(),
        de: kani::any(),
        hl: kani::any(),
        sp: kani::any(),
        pc: kani::any(),
        ime: kani::any(),
        ime_toggle: kani::any(),
        ..Sm83::default()
    }
}

fn any_register() -> Register {
    Register {
        q: kani::any(),
        master: kani::any(),
    }
}

fn any_flags() -> Flags {
    Flags::from_bits(kani::any())
}

/// Any state of the core, its decoded steps those of the inputs they are
/// kept for (the core writes the two together).
fn any_core() -> NetCore {
    let (op, m, cb, int, exec): (u8, u8, bool, bool, bool) = kani::any();
    kani::assume(m <= 7);
    NetCore {
        seq: NetSequencer {
            opcode: kani::any(),
            ir_next: kani::any(),
            mcyc: kani::any(),
            sel_reg_w: kani::any(),
            cb_next: kani::any(),
            cb_flag: kani::any(),
            halt: kani::any(),
            stop: kani::any(),
            halt_delayed: kani::any(),
            stop_delayed: kani::any(),
            pending_delayed: kani::any(),
            not_ready_1: kani::any(),
            not_ready_2: kani::any(),
            ime_state: kani::any(),
            ime: kani::any(),
            no_nmi: kani::any(),
            no_nmi_1: kani::any(),
            entering: kani::any(),
            int_entry: kani::any(),
            nmi_low_1: kani::any(),
            nmi_low_2: kani::any(),
            nmi_req: kani::any(),
            nmi_entering: kani::any(),
            nmi_entry: kani::any(),
        },
        regs: NetRegisters {
            a: any_register(),
            b: any_register(),
            c: any_register(),
            d: any_register(),
            e: any_register(),
            h: any_register(),
            l: any_register(),
            w: any_register(),
            z: any_register(),
            sph: any_register(),
            spl: any_register(),
            pch: any_register(),
            pcl: any_register(),
        },
        alu: NetAlu {
            flags: any_flags(),
            masters: any_flags(),
            cond_fail: kani::any(),
            z7_master: kani::any(),
            z7_delayed: kani::any(),
        },
        ints: NetCoreInterrupts {
            ie_next: kani::any(),
            ie: kani::any(),
            pending: kani::any(),
        },
        db: kani::any(),
        clk: kani::any(),
        exec_phase: kani::any(),
        fetch: kani::any(),
        sampled: Sampled {
            next_mcyc: kani::any(),
            mread: kani::any(),
            halt_step: kani::any(),
            stop: kani::any(),
            int_pending: kani::any(),
            not_ready: kani::any(),
            nmi: kani::any(),
        },
        decoded_key: u16::from(op)
            | u16::from(m) << 8
            | u16::from(cb) << 11
            | u16::from(int) << 12
            | u16::from(exec) << 13,
        decoded: decode(op, m, cb, int, exec),
    }
}

/// Any core where a fetch's M-cycle ends, with ceres's `cpu`: `any_core`
/// with what `relation::boundary` and `relation::entry_begins` both pin set
/// rather than assumed (Kani folds what is set, not what is assumed), the
/// rest anything. The interrupts' request latches are free.
fn core_after_fetch_any(cpu: &Sm83) -> NetCore {
    let mut core = any_core();
    let high = |pair: u16| pair.to_le_bytes()[1];
    let low = |pair: u16| pair.to_le_bytes()[0];
    let s = &mut core.seq;
    s.cb_next = false;
    (s.halt, s.stop, s.stop_delayed) = (false, false, false);
    (s.not_ready_1, s.not_ready_2) = (false, false);
    s.ime_state = cpu.ime ^ cpu.ime_toggle;
    (s.no_nmi, s.no_nmi_1, s.nmi_low_1, s.nmi_low_2) = (true, true, true, true);
    (s.nmi_req, s.nmi_entering, s.nmi_entry) = (false, false, false);
    s.int_entry = false;
    let r = &mut core.regs;
    (r.a.master, r.b.master, r.c.master) = (high(cpu.af), high(cpu.bc), low(cpu.bc));
    (r.d.master, r.e.master) = (high(cpu.de), low(cpu.de));
    (r.h.master, r.l.master) = (high(cpu.hl), low(cpu.hl));
    (r.sph.master, r.spl.master) = (high(cpu.sp), low(cpu.sp));
    (r.pch.master, r.pcl.master) = (high(cpu.pc), low(cpu.pc));
    core.alu.masters = Flags::from_bits(low(cpu.af));
    core.alu.cond_fail = false;
    core.ints.ie_next = core.ints.ie;
    (core.clk, core.exec_phase, core.fetch) = (false, true, true);
    core.sampled = Sampled {
        next_mcyc: 0,
        mread: false,
        halt_step: false,
        stop: false,
        int_pending: core.ints.pending != 0,
        not_ready: false,
        nmi: false,
    };
    core
}

/// Any core between instructions, ceres's `cpu` about to execute `op`.
fn core_between_any(cpu: &Sm83, op: u8) -> NetCore {
    let mut core = core_after_fetch_any(cpu);
    core.seq.ir_next = op;
    core.seq.entering = false;
    core
}

/// Any core where an interrupt's entry begins, ceres about to dispatch
/// (PC one on but after the HALT bug).
fn core_entering_any(cpu: &Sm83) -> NetCore {
    let mut core = core_after_fetch_any(cpu);
    let pc = cpu.pc.wrapping_add(u16::from(!cpu.halt_bug));
    let [pcl, pch] = pc.to_le_bytes();
    (core.regs.pch.master, core.regs.pcl.master) = (pch, pcl);
    core.seq.entering = true;
    core.seq.ime = true;
    core
}

/// Any core halting (`core_after_fetch_any`, HALT's step sampled) or
/// halted (its halt set and its clocks stopped), as `relation::in_halt` pins
/// them; the rest anything.
fn core_in_halt_any(cpu: &Sm83, halted: bool) -> NetCore {
    let mut core = core_after_fetch_any(cpu);
    core.seq.entering = false;
    core.seq.halt = halted;
    core.sampled.halt_step = !halted;
    core.sampled.int_pending = false;
    core.ints.pending = 0;
    core.seq.ime = core.seq.ime_state;
    (core.exec_phase, core.fetch) = (!halted, !halted);
    core.sampled.next_mcyc = if halted { 7 } else { 0 };
    core
}

/// The core is settled in `phase` with `d` on the chip's data bus, its
/// keeper holding `kept`, `int` on IF's lines and its clocks running if
/// `clk_ena`: a pass changes nothing.
fn settled_in(core: &NetCore, phase: u8, d: u8, kept: u8, int: u8, clk_ena: bool) -> bool {
    let i = inputs(phase, d, int, clk_ena);
    let mut again = *core;
    _ = again.settle_once(&i, kept);
    (core.clk == i.clk) & (core.exec_phase == i.exec_phase) & again.settled(core)
}

/// The core's M-cycles `mcycles` against ceres's accesses in `memory`,
/// with IF's lines as `memory` has them, less what the core acknowledged:
/// each the same, and the same acknowledge. IE's register drives the bus at
/// its address, not the memory; an access at IF's is the IF block's.
fn core_follows(net: &mut Net, memory: &AnyMemory, mcycles: core::ops::Range<usize>) {
    let mut acked = 0;
    for m in mcycles {
        let byte = memory.bytes.get(m).copied().unwrap_or(0);
        let ceres = memory.trace.get(m).copied().unwrap_or(Access::Idle);
        kani::assume(address(ceres) != Some(IF_ADDR));
        net.int = memory.line(Some(m)) & !acked;
        // Settled in one pass a phase, unconfirmed, and the phases that
        // repeat the clocks of the one before passed over: the bus must
        // repeat too.
        let access = net.mcycle(|addr| if addr == IE_ADDR { 0xFF } else { byte });
        assert!(access.is_some(), "the bus repeats where the clocks do");
        let access = access.unwrap_or(Access::Idle);
        assert!(address(access) == address(ceres), "the same address");
        assert!(access == ceres, "the same access");
        let acks = memory.acks.get(m).copied().unwrap_or(0);
        assert!(net.acks == acks, "the same acknowledge");
        acked |= net.acks;
        net.int_before &= !net.acks;
    }
}

/// An instruction of the class `in_class` (its opcode, and the byte after
/// it, which only the CB prefix's classes look at), which ceres runs in one
/// of `counts` M-cycles with the fetch that ends it: from every related
/// pair whose run takes `mcycles`, the accesses agree, and the pair is
/// related again. A class with one count takes one harness; a conditional
/// one, a harness per count, which together take every run. (The count is
/// given, not taken from ceres's run: the solver would run the core for
/// every count it cannot rule out.)
fn instruction_agrees(in_class: impl Fn(u8, u8) -> bool, counts: &[usize], mcycles: usize) {
    let op: u8 = kani::any();
    let mut memory = AnyMemory::new();
    kani::assume(in_class(op, memory.bytes[0]));
    let mut cpu = any_cpu();
    let mut net = Net {
        core: core_between_any(&cpu, op),
        low_before: kani::any(),
        confirm: false,
        int: 0,
        int_before: memory.line(None),
        acks: 0,
        halt_n: true,
        may_halt: false,
    };
    kani::assume(boundary(&net.core, &cpu, op));
    memory.ie = net.core.ints.ie;
    // Settled in the instruction before's last phase.
    kani::assume(settled_in(
        &net.core,
        PHASES - 1,
        kani::any(),
        kani::any(),
        memory.line(None),
        true,
    ));

    // As `run` does: an EI just before counts from here on (no interrupt is
    // taken here: the core's fetch did not take one).
    cpu.take_ime_toggle();
    cpu.exec(&mut memory, op);
    memory.end_step();
    // Ceres's next step up to its instruction: the fetch, or the dispatch's
    // first read, at PC; the core's fetch took the request, or fetched.
    let begun = cpu.begin(&mut memory);
    if begun == Begin::Dispatch {
        _ = memory.read(cpu.pc);
    }

    assert!(
        counts.contains(&memory.count),
        "ceres takes one of the class's counts"
    );
    kani::assume(memory.count == mcycles);
    core_follows(&mut net, &memory, 0..mcycles);
    match begun {
        Begin::Fetched(next) => assert!(
            boundary(&net.core, &cpu, next),
            "the next instruction begins"
        ),
        Begin::Dispatch => assert!(entry_from(&net.core, &cpu), "an interrupt's entry begins"),
        Begin::Halted => panic!("not halted"),
    }
}
