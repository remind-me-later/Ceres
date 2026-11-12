//! Ceres's CPU and the netlist's core side by side, an instruction at a
//! time, concretely (`cargo test -p ceres-core --features check sm83::cosim`).
//!
//! `check::boundary` on the instructions these run, in seconds, before Kani
//! takes every state: each opcode from random registers, at a random PC in
//! a memory of random bytes (the same on both sides, with what was
//! written). Each M-cycle's access (a read, a write or none, with its
//! address and byte) must be the same, and `boundary` must hold after the
//! fetch that ends the instruction.

use {
    super::{
        Bus, Sm83,
        check::{Access, Net, boundary},
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

/// Ceres's bus on the memory, which remembers each M-cycle's access.
struct TraceBus {
    memory: Memory,
    trace: Vec<Access>,
}

impl Bus for TraceBus {
    fn tick(&mut self) {
        self.trace.push(Access::Idle);
    }

    fn read(&mut self, addr: u16) -> u8 {
        let value = self.memory.read(addr);
        self.trace.push(Access::Read(addr, value));
        value
    }

    fn write(&mut self, addr: u16, val: u8) {
        self.memory.write(addr, val);
        self.trace.push(Access::Write(addr, val));
    }

    fn tick_oam_bug(&mut self, _: u16) {
        self.trace.push(Access::Idle);
    }

    fn peek(&self, addr: u16) -> u8 {
        self.memory.read(addr)
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
    let mut bus = TraceBus {
        memory,
        trace: Vec::new(),
    };

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
