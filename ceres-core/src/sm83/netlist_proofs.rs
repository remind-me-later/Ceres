//! Kani proofs that ceres's instructions are the netlist's core's
//! (`cargo kani -p ceres-core`), an instruction class at a time.
//!
//! From every pair of states `check::boundary` relates, with the core
//! having just fetched an opcode of the class: ceres executes it and reads
//! the next opcode, the core runs as many M-cycles, each M-cycle's access
//! is the same (the memory gives any bytes, the same to both), and
//! `boundary` holds again with the opcode read. IF's lines are anything in
//! each M-cycle (changing as phase E starts): where ceres's next step would
//! dispatch an interrupt, the core's fetch has begun its entry instead
//! (`check::entry_begins`), and `netlist_sm83_interrupt_entry` takes the
//! entry from there to the vector's opcode.
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
        Bus, Sm83,
        check::{Access, Net, PHASES, boundary, entry_begins, inputs},
    },
    ceres_netlist::sm83::{
        CoreInputs, Flags, NetAlu, NetCore, NetCoreInterrupts, NetRegisters, NetSequencer,
        Register, Sampled, decode,
    },
};

/// The most M-cycles an instruction takes with the fetch that ends it.
const MCYCLES: usize = 7;
/// IE's and IF's addresses.
const IE_ADDR: u16 = 0xFFFF;
const IF_ADDR: u16 = 0xFF0F;
/// Phase E, where IF's lines change.
const PHASE_E: u8 = 4;

/// The address of an access, if it has one.
const fn address(access: Access) -> Option<u16> {
    match access {
        Access::Idle => None,
        Access::Read(addr, _) | Access::Write(addr, _) => Some(addr),
    }
}

/// A memory whose reads give any bytes, in order, and which remembers each
/// M-cycle's access; with IE, and IF's lines, any in each M-cycle from its
/// phase E on (and before the first), as the co-simulation has them: at an
/// access's start (where an access leaves ceres's bus) ceres finds IF as
/// the lines took it at the E edge before, and once into the M-cycle
/// (`defer`), as they took it at its own; it acknowledges in the M-cycle
/// under way.
struct AnyMemory {
    bytes: [u8; MCYCLES],
    trace: [Access; MCYCLES],
    count: usize,
    lines: [u8; MCYCLES],
    before: u8,
    ie: u8,
    acked: u8,
    acks: [u8; MCYCLES],
    /// T-cycles into the last access's M-cycle (4: the next's start).
    into: i32,
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
            into: 4,
        }
    }

    /// IF's lines from phase E of M-cycle `m` on (before the first if
    /// `None`).
    fn line(&self, m: Option<usize>) -> u8 {
        m.map_or(self.before, |m| self.lines.get(m).copied().unwrap_or(0)) & 0x1F
    }

    /// IF where the bus is (see the struct).
    fn flags(&self) -> u8 {
        let back = if self.into == 0 { 2 } else { 1 };
        self.line(self.count.checked_sub(back)) & !self.acked
    }

    fn record(&mut self, access: Access) {
        self.into = 0;
        if let Some(slot) = self.trace.get_mut(self.count) {
            *slot = access;
        }
        self.count += 1;
    }
}

impl Bus for AnyMemory {
    fn tick(&mut self) {
        self.record(Access::Idle);
    }

    fn read(&mut self, addr: u16) -> u8 {
        let value = self.bytes.get(self.count).copied().unwrap_or(0);
        self.record(Access::Read(addr, value));
        value
    }

    fn write(&mut self, addr: u16, val: u8) {
        if addr == IE_ADDR {
            self.ie = val;
        }
        self.record(Access::Write(addr, val));
    }

    fn tick_oam_bug(&mut self, _: u16) {
        self.record(Access::Idle);
    }

    fn peek(&self, _: u16) -> u8 {
        kani::any()
    }

    fn flush(&mut self) {}
    fn defer(&mut self, t_cycles: i32) {
        self.into = 4 - t_cycles;
    }
    fn drop_deferred(&mut self) {}
    fn advance(&mut self, _: i32) {}
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
        if let Some(slot) = self.count.checked_sub(1).and_then(|m| self.acks.get_mut(m)) {
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
/// with what `check::boundary` and `check::entry_begins` both pin set
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

/// Any core where an interrupt's entry begins.
fn core_entering_any(cpu: &Sm83) -> NetCore {
    let mut core = core_after_fetch_any(cpu);
    core.seq.entering = true;
    core.seq.ime = true;
    core
}

/// The NMI's latches at rest: the DMG's core has no NMI.
const fn nmi_at_rest(core: &NetCore) -> bool {
    let s = &core.seq;
    s.no_nmi & s.no_nmi_1 & s.nmi_low_1 & s.nmi_low_2 & !s.nmi_req & !s.nmi_entering & !s.nmi_entry
}

/// The core is settled in `phase` with `d` on the chip's data bus, its
/// keeper holding `kept` and `int` on IF's lines: a pass changes nothing.
fn settled_in(core: &NetCore, phase: u8, d: u8, kept: u8, int: u8) -> bool {
    let i = inputs(phase, d, int, true);
    let mut again = *core;
    _ = again.settle_once(&i, kept);
    (core.clk == i.clk) & (core.exec_phase == i.exec_phase) & again.settled(core)
}

/// From `core`, one pass with `i`: a second changes neither the core nor
/// what it gives out, and the NMI's latches stay at rest.
fn one_pass_settles(mut core: NetCore, i: &CoreInputs, kept: u8) {
    let out = core.settle_once(i, kept);
    let mut again = core;
    let out_again = again.settle_once(i, kept);
    assert!(again.settled(&core), "a second pass changes the core");
    assert!(out_again == out, "a second pass changes what it gives out");
    assert!(nmi_at_rest(&core), "the NMI's latches stay at rest");
}

/// From any core settled in the phase before `phase` (any bus, any keeper,
/// any lines from IF), the NMI's latches at rest: `phase`'s edges, then one
/// pass with any bus, and any lines from IF if `phase` is E (where they
/// change), else the same.
fn settles_after_edges(phase: u8) {
    let core = any_core();
    let before = if phase == 0 { PHASES - 1 } else { phase - 1 };
    let int: u8 = kani::any();
    kani::assume(settled_in(&core, before, kani::any(), kani::any(), int));
    kani::assume(nmi_at_rest(&core));
    let int = if phase == PHASE_E { kani::any() } else { int };
    let i = inputs(phase, kani::any(), int, true);
    let kept = core.db;
    let mut core = core;
    core.edges(&i);
    one_pass_settles(core, &i, kept);
}

/// From any core settled in the phase before `phase` with a bus and a
/// keeper and lines from IF, the NMI's latches at rest, where `phase`'s
/// clocks are that phase's (`check::repeats_clocks`): `phase`'s edges (none) and a pass with
/// the same bus, the keeper holding the core's bus, change nothing, and it
/// gives out what the phase before did.
fn settles_repeating(phase: u8) {
    let core = any_core();
    let (d, kept, int): (u8, u8, u8) = kani::any();
    kani::assume(settled_in(&core, phase - 1, d, kept, int));
    kani::assume(nmi_at_rest(&core));
    let mut before = core;
    let out_before = before.settle_once(&inputs(phase - 1, d, int, true), kept);
    let i = inputs(phase, d, int, true);
    let mut again = core;
    again.edges(&i);
    let out = again.settle_once(&i, core.db);
    assert!(again.settled(&core), "the pass changes the core");
    assert!(out == out_before, "it gives out something else");
}

/// From any core settled in `phase` with a bus, a keeper and lines from
/// IF, the NMI's latches at rest: one pass with another bus (the memory's),
/// the keeper and the lines the same.
fn settles_driven(phase: u8) {
    let core = any_core();
    let (kept, int): (u8, u8) = kani::any();
    kani::assume(settled_in(&core, phase, kani::any(), kept, int));
    kani::assume(nmi_at_rest(&core));
    one_pass_settles(core, &inputs(phase, kani::any(), int, true), kept);
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
    ));

    // As `run` does: an EI just before counts from here on (no interrupt is
    // taken here: the core's fetch did not take one).
    cpu.take_ime_toggle();
    cpu.exec(&mut memory, op);
    let next = memory.read(cpu.pc);
    cpu.pc = cpu.pc.wrapping_add(1);

    assert!(
        counts.contains(&memory.count),
        "ceres takes one of the class's counts"
    );
    kani::assume(memory.count == mcycles);
    for m in 0..mcycles {
        let (byte, ceres) = (memory.bytes[m], memory.trace[m]);
        // IE's address is the core's own register's, not the memory's, and
        // IF's is the IF block's.
        kani::assume(address(ceres) != Some(IE_ADDR));
        kani::assume(address(ceres) != Some(IF_ADDR));
        net.int = memory.line(Some(m));
        // Settled in one pass a phase, unconfirmed, and the phases that
        // repeat the clocks of the one before passed over: the bus must
        // repeat too.
        let access = net.mcycle(|_| byte);
        assert!(access.is_some(), "the bus repeats where the clocks do");
        let access = access.unwrap_or(Access::Idle);
        assert!(address(access) == address(ceres), "the same address");
        assert!(access == ceres, "the same access");
        assert!(net.acks == 0, "no acknowledge in an instruction");
    }
    // Ceres's next step dispatches if IME was set before its EI's toggle,
    // and IF and IE share a request as the fetch's E edge finds them. Its
    // first M-cycle is the fetch's read, and the core's fetch took the
    // request, or fetched.
    let found = memory.line(mcycles.checked_sub(2));
    if cpu.ime & (found & memory.ie != 0) {
        assert!(entry_begins(&net.core, &cpu), "an interrupt's entry begins");
    } else {
        assert!(
            boundary(&net.core, &cpu, next),
            "the next instruction begins"
        );
    }
}

/// An interrupt's entry, from where it begins (`check::entry_begins`): ceres's
/// dispatch after its first read, and the fetch of the vector's opcode,
/// against the core's next five M-cycles, with any lines from IF and the
/// pushes anywhere but IF (they may write IE, which can take the request
/// away: then neither acknowledges, and both jump to 0). The accesses and
/// the acknowledges agree, and `boundary` holds with the opcode read.
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-halted
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_interrupt_entry() {
    let mut memory = AnyMemory::new();
    let mut cpu = any_cpu();
    let mut net = Net {
        core: core_entering_any(&cpu),
        low_before: kani::any(),
        confirm: false,
        int: 0,
        int_before: memory.line(Some(0)),
        acks: 0,
        halt_n: true,
    };
    kani::assume(entry_begins(&net.core, &cpu));
    memory.ie = net.core.ints.ie;
    kani::assume(settled_in(
        &net.core,
        PHASES - 1,
        kani::any(),
        kani::any(),
        memory.line(Some(0)),
    ));

    // Ceres's step: PC where the fetch read it, EI's toggle taken, IME set
    // before it; then the dispatch, whose first read was that fetch, and
    // the next step's fetch, at the vector.
    cpu.pc = cpu.pc.wrapping_sub(1);
    kani::assume(cpu.take_ime_toggle());
    cpu.dispatch_interrupt(&mut memory);
    let next = memory.read(cpu.pc);
    cpu.pc = cpu.pc.wrapping_add(1);
    assert!(memory.count == 6, "the dispatch takes five M-cycles");

    let mut acked = 0;
    for m in 1..6 {
        let (byte, ceres) = (memory.bytes[m], memory.trace[m]);
        kani::assume(address(ceres) != Some(IF_ADDR));
        net.int = memory.line(Some(m)) & !acked;
        // IE's register drives the bus at its address, not the memory.
        let access = net.mcycle(|addr| if addr == IE_ADDR { 0xFF } else { byte });
        assert!(access.is_some(), "the bus repeats where the clocks do");
        let access = access.unwrap_or(Access::Idle);
        assert!(address(access) == address(ceres), "the same address");
        assert!(access == ceres, "the same access");
        assert!(net.acks == memory.acks[m], "the same acknowledge");
        acked |= net.acks;
        net.int_before &= !net.acks;
    }
    assert!(
        boundary(&net.core, &cpu, next),
        "the vector's opcode begins"
    );
}

/// The 8-bit arithmetic and logic on A with a register: ADD, ADC, SUB,
/// SBC, AND, XOR, OR and CP with B, C, D, E, H, L or A.
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-halted
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_alu_a_r() {
    instruction_agrees(|op, _| (0x80..=0xBF).contains(&op) && op & 7 != 6, &[1], 1);
}

/// LD r,d8: B, C, D, E, H, L or A takes the byte after the opcode (two
/// M-cycles).
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-halted
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_ld_r_d8() {
    instruction_agrees(|op, _| op & 0xC7 == 0x06 && op != 0x36, &[2], 2);
}

/// LD r,r': between B, C, D, E, H, L and A.
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-halted
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_ld_r_r() {
    instruction_agrees(
        |op, _| (0x40..=0x7F).contains(&op) && op & 7 != 6 && op & 0x38 != 0x30,
        &[1],
        1,
    );
}

/// INC r and DEC r.
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-halted
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_inc_dec_r() {
    instruction_agrees(|op, _| op & 0xC6 == 0x04 && op & 0x38 != 0x30, &[1], 1);
}

/// The rotations of A (RLCA, RRCA, RLA, RRA), DAA, CPL, SCF and CCF.
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-halted
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_a_and_flags() {
    instruction_agrees(|op, _| op & 0xC7 == 0x07, &[1], 1);
}

/// NOP, JP HL, DI and EI.
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-halted
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_nop_jp_hl_di_ei() {
    instruction_agrees(|op, _| matches!(op, 0x00 | 0xE9 | 0xF3 | 0xFB), &[1], 1);
}

/// The 8-bit arithmetic and logic on A with the byte after the opcode.
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-halted
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_alu_a_d8() {
    instruction_agrees(|op, _| op & 0xC7 == 0xC6, &[2], 2);
}

/// LD r,(HL).
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-halted
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_ld_r_hl() {
    instruction_agrees(|op, _| op & 0xC7 == 0x46 && op != 0x76, &[2], 2);
}

/// LD (HL),r.
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-halted
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_ld_hl_r() {
    instruction_agrees(|op, _| op & 0xF8 == 0x70 && op != 0x76, &[2], 2);
}

/// The 8-bit arithmetic and logic on A with (HL).
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-halted
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_alu_a_hl() {
    instruction_agrees(|op, _| op & 0xC7 == 0x86, &[2], 2);
}

/// INC rr and DEC rr.
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-halted
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_inc_dec_rr() {
    instruction_agrees(|op, _| op & 0xC7 == 0x03, &[2], 2);
}

/// ADD HL,rr.
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-halted
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_add_hl_rr() {
    instruction_agrees(|op, _| op & 0xCF == 0x09, &[2], 2);
}

/// LD (BC),A, LD (DE),A, LD (HL+),A and LD (HL-),A, and their loads of A.
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-halted
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_ld_indirect_a() {
    instruction_agrees(|op, _| op & 0xC7 == 0x02, &[2], 2);
}

/// LDH (C),A and LDH A,(C).
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-halted
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_ldh_c() {
    instruction_agrees(|op, _| matches!(op, 0xE2 | 0xF2), &[2], 2);
}

/// LD SP,HL.
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-halted
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_ld_sp_hl() {
    instruction_agrees(|op, _| op == 0xF9, &[2], 2);
}

/// LD rr,d16.
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-halted
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_ld_rr_d16() {
    instruction_agrees(|op, _| op & 0xCF == 0x01, &[3], 3);
}

/// INC (HL), DEC (HL) and LD (HL),d8.
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-halted
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_hl_mem() {
    instruction_agrees(|op, _| matches!(op, 0x34..=0x36), &[3], 3);
}

/// POP rr.
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-halted
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_pop() {
    instruction_agrees(|op, _| op & 0xCF == 0xC1, &[3], 3);
}

/// LDH (a8),A and LDH A,(a8).
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-halted
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_ldh_a8() {
    instruction_agrees(|op, _| matches!(op, 0xE0 | 0xF0), &[3], 3);
}

/// LD HL,SP+e.
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-halted
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_ld_hl_sp_e() {
    instruction_agrees(|op, _| op == 0xF8, &[3], 3);
}

/// JR and JR cc, its condition false (two M-cycles).
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-halted
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_jr_2() {
    instruction_agrees(|op, _| op == 0x18 || op & 0xE7 == 0x20, &[2, 3], 2);
}

/// JR and JR cc, its condition true (three M-cycles).
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-halted
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_jr_3() {
    instruction_agrees(|op, _| op == 0x18 || op & 0xE7 == 0x20, &[2, 3], 3);
}

/// RET cc, its condition false (two M-cycles).
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-halted
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_ret_cc_2() {
    instruction_agrees(|op, _| op & 0xE7 == 0xC0, &[2, 5], 2);
}

/// JP and JP cc, its condition false (three M-cycles).
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-halted
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_jp_3() {
    instruction_agrees(|op, _| op == 0xC3 || op & 0xE7 == 0xC2, &[3, 4], 3);
}

/// CALL and CALL cc, its condition false (three M-cycles).
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-halted
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_call_3() {
    instruction_agrees(|op, _| op == 0xCD || op & 0xE7 == 0xC4, &[3, 6], 3);
}

/// After the CB prefix, the rotations, shifts and bit operations on a register (two M-cycles).
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-halted
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_cb_r() {
    instruction_agrees(|op, cb| op == 0xCB && cb & 7 != 6, &[2, 3, 4], 2);
}

/// After the CB prefix, BIT b,(HL) (three M-cycles).
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-halted
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_cb_bit_hl() {
    instruction_agrees(|op, cb| op == 0xCB && cb & 0xC7 == 0x46, &[2, 3, 4], 3);
}

/// JP and JP cc, its condition true (four M-cycles).
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-halted
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_jp_4() {
    instruction_agrees(|op, _| op == 0xC3 || op & 0xE7 == 0xC2, &[3, 4], 4);
}

/// PUSH rr.
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-halted
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_push() {
    instruction_agrees(|op, _| op & 0xCF == 0xC5, &[4], 4);
}

/// RST.
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-halted
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_rst() {
    instruction_agrees(|op, _| op & 0xC7 == 0xC7, &[4], 4);
}

/// RET and RETI.
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-halted
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_ret_reti() {
    instruction_agrees(|op, _| matches!(op, 0xC9 | 0xD9), &[4], 4);
}

/// LD (a16),A and LD A,(a16).
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-halted
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_ld_a16_a() {
    instruction_agrees(|op, _| matches!(op, 0xEA | 0xFA), &[4], 4);
}

/// ADD SP,e.
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-halted
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_add_sp_e() {
    instruction_agrees(|op, _| op == 0xE8, &[4], 4);
}

/// After the CB prefix, the rotations, shifts, RES and SET on (HL) (four M-cycles).
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-halted
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_cb_hl() {
    instruction_agrees(
        |op, cb| op == 0xCB && cb & 7 == 6 && cb & 0xC0 != 0x40,
        &[2, 3, 4],
        4,
    );
}

/// LD (a16),SP.
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-halted
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_ld_a16_sp() {
    instruction_agrees(|op, _| op == 0x08, &[5], 5);
}

/// RET cc, its condition true (five M-cycles).
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-halted
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_ret_cc_5() {
    instruction_agrees(|op, _| op & 0xE7 == 0xC0, &[2, 5], 5);
}

/// CALL and CALL cc, its condition true (six M-cycles).
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-halted
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_call_6() {
    instruction_agrees(|op, _| op == 0xCD || op & 0xE7 == 0xC4, &[3, 6], 6);
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
