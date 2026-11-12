//! Kani proofs that ceres's instructions are the netlist's core's
//! (`cargo kani -p ceres-core`), an instruction class at a time.
//!
//! From every pair of states `check::boundary` relates, with the core
//! having just fetched an opcode of the class: ceres executes it and reads
//! the next opcode, the core runs as many M-cycles, each M-cycle's access
//! is the same (the memory gives any bytes, the same to both), and
//! `boundary` holds again with the opcode read.

use {
    super::{
        Bus, Sm83,
        check::{Access, Net, boundary},
    },
    ceres_netlist::sm83::{
        Flags, NetAlu, NetCore, NetCoreInterrupts, NetRegisters, NetSequencer, Register, Sampled,
        decode,
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
        core: any_core(),
        low_before: kani::any(),
    };
    kani::assume(boundary(&net.core, &cpu, op));

    let mut memory = AnyMemory::new();
    cpu.exec(&mut memory, op);
    let next = memory.read(cpu.pc);
    cpu.pc = cpu.pc.wrapping_add(1);

    assert!(memory.count == mcycles);
    for m in 0..mcycles {
        let (byte, ceres) = (memory.bytes[m], memory.trace[m]);
        // IE's address is the core's own register's, not the memory's.
        kani::assume(address(ceres) != Some(IE_ADDR));
        let access = net.mcycle(|_| byte);
        assert!(access.is_some(), "the core settles in each phase");
        let access = access.unwrap_or(Access::Idle);
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
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_alu_a_r() {
    instruction_agrees(|op| (0x80..=0xBF).contains(&op) && op & 7 != 6, 1);
}
