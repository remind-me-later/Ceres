//! Ceres's CPU and the netlist's core side by side, an instruction at a
//! time, concretely (`cargo test -p ceres-core --features check sm83::cosim`).
//!
//! `check::boundary` on the instructions these run, in seconds, before Kani
//! takes every state: each opcode from random registers, at a random PC in
//! a memory of random bytes (the same on both sides, with what was
//! written). Each M-cycle's access (a read, a write or none, with its
//! address and byte) must be the same, and `boundary` must hold after the
//! fetch that ends the instruction.
//!
//! The interrupts' (`interrupts_agree_with_the_core`) runs ceres's steps on
//! a few instructions with IE and IF's requests, and holds each step's
//! first M-cycle to `boundary`, or to `entry_begins` where ceres
//! dispatches.

use {
    super::{
        Bus, Sm83,
        check::{Access, Net, boundary, entry_begins, entry_from, halt_begins, in_halt, waking},
    },
    ceres_netlist::sm83::{Flags, NetCore, NetRegisters, Register},
    std::collections::BTreeMap,
};

/// IE's address: the core's own register, which the memory here is not.
const IE_ADDR: u16 = 0xFFFF;
/// The prefix of the second table of opcodes.
const CB_PREFIX: u8 = 0xCB;
/// Random states per opcode.
const STATES: u32 = 400;

/// A xorshift64 generator: deterministic per seed.
struct Rng(u64);

impl Rng {
    const fn step(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    const fn flag(&mut self) -> bool {
        self.step() & 1 != 0
    }

    const fn word(&mut self) -> u16 {
        let bytes = self.step().to_le_bytes();
        u16::from_le_bytes([bytes[0], bytes[1]])
    }
}

/// A memory of any bytes: a byte per address from `seed`, and the writes.
#[derive(Clone)]
struct Memory {
    seed: u64,
    writes: Vec<(u16, u8)>,
}

impl Memory {
    fn read(&self, addr: u16) -> u8 {
        if let Some(write) = self.writes.iter().rev().find(|write| write.0 == addr) {
            return write.1;
        }
        let mut x = self.seed ^ (u64::from(addr) + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        x ^= x >> 29;
        x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
        x ^= x >> 32;
        x.to_le_bytes()[0]
    }

    fn write(&mut self, addr: u16, value: u8) {
        self.writes.push((addr, value));
    }
}

/// IF's lines: `bits` rise as phase E of M-cycle `rise` starts, and
/// `later` as that of `rise_later` (none if 0); they stay until
/// acknowledged.
#[derive(Clone, Copy, Default)]
struct Requests {
    rise: usize,
    bits: u8,
    rise_later: usize,
    later: u8,
}

impl Requests {
    /// The lines from phase E of M-cycle `m` on.
    const fn lines(self, m: usize) -> u8 {
        let first = if m >= self.rise { self.bits } else { 0 };
        let second = if m >= self.rise_later { self.later } else { 0 };
        first | second
    }
}

/// Ceres's bus on the memory, which remembers each M-cycle's access, with
/// IE and IF's requests. It keeps time as ceres's own bus does
/// (`cpu_bus`): an access flushes the time deferred, happens at its
/// M-cycle's start and defers the M-cycle's four dots; `tick` defers four
/// more, `defer` flushes all but some, `advance` moves time on and
/// `drop_deferred` forgets what is deferred; a step's end flushes it. An
/// M-cycle with no access is idle. Ceres's M-cycle starts at the netlist's
/// E (`mapping-m-cycle-at-e`): at an M-cycle's start ceres finds IF as the
/// lines took it at the E edge before, and once into it, as they took it at
/// its own.
struct TraceBus {
    memory: Memory,
    trace: Vec<Access>,
    /// IE answers at its address (the interrupts' co-simulation), rather
    /// than the memory.
    with_ie: bool,
    ie: u8,
    requests: Requests,
    acked: u8,
    /// The acknowledges, and the M-cycle each came in.
    acks: Vec<(usize, u8)>,
    /// The dispatches (each triggers the OAM bug at SP, and only they do).
    dispatches: u32,
    /// The time, in dots, and the dots deferred.
    time: usize,
    deferred: usize,
    /// The accesses that fell in one M-cycle were the same.
    overlaps_agree: bool,
}

impl TraceBus {
    fn new(memory: Memory) -> Self {
        Self {
            memory,
            trace: Vec::new(),
            with_ie: false,
            ie: 0,
            requests: Requests::default(),
            acked: 0,
            acks: Vec::new(),
            dispatches: 0,
            time: 0,
            deferred: 0,
            overlaps_agree: true,
        }
    }

    /// IF where the time is: at an M-cycle's start, as its E edge finds it;
    /// past that, with what the lines took there.
    fn flags(&self) -> u8 {
        let now = self.time / 4;
        let at = if self.time.is_multiple_of(4) {
            now.checked_sub(1)
        } else {
            Some(now)
        };
        at.map_or(0, |m| self.requests.lines(m)) & !self.acked
    }

    /// `access` in M-cycle `m`, the M-cycles before it idle. An access in
    /// an M-cycle that has one already (HALT's read, whose time ceres drops,
    /// and the fetch after it) must be the same.
    fn record(&mut self, m: usize, access: Access) {
        while self.trace.len() < m {
            self.trace.push(Access::Idle);
        }
        if let Some(had) = self.trace.get(m) {
            self.overlaps_agree &= *had == access;
        } else {
            self.trace.push(access);
        }
    }

    /// An access: the time deferred flushed, at its M-cycle's start, which
    /// four dots are deferred.
    fn access(&mut self, access: Access) {
        self.time += self.deferred;
        self.deferred = 4;
        self.record(self.time / 4, access);
    }

    /// A step's end: the time deferred flushed, and the M-cycles it took
    /// with no access idle.
    fn end_step(&mut self) {
        self.time += self.deferred;
        self.deferred = 0;
        while self.trace.len() < self.time.div_ceil(4) {
            self.trace.push(Access::Idle);
        }
    }
}

impl Bus for TraceBus {
    fn tick(&mut self) {
        let m = (self.time + self.deferred) / 4;
        self.deferred += 4;
        self.record(m, Access::Idle);
    }

    fn read(&mut self, addr: u16) -> u8 {
        let value = if addr == IE_ADDR && self.with_ie {
            self.ie
        } else {
            self.memory.read(addr)
        };
        self.access(Access::Read(addr, value));
        value
    }

    fn write(&mut self, addr: u16, val: u8) {
        if addr == IE_ADDR && self.with_ie {
            self.ie = val;
        }
        self.memory.write(addr, val);
        self.access(Access::Write(addr, val));
    }

    fn tick_oam_bug(&mut self, _: u16) {
        self.access(Access::Idle);
    }

    fn peek(&self, addr: u16) -> u8 {
        if addr == IE_ADDR && self.with_ie {
            self.ie
        } else {
            self.memory.read(addr)
        }
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
        self.acks.push((self.time / 4, bit));
    }
    fn clear_ie(&mut self) {}
    fn trigger_oam_bug(&mut self, _: u16) {
        self.dispatches += 1;
    }
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

/// The core between two instructions with ceres's registers and IME, a NOP
/// in its opcode register and the fetch of the opcode at `cpu.pc` next; W
/// and Z hold `wz`.
fn core_at(cpu: &Sm83, wz: u16) -> NetCore {
    let high = |pair: u16| Register::new(pair.to_le_bytes()[1]);
    let low = |pair: u16| Register::new(pair.to_le_bytes()[0]);
    let mut core = NetCore::POWER_ON;
    core.seq.not_ready_1 = false;
    core.seq.not_ready_2 = false;
    core.sampled.not_ready = false;
    core.regs = NetRegisters {
        a: high(cpu.af),
        b: high(cpu.bc),
        c: low(cpu.bc),
        d: high(cpu.de),
        e: low(cpu.de),
        h: high(cpu.hl),
        l: low(cpu.hl),
        w: high(wz),
        z: low(wz),
        sph: high(cpu.sp),
        spl: low(cpu.sp),
        pch: high(cpu.pc),
        pcl: low(cpu.pc),
    };
    core.alu.flags = Flags::from_bits(low(cpu.af).q);
    core.alu.masters = core.alu.flags;
    core.seq.ime_state = cpu.ime;
    core.seq.ime = cpu.ime;
    core
}

/// The same, with IE holding `ie`.
fn core_with_ie(cpu: &Sm83, wz: u16, ie: u8) -> NetCore {
    let mut core = core_at(cpu, wz);
    core.ints.ie = ie;
    core.ints.ie_next = ie;
    core
}

/// The registers where the core's masters differ from ceres's, by name,
/// and IME.
fn differing(core: &NetCore, cpu: &Sm83) -> String {
    let r = &core.regs;
    let pair = |high: Register, low: Register| u16::from_le_bytes([low.master, high.master]);
    let af = u16::from_le_bytes([core.alu.masters.bits(), r.a.master]);
    let mut out = Vec::new();
    for (name, net, cer) in [
        ("AF", af, cpu.af),
        ("BC", pair(r.b, r.c), cpu.bc),
        ("DE", pair(r.d, r.e), cpu.de),
        ("HL", pair(r.h, r.l), cpu.hl),
        ("SP", pair(r.sph, r.spl), cpu.sp),
        ("PC", pair(r.pch, r.pcl), cpu.pc),
    ] {
        if net != cer {
            out.push(format!("{name} {net:04X} != {cer:04X}"));
        }
    }
    // IME as its latch holds it: EI's is set already, though it counts an
    // instruction later (`ime_toggle`). What the core shows of it (`ime`)
    // is the latch's value as the M-cycle before found it, and the next
    // M-cycle's start shows the latch again.
    let ime_state = cpu.ime ^ cpu.ime_toggle;
    if core.seq.ime_state != ime_state {
        out.push(format!(
            "IME {} != {}",
            u8::from(core.seq.ime_state),
            u8::from(ime_state)
        ));
    }
    out.join(", ")
}

/// The core's control state between two instructions, for the survey of
/// what it can be there.
fn shape(core: &NetCore) -> String {
    let s = &core.seq;
    format!(
        "mcyc {} next {} w {} cb {}{} mread {} cond_fail {} fetch {} ime {}{} halt {} entering {}",
        s.mcyc,
        core.sampled.next_mcyc,
        u8::from(s.sel_reg_w),
        u8::from(s.cb_next),
        u8::from(s.cb_flag),
        u8::from(core.sampled.mread),
        u8::from(core.alu.cond_fail),
        u8::from(core.fetch),
        u8::from(s.ime_state),
        u8::from(s.ime),
        u8::from(s.halt),
        u8::from(s.entering),
    )
}

/// The opcodes compared: all but HALT, STOP and those that are not
/// instructions.
const fn compared(op: u8) -> bool {
    !matches!(
        op,
        0x10 | 0x76 | 0xD3 | 0xDB | 0xDD | 0xE3 | 0xE4 | 0xEB | 0xEC | 0xED | 0xF4 | 0xFC | 0xFD
    )
}

/// What the runs found.
#[derive(Default)]
struct Tally {
    runs: u64,
    /// Runs left out: an access at IE's address.
    at_ie: u64,
    /// Per opcode (and whether it is prefixed), the runs that differed, and
    /// the first of them.
    differing: BTreeMap<(bool, u8), (u32, String)>,
    /// The core's control states between instructions, and the opcodes
    /// after which each was seen first.
    shapes: BTreeMap<String, u8>,
    /// Registers whose slave differed from its master between instructions.
    stale_slaves: u64,
}

/// One instruction from one state: `op` at any PC (after the prefix CB if
/// `prefixed`), any registers and IME.
fn run(op: u8, prefixed: bool, rng: &mut Rng, tally: &mut Tally) {
    let mut cpu = Sm83 {
        af: rng.word() & 0xFFF0,
        bc: rng.word(),
        de: rng.word(),
        hl: rng.word(),
        sp: rng.word(),
        pc: rng.word(),
        ime: rng.flag(),
        ..Sm83::default()
    };
    let mut memory = Memory {
        seed: rng.step(),
        writes: Vec::new(),
    };
    if prefixed {
        memory.write(cpu.pc, CB_PREFIX);
        memory.write(cpu.pc.wrapping_add(1), op);
    } else {
        memory.write(cpu.pc, op);
    }
    let mut net = Net {
        core: core_at(&cpu, rng.word()),
        low_before: 0,
        confirm: true,
        int: 0,
        int_before: 0,
        acks: 0,
        halt_n: true,
        may_halt: false,
    };
    let mut net_memory = memory.clone();
    // The core settles in every phase, in the passes `check` gives it.
    let mut settles = true;
    let mut mcycle = |side: &mut Net| {
        let access = side.mcycle(|addr| net_memory.read(addr));
        settles &= access.is_some();
        let access = access.unwrap_or(Access::Idle);
        if let Access::Write(addr, value) = access {
            net_memory.write(addr, value);
        }
        access
    };
    let mut bus = TraceBus::new(memory);

    // The fetch of the opcode: the core's NOP ends in it.
    let fetched = bus.read(cpu.pc);
    cpu.pc = cpu.pc.wrapping_add(1);
    let first_access = mcycle(&mut net);
    let started = boundary(&net.core, &cpu, fetched);

    // The instruction, and the fetch of the next opcode.
    cpu.exec(&mut bus, fetched);
    let following = bus.read(cpu.pc);
    cpu.pc = cpu.pc.wrapping_add(1);

    let mut net_trace = vec![first_access];
    for _ in 1..bus.trace.len() {
        net_trace.push(mcycle(&mut net));
    }
    let touches_ie = |trace: &[Access]| {
        trace.iter().any(|access| {
            matches!(access, Access::Read(addr, _) | Access::Write(addr, _) if *addr == IE_ADDR)
        })
    };
    if touches_ie(&bus.trace) || touches_ie(&net_trace) {
        tally.at_ie += 1;
        return;
    }
    tally.runs += 1;
    let registers = differing(&net.core, &cpu);
    let r = &net.core.regs;
    tally.stale_slaves += u64::from(
        [
            r.a, r.b, r.c, r.d, r.e, r.h, r.l, r.sph, r.spl, r.pch, r.pcl,
        ]
        .iter()
        .any(|register| register.q != register.master)
            || net.core.alu.flags != net.core.alu.masters,
    );
    tally.shapes.entry(shape(&net.core)).or_insert(op);
    let related = settles && started && boundary(&net.core, &cpu, following);
    if net_trace != bus.trace || !related {
        let entry = tally.differing.entry((prefixed, op)).or_insert_with(|| {
            (
                0,
                format!(
                    "ceres {:?}\n      core  {:?}\n      settles {settles}, boundary before {started}, after: \
                     {registers}; {}",
                    bus.trace,
                    net_trace,
                    shape(&net.core)
                ),
            )
        });
        entry.0 += 1;
    }
}

#[test]
#[expect(
    clippy::print_stderr,
    reason = "a test, which reports what it compared"
)]
fn instructions_agree_with_the_core() {
    let mut tally = Tally::default();
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    for op in 0..=u8::MAX {
        for _ in 0..STATES {
            if compared(op) {
                run(op, false, &mut rng, &mut tally);
            }
            run(op, true, &mut rng, &mut tally);
        }
    }
    eprintln!(
        "{} runs compared ({} left out at IE's address); in {} of them a slave had not shown \
         its master yet",
        tally.runs, tally.at_ie, tally.stale_slaves
    );
    eprintln!("the core between instructions:");
    for (shape, op) in &tally.shapes {
        eprintln!("  {shape} (first after {op:02X})");
    }
    for (&(prefixed, op), &(count, ref first)) in &tally.differing {
        let prefix = if prefixed { "CB " } else { "" };
        eprintln!("  {prefix}{op:02X}: {count} of {STATES} differ; the first:\n      {first}");
    }
    assert!(
        tally.differing.is_empty(),
        "{} opcodes differ",
        tally.differing.len()
    );
}

/// IF's address.
const IF_ADDR: u16 = 0xFF0F;
/// HALT.
const HALT: u8 = 0x76;
/// Instructions a run of the interrupts' co-simulation goes on for, at
/// most.
const STEPS: usize = 16;
/// Runs of it.
const INTERRUPT_RUNS: u32 = 50_000;

/// What a run of instructions with requests found.
#[derive(Default)]
struct IntTally {
    runs: u64,
    /// Runs left out: an access at IF's address.
    at_if: u64,
    /// Runs with an access at IE's, where the core's own register answers.
    at_ie: u64,
    /// Runs in which an interrupt was dispatched, and those in which a
    /// dispatch's push took the request away (IE written): no acknowledge,
    /// and a jump to 0.
    dispatched: u64,
    cancelled: u64,
    /// Runs that went each of HALT's ways (`Steps::ways`).
    ways: [u64; 4],
    differing: u64,
    first: Option<String>,
}

/// The core beside ceres's steps: its memory, its accesses and its
/// acknowledges, M-cycle by M-cycle.
struct CoreSide {
    net: Net,
    memory: Memory,
    trace: Vec<Access>,
    acks: Vec<(usize, u8)>,
    acked: u8,
    settles: bool,
}

impl CoreSide {
    /// The next M-cycle, with IF's lines as `requests` has them, less
    /// what the core acknowledged.
    fn mcycle(&mut self, requests: Requests) -> Access {
        let m = self.trace.len();
        self.net.int = requests.lines(m) & !self.acked;
        // The memory leaves the bus alone at IE's address, where the core's
        // register drives it.
        let memory = &self.memory;
        let access = self.net.mcycle(|addr| {
            if addr == IE_ADDR {
                0xFF
            } else {
                memory.read(addr)
            }
        });
        self.settles &= access.is_some();
        let access = access.unwrap_or(Access::Idle);
        if let Access::Write(addr, value) = access {
            self.memory.write(addr, value);
        }
        let acks = self.net.acks;
        if acks != 0 {
            self.acks.push((m, acks));
            self.acked |= acks;
            self.net.int &= !acks;
            self.net.int_before &= !acks;
        }
        self.trace.push(access);
        access
    }
}

/// An access at `at` in `trace`.
fn touches(trace: &[Access], at: u16) -> bool {
    trace.iter().any(
        |access| matches!(access, Access::Read(addr, _) | Access::Write(addr, _) if *addr == at),
    )
}

/// A step of ceres's, for the relations after its M-cycles: where it
/// starts, ceres as its first M-cycle and as the step leaves it, and what
/// it did.
#[expect(clippy::struct_excessive_bools, reason = "what a step did")]
struct Step {
    start: usize,
    first: Sm83,
    after: Sm83,
    /// It executed HALT; it left ceres halted.
    halt: bool,
    halted: bool,
    /// It started halted, or from a fetch (neither halted nor with the
    /// HALT bug's fetch to come).
    halted_before: bool,
    from_fetch: bool,
    dispatched: bool,
}

/// What a run of steps found, step by step: whether each step's first
/// M-cycle left the two related (with the first failure), and which of
/// HALT's ways it went.
#[derive(Default)]
struct Steps {
    unrelated: Option<String>,
    /// HALT's ways: halted, the HALT bug, woke to the next instruction,
    /// woke to a dispatch.
    ways: [bool; 4],
}

impl Steps {
    /// After a step's first M-cycle `m`: the core `c` (whose access was
    /// `access`) and ceres as that M-cycle left it (`first`) are related
    /// as a fetch's end or an entry's beginning.
    fn first_mcycle(
        &mut self,
        m: usize,
        c: &NetCore,
        first: &Sm83,
        access: Access,
        dispatched: bool,
    ) {
        let holds = if dispatched {
            entry_begins(c, first)
        } else {
            matches!(access, Access::Read(_, op) if boundary(c, first, op))
        };
        let what = if dispatched { "dispatch" } else { "fetch" };
        self.holds(holds, m, what, c, first);
    }

    /// After the step's M-cycle `m` (whose access was `access`), the core
    /// `c` and ceres are related: after a fetch or a dispatch's first read;
    /// after HALT's M-cycle (halting, or the HALT bug's fetch or dispatch
    /// sharing it); halted, and then after the wake-up's fetch or
    /// dispatch's first read.
    fn after_mcycle(&mut self, step: &Step, m: usize, c: &NetCore, access: Access) {
        let first = &step.first;
        if m == step.start && step.from_fetch {
            self.first_mcycle(m, c, first, access, step.dispatched);
        }
        if step.halt && m == step.start + 1 {
            let after = &step.after;
            if step.halted {
                self.holds(halt_begins(c, after), m, "HALT", c, after);
            } else {
                let holds = if c.seq.entering {
                    entry_from(c, after)
                } else {
                    matches!(access, Access::Read(_, op) if boundary(c, after, op))
                };
                self.holds(holds, m, "HALT bug", c, after);
            }
        }
        if step.halted_before {
            if m == step.start {
                // The halted M-cycle: still halted, or woken by a request
                // both sampled.
                let mut before = registers_of(first);
                before.pc = before.pc.wrapping_sub(1);
                if step.halted {
                    self.holds(in_halt(c, &before), m, "halted", c, &before);
                } else {
                    self.holds(waking(c, &before), m, "waking", c, &before);
                }
            } else if m == step.start + 1 {
                self.first_mcycle(m, c, first, access, step.dispatched);
            } else {
                // The instruction or the dispatch goes on.
            }
        }
    }

    /// Which of HALT's ways a step went: from HALT (`at_halt`, the opcode
    /// at PC) or halted, to `cpu`.
    fn note_ways(&mut self, at_halt: bool, halted_before: bool, cpu: &Sm83, dispatched: bool) {
        let ways = [
            cpu.is_halted(),
            at_halt && !halted_before && cpu.halt_bug,
            halted_before && !cpu.is_halted() && !dispatched,
            halted_before && dispatched,
        ];
        for (seen, now) in self.ways.iter_mut().zip(ways) {
            *seen |= now;
        }
    }

    /// A relation that must hold after M-cycle `m`, between the core `c`
    /// and ceres `cpu`.
    fn holds(&mut self, holds: bool, m: usize, what: &str, c: &NetCore, cpu: &Sm83) {
        if !holds && self.unrelated.is_none() {
            self.unrelated = Some(format!(
                "M-cycle {m}, {what}: {}; {}",
                differing(c, cpu),
                shape(c)
            ));
        }
    }
}

/// Ceres's registers, IME and the HALT bug, as `cpu` has them.
fn registers_of(cpu: &Sm83) -> Sm83 {
    Sm83 {
        af: cpu.af,
        bc: cpu.bc,
        de: cpu.de,
        hl: cpu.hl,
        sp: cpu.sp,
        pc: cpu.pc,
        ime: cpu.ime,
        ime_toggle: cpu.ime_toggle,
        halt_bug: cpu.halt_bug,
        ..Sm83::default()
    }
}

/// Instructions from `cpu.pc` on, ceres a step at a time as it runs them
/// (EI's delay, the check for an interrupt before the fetch, the
/// dispatch), the core as many M-cycles, with IE and IF's requests on both:
/// the accesses and the acknowledges must be the same, M-cycle by M-cycle.
/// A run stops before HALT, STOP or an opcode that is not an instruction.
/// Any registers and IME, one state in four with SP just above 0000,
/// where a push writes IE.
fn random_cpu(rng: &mut Rng) -> Sm83 {
    let sp = if rng.word().is_multiple_of(4) {
        rng.word() % 4
    } else {
        rng.word()
    };
    Sm83 {
        af: rng.word() & 0xFFF0,
        bc: rng.word(),
        de: rng.word(),
        hl: rng.word(),
        sp,
        pc: rng.word(),
        ime: rng.flag(),
        ..Sm83::default()
    }
}

fn run_with_requests(rng: &mut Rng, tally: &mut IntTally) {
    let mut cpu = random_cpu(rng);
    let mut memory = Memory {
        seed: rng.step(),
        writes: Vec::new(),
    };
    // One run in four starts on a HALT.
    if rng.word().is_multiple_of(4) {
        memory.write(cpu.pc, HALT);
    }
    let ie = rng.word().to_le_bytes()[0];
    let rise = usize::from(rng.word() % 24);
    let requests = Requests {
        rise,
        bits: rng.word().to_le_bytes()[0] & 0x1F,
        rise_later: rise + usize::from(rng.word() % 8),
        later: rng.word().to_le_bytes()[0] & 0x1F,
    };
    let mut core = CoreSide {
        net: Net {
            core: core_with_ie(&cpu, rng.word(), ie),
            low_before: 0,
            confirm: true,
            int: 0,
            int_before: 0,
            acks: 0,
            halt_n: true,
            may_halt: true,
        },
        memory: memory.clone(),
        trace: Vec::new(),
        acks: Vec::new(),
        acked: 0,
        settles: true,
    };
    let mut bus = TraceBus::new(memory);
    bus.with_ie = true;
    bus.ie = ie;
    bus.requests = requests;

    let mut steps = Steps::default();
    for _ in 0..STEPS {
        let opcode = bus.peek(cpu.pc);
        if opcode != CB_PREFIX && opcode != HALT && !compared(opcode) {
            break;
        }
        // A step from HALT, or with the HALT bug's fetch still to come, does
        // not start where `boundary` or `entry_begins` speaks.
        let halted_before = cpu.is_halted();
        let was_halted = halted_before || cpu.halt_bug;
        let start = bus.trace.len();
        let dispatches_before = bus.dispatches;
        // Ceres as the step's first M-cycle leaves it, PC one on (as the
        // core's fetch has it, whether ceres fetched or began a dispatch).
        let mut first = registers_of(&cpu);
        first.pc = first.pc.wrapping_add(1);
        cpu.step(&mut bus);
        bus.end_step();
        let dispatched = bus.dispatches != dispatches_before;
        steps.note_ways(opcode == HALT, halted_before, &cpu, dispatched);
        let step = Step {
            start,
            first,
            after: registers_of(&cpu),
            halt: opcode == HALT && !halted_before && !dispatched,
            halted: cpu.is_halted(),
            halted_before,
            from_fetch: !was_halted,
            dispatched,
        };
        while core.trace.len() < bus.trace.len() {
            let m = core.trace.len();
            let access = core.mcycle(requests);
            steps.after_mcycle(&step, m, &core.net.core, access);
        }
    }
    if touches(&bus.trace, IF_ADDR) || touches(&core.trace, IF_ADDR) {
        tally.at_if += 1;
        return;
    }
    tally.runs += 1;
    for (count, seen) in tally.ways.iter_mut().zip(steps.ways) {
        *count += u64::from(seen);
    }
    tally.at_ie += u64::from(touches(&bus.trace, IE_ADDR));
    tally.dispatched += u64::from(bus.dispatches != 0);
    tally.cancelled += u64::from(bus.dispatches as usize > bus.acks.len());
    let settles = core.settles;
    let related = steps.unrelated.is_none();
    let why = steps.unrelated.unwrap_or_default();
    if !settles
        || !related
        || !bus.overlaps_agree
        || core.trace != bus.trace
        || core.acks != bus.acks
    {
        tally.differing += 1;
        tally.first.get_or_insert_with(|| {
            format!(
                "IE {ie:02X}, IF {:02X} from M-cycle {}, {:02X} from {}, IME {}\n  ceres {:?}\n        acks {:?}\n                   core  {:?}\n        acks {:?}\n  settles {settles}, related {related} {why}",
                requests.bits, requests.rise, requests.later, requests.rise_later, u8::from(cpu.ime), bus.trace, bus.acks, core.trace, core.acks
            )
        });
    }
}

#[test]
#[expect(
    clippy::print_stderr,
    reason = "a test, which reports what it compared"
)]
fn interrupts_agree_with_the_core() {
    let mut tally = IntTally::default();
    let mut rng = Rng(0x2545_F491_4F6C_DD1D);
    for _ in 0..INTERRUPT_RUNS {
        run_with_requests(&mut rng, &mut tally);
    }
    eprintln!(
        "{} runs compared ({} left out at IF's address; {} with an access at IE's), {} with a \
         dispatch ({} cancelled by a push to IE), {} differ",
        tally.runs, tally.at_if, tally.at_ie, tally.dispatched, tally.cancelled, tally.differing
    );
    eprintln!(
        "  HALT: halted in {}, the HALT bug in {}; woke to the next instruction in {}, to a \
         dispatch in {}",
        tally.ways[0], tally.ways[1], tally.ways[2], tally.ways[3]
    );
    if let Some(first) = tally.first.as_ref() {
        eprintln!("the first:\n{first}");
    }
    assert_eq!(tally.differing, 0);
}
