//! Kani proofs that ceres's instructions are the netlist's core's
//! (`cargo kani -p ceres-core`), an instruction class at a time.
//!
//! From every pair of states `check::boundary` relates, with the core
//! having just fetched an opcode of the class: ceres executes it and reads
//! the next opcode, the core runs as many M-cycles, each M-cycle's access
//! is the same (the memory gives any bytes, the same to both), and
//! `boundary` holds again with the opcode read.
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
//! with it, one M-cycle took 6.7 GB, which left no room for two.)

use {
    super::{
        Bus, Sm83,
        check::{Access, Net, PHASES, boundary, inputs},
    },
    ceres_netlist::sm83::{
        CoreInputs, Flags, NetAlu, NetCore, NetCoreInterrupts, NetRegisters, NetSequencer,
        Register, Sampled, decode,
    },
};

/// The most M-cycles an instruction takes with the fetch that ends it.
const MCYCLES: usize = 7;
/// IE's address.
const IE_ADDR: u16 = 0xFFFF;

/// The address of an access, if it has one.
const fn address(access: Access) -> Option<u16> {
    match access {
        Access::Idle => None,
        Access::Read(addr, _) | Access::Write(addr, _) => Some(addr),
    }
}

/// A memory whose reads give any bytes, in order, and which remembers each
/// M-cycle's access.
struct AnyMemory {
    bytes: [u8; MCYCLES],
    trace: [Access; MCYCLES],
    count: usize,
}

impl AnyMemory {
    fn new() -> Self {
        Self {
            bytes: kani::any(),
            trace: [Access::Idle; MCYCLES],
            count: 0,
        }
    }

    fn record(&mut self, access: Access) {
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
        self.record(Access::Write(addr, val));
    }

    fn tick_oam_bug(&mut self, _: u16) {
        self.record(Access::Idle);
    }

    fn peek(&self, _: u16) -> u8 {
        kani::any()
    }

    fn flush(&mut self) {}
    fn defer(&mut self, _: i32) {}
    fn drop_deferred(&mut self) {}
    fn advance(&mut self, _: i32) {}
    fn is_cgb_hardware(&self) -> bool {
        false
    }
    fn interrupts_pending(&self) -> bool {
        false
    }
    fn read_if(&self) -> u8 {
        0xE0
    }
    fn read_ie(&self) -> u8 {
        0
    }
    fn ack_interrupt(&mut self, _: u8) {}
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

/// Any core between instructions, ceres's `cpu` about to execute `op`:
/// `any_core` with what `check::boundary` pins set rather than assumed
/// (Kani folds what is set, not what is assumed), the rest anything.
fn core_between_any(cpu: &Sm83, op: u8) -> NetCore {
    let mut core = any_core();
    let high = |pair: u16| pair.to_le_bytes()[1];
    let low = |pair: u16| pair.to_le_bytes()[0];
    let s = &mut core.seq;
    s.ir_next = op;
    s.cb_next = false;
    (s.halt, s.stop, s.halt_delayed, s.stop_delayed) = (false, false, false, false);
    (s.pending_delayed, s.not_ready_1, s.not_ready_2) = (false, false, false);
    s.ime_state = cpu.ime ^ cpu.ime_toggle;
    (s.no_nmi, s.no_nmi_1, s.nmi_low_1, s.nmi_low_2) = (true, true, true, true);
    (s.nmi_req, s.nmi_entering, s.nmi_entry) = (false, false, false);
    (s.entering, s.int_entry) = (false, false);
    let r = &mut core.regs;
    (r.a.master, r.b.master, r.c.master) = (high(cpu.af), high(cpu.bc), low(cpu.bc));
    (r.d.master, r.e.master) = (high(cpu.de), low(cpu.de));
    (r.h.master, r.l.master) = (high(cpu.hl), low(cpu.hl));
    (r.sph.master, r.spl.master) = (high(cpu.sp), low(cpu.sp));
    (r.pch.master, r.pcl.master) = (high(cpu.pc), low(cpu.pc));
    core.alu.masters = Flags::from_bits(low(cpu.af));
    core.alu.cond_fail = false;
    core.ints.pending = 0;
    (core.clk, core.exec_phase, core.fetch) = (false, true, true);
    core.sampled = Sampled {
        next_mcyc: 0,
        mread: false,
        halt_step: false,
        stop: false,
        int_pending: false,
        not_ready: false,
        nmi: false,
    };
    core
}

/// The NMI's latches at rest: the DMG's core has no NMI.
const fn nmi_at_rest(core: &NetCore) -> bool {
    let s = &core.seq;
    s.no_nmi & s.no_nmi_1 & s.nmi_low_1 & s.nmi_low_2 & !s.nmi_req & !s.nmi_entering & !s.nmi_entry
}

/// The core is settled in `phase` with `d` on the chip's data bus and its
/// keeper holding `kept`: a pass changes nothing.
fn settled_in(core: &NetCore, phase: u8, d: u8, kept: u8) -> bool {
    let i = inputs(phase, d);
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

/// From any core settled in the phase before `phase` (any bus, any keeper),
/// the NMI's latches at rest: `phase`'s edges, then one pass with any bus.
fn settles_after_edges(phase: u8) {
    let core = any_core();
    let before = if phase == 0 { PHASES - 1 } else { phase - 1 };
    kani::assume(settled_in(&core, before, kani::any(), kani::any()));
    kani::assume(nmi_at_rest(&core));
    let i = inputs(phase, kani::any());
    let kept = core.db;
    let mut core = core;
    core.edges(&i);
    one_pass_settles(core, &i, kept);
}

/// From any core settled in `phase` with a bus and a keeper, the NMI's
/// latches at rest: one pass with another bus (the memory's), the keeper
/// the same.
fn settles_driven(phase: u8) {
    let core = any_core();
    let kept: u8 = kani::any();
    kani::assume(settled_in(&core, phase, kani::any(), kept));
    kani::assume(nmi_at_rest(&core));
    one_pass_settles(core, &inputs(phase, kani::any()), kept);
}

/// An instruction of the class `in_class`, which takes `mcycles` M-cycles
/// with the fetch that ends it, from every related pair: the accesses
/// agree, and the pair is related again. (The count is given, not taken
/// from ceres's run: the solver would run the core for every count it
/// cannot rule out.)
fn instruction_agrees(in_class: impl Fn(u8) -> bool, mcycles: usize) {
    let op: u8 = kani::any();
    kani::assume(in_class(op));
    let mut cpu = any_cpu();
    let mut net = Net {
        core: core_between_any(&cpu, op),
        low_before: kani::any(),
        confirm: false,
    };
    kani::assume(boundary(&net.core, &cpu, op));
    // Settled in the instruction before's last phase.
    kani::assume(settled_in(&net.core, PHASES - 1, kani::any(), kani::any()));

    let mut memory = AnyMemory::new();
    cpu.exec(&mut memory, op);
    let next = memory.read(cpu.pc);
    cpu.pc = cpu.pc.wrapping_add(1);

    assert!(memory.count == mcycles);
    for m in 0..mcycles {
        let (byte, ceres) = (memory.bytes[m], memory.trace[m]);
        // IE's address is the core's own register's, not the memory's.
        kani::assume(address(ceres) != Some(IE_ADDR));
        // Settled in one pass a phase, unconfirmed: always an access.
        let access = net.mcycle(|_| byte).unwrap_or(Access::Idle);
        assert!(address(access) == address(ceres), "the same address");
        assert!(access == ceres, "the same access");
    }
    assert!(boundary(&net.core, &cpu, next));
}

/// The 8-bit arithmetic and logic on A with a register: ADD, ADC, SUB,
/// SBC, AND, XOR, OR and CP with B, C, D, E, H, L or A.
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-no-interrupt
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_alu_a_r() {
    instruction_agrees(|op| (0x80..=0xBF).contains(&op) && op & 7 != 6, 1);
}

/// LD r,d8: B, C, D, E, H, L or A takes the byte after the opcode (two
/// M-cycles).
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-no-interrupt
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_ld_r_d8() {
    instruction_agrees(|op| op & 0xC7 == 0x06 && op != 0x36, 2);
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
