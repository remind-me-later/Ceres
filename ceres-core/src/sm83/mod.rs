//! The SM83, the Game Boy's CPU: registers, the step loop with interrupt
//! dispatch and HALT, and operand decoding. The instructions are in
//! `instructions`, the arithmetic in `alu`, and the machine side of the bus
//! in `crate::cpu_bus`.

mod alu;
mod bus;
pub(crate) mod conflict;
mod instructions;
#[cfg(kani)]
mod proofs;

use crate::{
    interrupts::{INT_MASK, INT_VECTOR_BASE},
    memory::{IE, IF, P1, io_addr},
};

pub(crate) use bus::Bus;
#[cfg(feature = "debug")]
use core::mem;

const ZF: u16 = 0x80;
const NF: u16 = 0x40;
const HF: u16 = 0x20;
const CF: u16 = 0x10;

/// A register pair, as an opcode names it.
#[derive(Clone, Copy)]
enum Pair {
    Af,
    Bc,
    De,
    Hl,
    Sp,
}

impl Pair {
    /// The pair in bits 4-5 of `op`: BC, DE, HL or SP.
    const fn from_op(op: u8) -> Self {
        match (op >> 4) & 3 {
            0 => Self::Bc,
            1 => Self::De,
            2 => Self::Hl,
            _ => Self::Sp,
        }
    }

    /// The pair in bits 4-5 of `op` with AF in place of SP (PUSH and POP).
    const fn from_push_pop(op: u8) -> Self {
        match Self::from_op(op) {
            Self::Sp => Self::Af,
            pair => pair,
        }
    }
}

/// An 8-bit operand, as bits 0-2 of an opcode name it (bits 3-5 for the
/// destination of LD r,r'): a register, or the byte at HL.
#[derive(Clone, Copy)]
enum Reg8 {
    B,
    C,
    D,
    E,
    H,
    L,
    HlIndirect,
    A,
}

impl Reg8 {
    const fn from_op(op: u8) -> Self {
        match op & 7 {
            0 => Self::B,
            1 => Self::C,
            2 => Self::D,
            3 => Self::E,
            4 => Self::H,
            5 => Self::L,
            6 => Self::HlIndirect,
            _ => Self::A,
        }
    }
}

const fn high(pair: u16) -> u8 {
    pair.to_le_bytes()[1]
}

const fn low(pair: u16) -> u8 {
    pair.to_le_bytes()[0]
}

const fn with_high(pair: u16, val: u8) -> u16 {
    u16::from_le_bytes([low(pair), val])
}

const fn with_low(pair: u16, val: u8) -> u16 {
    u16::from_le_bytes([val, high(pair)])
}

#[expect(clippy::struct_excessive_bools, reason = "Independent CPU state flags")]
#[derive(Default)]
pub(crate) struct Sm83 {
    af: u16,
    bc: u16,
    de: u16,
    ime_toggle: bool,
    just_halted: bool,
    hl: u16,
    ime: bool,
    halt_bug: bool,
    /// An opcode HALT or STOP read ahead for a pending transfer: it runs
    /// as read then, whatever the memory holds by the time it runs.
    prefetched: Option<u8>,
    is_halted: bool,
    #[cfg(feature = "debug")]
    has_executed_illegal_opcode: bool,
    #[cfg(feature = "debug")]
    ld_b_b_breakpoint: bool,
    pc: u16,
    sp: u16,
}

impl Sm83 {
    #[cfg(feature = "debug")]
    pub(crate) const fn take_illegal_opcode(&mut self) -> bool {
        mem::replace(&mut self.has_executed_illegal_opcode, false)
    }

    pub(crate) const fn a(&self) -> u8 {
        high(self.af)
    }

    pub(crate) const fn af(&self) -> u16 {
        self.af
    }

    pub(crate) const fn bc(&self) -> u16 {
        self.bc
    }

    pub(crate) const fn de(&self) -> u16 {
        self.de
    }

    pub(crate) const fn hl(&self) -> u16 {
        self.hl
    }

    pub(crate) const fn is_halted(&self) -> bool {
        self.is_halted
    }

    pub(crate) const fn ime(&self) -> bool {
        self.ime
    }

    #[cfg(feature = "debug")]
    pub(crate) fn take_ld_b_b_breakpoint(&mut self) -> bool {
        mem::take(&mut self.ld_b_b_breakpoint)
    }

    pub(crate) const fn pc(&self) -> u16 {
        self.pc
    }

    pub(crate) const fn sp(&self) -> u16 {
        self.sp
    }
}

impl Sm83 {
    /// Sets IME (a state load).
    pub(crate) const fn set_ime(&mut self, ime: bool) {
        self.ime = ime;
        self.ime_toggle = false;
    }

    /// Halts or wakes the CPU (a state load).
    pub(crate) const fn set_halted(&mut self, halted: bool) {
        self.is_halted = halted;
    }

    /// Set the program counter.
    pub(crate) const fn set_pc(&mut self, pc: u16) {
        self.pc = pc;
    }

    pub(crate) const fn set_af(&mut self, af: u16) {
        // The low nibble of F is always zero.
        self.af = af & 0xFFF0;
    }

    pub(crate) const fn set_bc(&mut self, bc: u16) {
        self.bc = bc;
    }

    pub(crate) const fn set_de(&mut self, de: u16) {
        self.de = de;
    }

    pub(crate) const fn set_hl(&mut self, hl: u16) {
        self.hl = hl;
    }

    pub(crate) const fn set_sp(&mut self, sp: u16) {
        self.sp = sp;
    }
}

impl Sm83 {
    /// Executes exactly one instruction (or one interrupt dispatch / HALT
    /// M-cycle). All system interaction goes through `bus`; nothing else is
    /// observable from the outside.
    ///
    /// EI semantics: hardware EI schedules IME to flip after the next
    /// instruction completes. SameBoy (sm83_cpu.c:1636-1640) toggles IME
    /// at the start of each instruction fetch using `ime_toggle`, and the
    /// pre-toggle IME is used for interrupt dispatch decisions. We mirror
    /// that: capture the IME value seen by this instruction (`effective_ime`),
    /// then if EI was pending, flip IME in place. If the instruction is DI
    /// it will clear IME again; if it's anything else, IME will stay true
    /// through subsequent step calls.
    pub(crate) fn step<B: Bus>(&mut self, bus: &mut B) {
        // Port of SameBoy's `GB_cpu_run` control flow. All time of the
        // previous instruction has been flushed, so interrupt lines are
        // sampled with the hardware at instruction end.
        let cgb = bus.is_cgb_hardware();

        if bus.is_stopped() {
            bus.advance(4);
            if bus.peek(io_addr(P1)) & 0xF != 0xF {
                bus.leave_stop();
                bus.dma_run(true);
                bus.advance(8);
            }
            return;
        }

        // While halted, DMG samples the interrupt lines two dots into each
        // 4-dot step; CGB (and the first step after HALT) samples at the start.
        if self.is_halted && !cgb && !self.just_halted {
            bus.advance(2);
        }

        let interrupt_pending = bus.interrupts_pending();

        if self.is_halted {
            bus.advance(if cgb || self.just_halted { 4 } else { 2 });
            if bus.take_unhalt() {
                self.is_halted = false;
                bus.set_halted(false);
            }
        }
        self.just_halted = false;

        let effective_ime = self.ime;
        if self.ime_toggle {
            self.ime = !self.ime;
            self.ime_toggle = false;
        }

        if self.is_halted && interrupt_pending && (!effective_ime || self.halt_bug) {
            // Wake up from HALT without calling the interrupt code. With the
            // IME set, the opcode HALT prefetched for a transfer runs before
            // the interrupt is dispatched (gambatte's `setMinIntTime`).
            self.is_halted = false;
            bus.wake_from_stop();
            bus.dma_run(true);
        } else if effective_ime && interrupt_pending {
            // Only a wake-up from HALT gives the OAM DMA its extra step.
            let woke = self.is_halted;
            self.is_halted = false;
            bus.wake_from_stop();
            bus.dma_run(woke);
            self.dispatch_interrupt(bus);
            return;
        } else {
            // Nothing to wake up for or to dispatch.
        }

        if !self.is_halted {
            let fetched = bus.read(self.pc);
            let op = self.prefetched.take().unwrap_or(fetched);
            self.pc = self.pc.wrapping_add(1);

            // A pending HDMA burst steals the bus right after the opcode
            // fetch, while the fetch's M-cycle is still pending.
            bus.tick_hdma();

            if self.halt_bug {
                self.pc = self.pc.wrapping_sub(1);
                self.halt_bug = false;
            }

            self.exec(bus, op);
        }
    }

    /// The five-M-cycle interrupt dispatch (SameBoy: fetch, OAM-bug cycle,
    /// internal cycle, push high, push low).
    fn dispatch_interrupt<B: Bus>(&mut self, bus: &mut B) {
        // M1: dummy fetch. M2: PC (and SP) on the address bus.
        bus.read(self.pc);
        // A pending HDMA burst goes first, like after any opcode fetch.
        bus.tick_hdma();
        bus.tick_oam_bug(self.pc.wrapping_add(1));
        bus.trigger_oam_bug(self.sp);
        bus.tick();

        let [lo, hi] = self.pc.to_le_bytes();

        // Push Hi. The write lands after all preceding M-cycles flush.
        self.sp = self.sp.wrapping_sub(1);
        bus.write(self.sp, hi);

        // Push Lo. IF/IE are re-evaluated after the Lo push finishes, using
        // the value from BEFORE the write if it targets IF or IE.
        self.sp = self.sp.wrapping_sub(1);

        let old_flags = (self.sp == io_addr(IF)).then(|| bus.read_if() & INT_MASK);
        let old_enable = (self.sp == io_addr(IE)).then(|| bus.read_ie() & INT_MASK);

        bus.write(self.sp, lo);

        let flags = old_flags.unwrap_or_else(|| bus.read_if() & INT_MASK);
        let enable = old_enable.unwrap_or_else(|| bus.read_ie() & INT_MASK);

        let queue = enable & flags;

        // Two of the last M-cycle's four dots elapse before the interrupt is
        // acknowledged and the vector is chosen.
        bus.defer(2);

        if queue != 0 {
            let bit = (queue.trailing_zeros() & 7) as u8;
            bus.ack_interrupt(1 << bit);
            self.pc = INT_VECTOR_BASE | (u16::from(bit) << 3);
        } else {
            // The request went away during the dispatch (IE overwritten by
            // the push): the CPU jumps to 0.
            self.pc = 0x0000;
        }

        self.ime = false;
    }
}

// Internal
impl Sm83 {
    fn do_call(&mut self, bus: &mut impl Bus) {
        let addr = self.imm16(bus);
        self.push(bus, self.pc);
        self.pc = addr;
    }

    fn do_jump_relative(&mut self, bus: &mut impl Bus) {
        let offset = self.imm8_offset(bus);
        bus.tick_oam_bug(self.pc);
        self.pc = self.pc.wrapping_add(offset);
    }

    fn do_jump_to_immediate(&mut self, bus: &mut impl Bus) {
        let addr = self.imm16(bus);
        self.pc = addr;
        bus.tick();
    }

    #[must_use]
    #[expect(
        clippy::inline_always,
        reason = "about 2% of a frame's time: the `Reg8` match stops it being inlined"
    )]
    #[inline(always)]
    fn get_r(&self, bus: &mut impl Bus, op: u8) -> u8 {
        match Reg8::from_op(op) {
            Reg8::B => high(self.bc),
            Reg8::C => low(self.bc),
            Reg8::D => high(self.de),
            Reg8::E => low(self.de),
            Reg8::H => high(self.hl),
            Reg8::L => low(self.hl),
            Reg8::HlIndirect => bus.read(self.hl),
            Reg8::A => self.a(),
        }
    }

    #[must_use]
    const fn get_rr(&self, pair: Pair) -> u16 {
        match pair {
            Pair::Af => self.af,
            Pair::Bc => self.bc,
            Pair::De => self.de,
            Pair::Hl => self.hl,
            Pair::Sp => self.sp,
        }
    }

    #[must_use]
    fn imm16(&mut self, bus: &mut impl Bus) -> u16 {
        let lo = self.imm8(bus);
        let hi = self.imm8(bus);
        u16::from_le_bytes([lo, hi])
    }

    #[must_use]
    fn imm8(&mut self, bus: &mut impl Bus) -> u8 {
        let val = bus.read(self.pc);
        self.pc = self.pc.wrapping_add(1);
        val
    }

    /// The signed immediate byte, sign extended.
    #[must_use]
    fn imm8_offset(&mut self, bus: &mut impl Bus) -> u16 {
        i16::from(self.imm8(bus).cast_signed()).cast_unsigned()
    }

    /// SP plus the signed immediate, with the flags of ADD SP,r8 and
    /// LD HL,SP+r8.
    fn sp_plus_imm8(&mut self, bus: &mut impl Bus) -> u16 {
        let sp = self.sp;
        let offset = self.imm8_offset(bus);
        self.af &= 0xFF00;
        if (sp & 0xF) + (offset & 0xF) > 0xF {
            self.af |= HF;
        }
        if (sp & 0xFF) + (offset & 0xFF) > 0xFF {
            self.af |= CF;
        }
        sp.wrapping_add(offset)
    }

    #[must_use]
    fn pop(&mut self, bus: &mut impl Bus) -> u16 {
        let lo = bus.read(self.sp);
        self.sp = self.sp.wrapping_add(1);
        let hi = bus.read(self.sp);
        self.sp = self.sp.wrapping_add(1);
        u16::from_le_bytes([lo, hi])
    }

    /// PUSH rr instruction timing (verified by Mooneye `push_timing` test):
    /// M=0: Instruction decode (implicit)
    /// M=1: Internal delay
    /// M=2: Memory write for high byte
    /// M=3: Memory write for low byte
    fn push(&mut self, bus: &mut impl Bus, val: u16) {
        let [lo, hi] = val.to_le_bytes();

        // M=1: Internal delay with SP on the address bus (DMG OAM bug).
        bus.tick_oam_bug(self.sp);

        // M=2: Write high byte
        self.sp = self.sp.wrapping_sub(1);
        bus.write(self.sp, hi);

        // M=3: Write low byte
        self.sp = self.sp.wrapping_sub(1);
        bus.write(self.sp, lo);
    }

    #[must_use]
    const fn satisfies_branch_condition(&self, op: u8) -> bool {
        match (op >> 3) & 3 {
            0 => self.af & ZF == 0,
            1 => self.af & ZF != 0,
            2 => self.af & CF == 0,
            _ => self.af & CF != 0,
        }
    }

    const fn set_a(&mut self, val: u8) {
        self.af = with_high(self.af, val);
    }

    #[expect(clippy::inline_always, reason = "as `get_r`")]
    #[inline(always)]
    fn set_r(&mut self, bus: &mut impl Bus, op: u8, val: u8) {
        match Reg8::from_op(op) {
            Reg8::B => self.bc = with_high(self.bc, val),
            Reg8::C => self.bc = with_low(self.bc, val),
            Reg8::D => self.de = with_high(self.de, val),
            Reg8::E => self.de = with_low(self.de, val),
            Reg8::H => self.hl = with_high(self.hl, val),
            Reg8::L => self.hl = with_low(self.hl, val),
            Reg8::HlIndirect => bus.write(self.hl, val),
            Reg8::A => self.set_a(val),
        }
    }

    const fn set_rr(&mut self, pair: Pair, val: u16) {
        match pair {
            Pair::Af => self.af = val,
            Pair::Bc => self.bc = val,
            Pair::De => self.de = val,
            Pair::Hl => self.hl = val,
            Pair::Sp => self.sp = val,
        }
    }
}
