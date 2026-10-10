pub mod conflict;
#[cfg(kani)]
mod proofs;

use crate::{
    AudioCallback, Gb, Model,
    interrupts::{INT_MASK, INT_VECTOR_BASE},
    memory::{IE, IF, IO_START, P1, SCX, SwitchHdma, io_addr},
    ppu::{
        LCDC_BG_EN_B, LCDC_BG_MAP_B, LCDC_OBJ_EN_B, LCDC_OBJ_SIZE_B, LCDC_ON_B, LCDC_TILE_SEL_B,
        LCDC_WIN_EN_B, LCDC_WIN_MAP_B, Mode, STAT_IF_HBLANK_B, STAT_IF_LYC_B, STAT_IF_OAM_B,
        STAT_IF_VBLANK_B,
    },
};
use conflict::ConflictType;
use core::mem;

const ZF: u16 = 0x80;
const NF: u16 = 0x40;
const HF: u16 = 0x20;
const CF: u16 = 0x10;

/// The bus the SM83 executes against. The CPU is fully decoupled from the
/// rest of the system: on every M-cycle it either performs one internal
/// cycle (`tick`) or one bus access (`read`/`write`).
///
/// # Timing model
///
/// Time is consumed in M-cycle quanta (4 T-cycles), deferred by the CPU and
/// flushed by the bus at access boundaries:
///
/// - `tick` defers one internal M-cycle's 4 T-cycles.
/// - `read`/`write` first flush any deferred time (so the access observes
///   the machine after all preceding M-cycles of the instruction), perform
///   the access, then defer the access's own 4 T-cycles.
/// - The host flushes whatever remains at the end of each `step`.
pub trait Bus {
    /// One internal (no bus access) M-cycle: defer 4 T-cycles.
    fn tick(&mut self);

    /// Flush all deferred time, perform the bus read at that instant, then
    /// defer the read M-cycle's 4 T-cycles.
    fn read(&mut self, addr: u16) -> u8;

    /// Flush all deferred time, perform the bus write at that instant, then
    /// defer the write M-cycle's 4 T-cycles.
    fn write(&mut self, addr: u16, val: u8);

    /// Interrupt-dispatch tail: flush all but `t_cycles` T-cycles, which
    /// stay deferred across the step boundary.
    fn defer(&mut self, t_cycles: i32);

    /// `(IF & IE) != 0` — some enabled interrupt line is asserted.
    fn interrupts_pending(&self) -> bool;

    /// Raw IF register (upper 3 bits set).
    fn read_if(&self) -> u8;

    /// Raw IE register.
    fn read_ie(&self) -> u8;

    /// Clear the acknowledged interrupt bit in IF.
    fn ack_interrupt(&mut self, bit: u8);

    /// SM83 illegal-opcode behavior: IE is cleared.
    fn clear_ie(&mut self);

    /// Run a pending HDMA transfer chunk, if any.
    fn tick_hdma(&mut self);

    /// An internal M-cycle with `addr` on the address bus: flushes the
    /// deferred time, triggers the DMG OAM bug for `addr`, defers 4 T-cycles.
    fn tick_oam_bug(&mut self, addr: u16);

    /// The DMG OAM bug for an address placed on the bus (no time passes).
    fn trigger_oam_bug(&mut self, addr: u16);

    /// Runs the OAM DMA for the cycles it is owed (0 unless `wake` is set; on
    /// a wake-up the DMA is given one M-cycle).
    fn dma_run(&mut self, wake: bool);

    /// The CPU is about to halt: an OAM DMA whose last step is due takes it
    /// first (a halted DMA does not move, and that step frees OAM).
    fn dma_finish_before_halt(&mut self);

    /// Discards the time deferred so far (it was already accounted for).
    fn drop_deferred(&mut self);

    /// The CPU entered (or, with `false`, left) HALT.
    fn set_halted(&mut self, halted: bool);

    /// Advance the machine by `t_cycles` T-cycles immediately (nothing is
    /// deferred when this is called at the start of a step).
    fn advance(&mut self, t_cycles: i32);

    /// Whether the machine is CGB hardware (regardless of ROM mode).
    fn is_cgb_hardware(&self) -> bool;

    /// Cancel STOP mode (`ppu.leave_stop_mode` + unfreeze the clock).
    fn wake_from_stop(&mut self);

    /// Enter STOP mode: DIV write, DIV freeze when interrupts are disabled,
    /// PPU stop, clock stop. `ime` is the CPU's current IME state.
    fn enter_stop(&mut self, ime: bool);

    /// Flush all deferred time.
    fn flush(&mut self);

    /// Read without consuming time or triggering side effects.
    fn peek(&self, addr: u16) -> u8;

    /// The CPU is in STOP mode (waiting for a joypad press).
    fn is_stopped(&self) -> bool;

    /// KEY1 speed-switch requested (`key1.is_requested`).
    fn speed_switch_requested(&self) -> bool;

    /// Start the CGB speed switch (SameBoy's `stop` speed-switch block).
    fn begin_speed_switch(&mut self, interrupt_pending: bool);

    /// Leave STOP mode without touching the speed-switch halt countdown.
    fn leave_stop(&mut self);

    /// Cancel the post-speed-switch halt.
    fn clear_speed_switch_halt(&mut self);

    /// The post-speed-switch halt expired since the last call.
    fn take_unhalt(&mut self) -> bool;

    /// An HBlank transfer is requested and has not run yet, on a CGB-C
    /// (gambatte's HALT then prefetches the next opcode).
    fn hdma_request_pending(&self) -> bool;

    /// HALT prefetched the next opcode for a pending transfer: the transfer
    /// runs at the wake in the time of that fetch.
    fn note_halt_prefetch(&mut self);
}

#[expect(clippy::struct_excessive_bools, reason = "Independent CPU state flags")]
#[derive(Default)]
pub struct Sm83 {
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
    has_executed_illegal_opcode: bool,
    ld_b_b_breakpoint: bool,
    pc: u16,
    sp: u16,
}

impl Sm83 {
    pub const fn take_illegal_opcode(&mut self) -> bool {
        mem::replace(&mut self.has_executed_illegal_opcode, false)
    }

    pub const fn a(&self) -> u8 {
        (self.af >> 8) as u8
    }

    pub const fn af(&self) -> u16 {
        self.af
    }

    pub const fn bc(&self) -> u16 {
        self.bc
    }

    pub const fn de(&self) -> u16 {
        self.de
    }

    pub const fn hl(&self) -> u16 {
        self.hl
    }

    pub const fn is_halted(&self) -> bool {
        self.is_halted
    }

    pub const fn ime(&self) -> bool {
        self.ime
    }

    pub fn take_ld_b_b_breakpoint(&mut self) -> bool {
        mem::take(&mut self.ld_b_b_breakpoint)
    }

    pub const fn pc(&self) -> u16 {
        self.pc
    }

    pub const fn sp(&self) -> u16 {
        self.sp
    }
}

impl Sm83 {
    /// Set the program counter.
    pub const fn set_pc(&mut self, pc: u16) {
        self.pc = pc;
    }

    pub const fn set_af(&mut self, af: u16) {
        // The low nibble of F is always zero.
        self.af = af & 0xFFF0;
    }

    pub const fn set_bc(&mut self, bc: u16) {
        self.bc = bc;
    }

    pub const fn set_de(&mut self, de: u16) {
        self.de = de;
    }

    pub const fn set_hl(&mut self, hl: u16) {
        self.hl = hl;
    }

    pub const fn set_sp(&mut self, sp: u16) {
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
    /// that: capture the IME value seen by this instruction (effective_ime),
    /// then if EI was pending, flip IME in place. If the instruction is DI
    /// it will clear IME again; if it's anything else, IME will stay true
    /// through subsequent step calls.
    pub fn step<B: Bus>(&mut self, bus: &mut B) {
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
    fn get_r(&self, bus: &mut impl Bus, op: u8) -> u8 {
        let (id, lo) = Self::operand(op);
        if id == 0 {
            if lo { self.a() } else { bus.read(self.hl) }
        } else if lo {
            (self.get_rr(id) & 0xFF) as u8
        } else {
            (self.get_rr(id) >> 8) as u8
        }
    }

    #[must_use]
    const fn get_rr(&self, id: u8) -> u16 {
        match id {
            0 => self.af,
            1 => self.bc,
            2 => self.de,
            3 => self.hl,
            4 => self.sp,
            _ => unreachable!(),
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

    /// The register pair in bits 4-5 of `op`: BC, DE, HL or SP (ids 1 to 4,
    /// as in `get_rr`).
    #[must_use]
    const fn pair_id(op: u8) -> u8 {
        (op >> 4) + 1
    }

    /// The pair in bits 4-5 of `op` with AF (id 0) in place of SP: PUSH, POP
    /// and the registers B, D, H and A, the high halves of the pairs.
    #[must_use]
    const fn pair_id_af(op: u8) -> u8 {
        Self::pair_id(op) & 0x03
    }

    /// Where the 8-bit operand in bits 0-2 of `op` is (`op >> 3` for the
    /// destination of LD r,r'): B C D E H L (HL) A are the high and low
    /// halves of the pairs 1, 2, 3 and 0, (HL) taking the place of F. Returns
    /// the pair id and whether it is the low half.
    #[must_use]
    const fn operand(op: u8) -> (u8, bool) {
        (((op >> 1) + 1) & 3, op & 1 != 0)
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

    fn set_a(&mut self, val: u8) {
        self.af = (u16::from(val) << 8) | (self.af & 0xFF);
    }

    fn set_r(&mut self, bus: &mut impl Bus, op: u8, val: u8) {
        let (id, lo) = Self::operand(op);
        if id == 0 {
            if lo {
                self.set_a(val);
            } else {
                bus.write(self.hl, val);
            }
        } else if lo {
            self.set_rr(id, u16::from(val) | self.get_rr(id) & 0xFF00);
        } else {
            self.set_rr(id, (u16::from(val) << 8) | self.get_rr(id) & 0xFF);
        }
    }

    fn set_rr(&mut self, id: u8, val: u16) {
        match id {
            0 => self.af = val,
            1 => self.bc = val,
            2 => self.de = val,
            3 => self.hl = val,
            4 => self.sp = val,
            _ => unreachable!(),
        }
    }
}

// ALU
impl Sm83 {
    /// The ALU operation selected by bits 3-5 of `op`.
    fn alu(&mut self, op: u8, val: u8) {
        match (op >> 3) & 7 {
            0 => self.add(val, false),
            1 => self.add(val, self.af & CF != 0),
            2 => self.sub(val, false),
            3 => self.sub(val, self.af & CF != 0),
            4 => self.and(val),
            5 => self.xor(val),
            6 => self.or(val),
            _ => self.cp(val),
        }
    }

    fn add(&mut self, val: u8, carry: bool) {
        let val = u16::from(val);
        let a = self.af >> 8;
        let carry = u16::from(carry);
        let res = a + val + carry;
        self.af = res << 8;
        if res.trailing_zeros() >= 8 {
            self.af |= ZF;
        }
        if (a & 0xF) + (val & 0xF) + carry > 0x0F {
            self.af |= HF;
        }
        if res > 0xFF {
            self.af |= CF;
        }
    }

    fn and(&mut self, val: u8) {
        let val = u16::from(val);
        let a = self.af >> 8;
        let a = a & val;
        self.af = (a << 8) | HF;
        if a == 0 {
            self.af |= ZF;
        }
    }

    const fn cp(&mut self, val: u8) {
        let a = self.a();
        self.af &= 0xFF00;
        self.af |= NF;
        if a == val {
            self.af |= ZF;
        }
        if a & 0xF < val & 0xF {
            self.af |= HF;
        }
        if a < val {
            self.af |= CF;
        }
    }

    fn or(&mut self, val: u8) {
        let val = u16::from(val);
        let a = self.af >> 8;
        self.af = (a | val) << 8;
        if a | val == 0 {
            self.af |= ZF;
        }
    }

    fn sub(&mut self, val: u8, carry: bool) {
        let val = u16::from(val);
        let a = self.af >> 8;
        let carry = u16::from(carry);
        let res = a.wrapping_sub(val).wrapping_sub(carry);
        self.af = (res << 8) | NF;
        if res.trailing_zeros() >= 8 {
            self.af |= ZF;
        }
        if (a & 0xF) < (val & 0xF) + carry {
            self.af |= HF;
        }
        if res > 0xFF {
            self.af |= CF;
        }
    }

    fn xor(&mut self, val: u8) {
        let val = u16::from(val);
        let a = self.af >> 8;
        let a = a ^ val;
        self.af = a << 8;
        if a == 0 {
            self.af |= ZF;
        }
    }
}

// Instructions. In the names, r is an 8-bit register and rr a pair, high
// and low are the halves of a pair (B D H A and C E L), dhl is (HL), d8 and
// d16 are immediates, a8 and a16 immediate addresses.
impl Sm83 {
    fn exec(&mut self, bus: &mut impl Bus, op: u8) {
        match op {
            0x00 | 0x5B | 0x6D | 0x7F | 0x49 | 0x52 | 0x64 => self.nop(),
            0x01 | 0x11 | 0x21 | 0x31 => self.ld_rr_d16(bus, op),
            0x02 | 0x12 => self.ld_drr_a(bus, op),
            0x03 | 0x13 | 0x23 | 0x33 => self.inc_rr(bus, op),
            0x04 | 0x14 | 0x24 | 0x3C => self.inc_high(op),
            0x05 | 0x15 | 0x25 | 0x3D => self.dec_high(op),
            0x06 | 0x16 | 0x26 | 0x3E => self.ld_high_d8(bus, op),
            0x07 => self.rlca(),
            0x08 => self.ld_da16_sp(bus),
            0x09 | 0x19 | 0x29 | 0x39 => self.add_hl_rr(bus, op),
            0x0A | 0x1A => self.ld_a_drr(bus, op),
            0x0B | 0x1B | 0x2B | 0x3B => self.dec_rr(bus, op),
            0x0C | 0x1C | 0x2C => self.inc_low(op),
            0x0D | 0x1D | 0x2D => self.dec_low(op),
            0x0E | 0x1E | 0x2E => self.ld_low_d8(bus, op),
            0x0F => self.rrca(),
            0x10 => self.stop(bus),
            0x17 => self.rla(),
            0x18 => self.jr_d(bus),
            0x1F => self.rra(),
            0x20 | 0x28 | 0x30 | 0x38 => self.jr_cc(bus, op),
            0x22 => self.ld_dhli_a(bus),
            0x27 => self.daa(),
            0x2A => self.ld_a_dhli(bus),
            0x2F => self.cpl(),
            0x32 => self.ld_dhld_a(bus),
            0x34 => self.inc_dhl(bus),
            0x35 => self.dec_dhl(bus),
            0x36 => self.ld_dhl_d8(bus),
            0x37 => self.scf(),
            0x3A => self.ld_a_dhld(bus),
            0x3F => self.ccf(),
            0x40 => self.ld_b_b(),
            // The LD r,r' that are not NOPs (matched above).
            0x41..=0x75 | 0x77..=0x7F => self.ld(bus, op),
            0x76 => self.halt(bus),
            0x80..=0xBF => self.alu_a_r(bus, op),
            0xC6 | 0xCE | 0xD6 | 0xDE | 0xE6 | 0xEE | 0xF6 | 0xFE => self.alu_a_d8(bus, op),
            0xC0 | 0xC8 | 0xD0 | 0xD8 => self.ret_cc(bus, op),
            0xC1 | 0xD1 | 0xE1 | 0xF1 => self.pop_rr(bus, op),
            0xC2 | 0xCA | 0xD2 | 0xDA => self.jp_cc(bus, op),
            0xC3 => self.jp_a16(bus),
            0xC4 | 0xCC | 0xD4 | 0xDC => self.call_cc_a16(bus, op),
            0xC5 | 0xD5 | 0xE5 | 0xF5 => self.push_rr(bus, op),
            0xC7 | 0xCF | 0xD7 | 0xDF | 0xE7 | 0xEF | 0xF7 | 0xFF => self.rst(bus, op),
            0xC9 => self.ret(bus),
            0xCB => self.exec_cb(bus),
            0xCD => self.call_nn(bus),
            0xD9 => self.reti(bus),
            0xE0 => self.ldh_da8_a(bus),
            0xE2 => self.ldh_dc_a(bus),
            0xE8 => self.add_sp_r8(bus),
            0xE9 => self.jp_hl(),
            0xEA => self.ld_da16_a(bus),
            0xF0 => self.ldh_a_da8(bus),
            0xF2 => self.ldh_a_dc(bus),
            0xF3 => self.di(),
            0xF8 => self.ld_hl_sp_r8(bus),
            0xF9 => self.ld16_sp_hl(bus),
            0xFA => self.ld_a_da16(bus),
            0xFB => self.ei(),
            _ => self.illegal(bus),
        }
    }

    fn exec_cb(&mut self, bus: &mut impl Bus) {
        let op = self.imm8(bus);
        if op < 0x40 {
            self.shift_r(bus, op);
        } else {
            self.bit_r(bus, op);
        }
    }

    fn alu_a_d8(&mut self, bus: &mut impl Bus, op: u8) {
        let val = self.imm8(bus);
        self.alu(op, val);
    }

    fn alu_a_r(&mut self, bus: &mut impl Bus, op: u8) {
        let val = self.get_r(bus, op);
        self.alu(op, val);
    }

    /// The CB rotates, shifts and SWAP.
    fn shift_r(&mut self, bus: &mut impl Bus, op: u8) {
        let val = self.get_r(bus, op);
        let carry_in = self.af & CF != 0;
        let (res, carry) = match op >> 3 {
            // RLC, RRC
            0 => (val.rotate_left(1), val & 0x80 != 0),
            1 => (val.rotate_right(1), val & 1 != 0),
            // RL, RR
            2 => ((val << 1) | u8::from(carry_in), val & 0x80 != 0),
            3 => ((val >> 1) | (u8::from(carry_in) << 7), val & 1 != 0),
            // SLA, SRA
            4 => (val << 1, val & 0x80 != 0),
            5 => ((val >> 1) | (val & 0x80), val & 1 != 0),
            // SWAP, SRL
            6 => (val.rotate_left(4), false),
            _ => (val >> 1, val & 1 != 0),
        };
        self.set_r(bus, op, res);
        self.af &= 0xFF00;
        if carry {
            self.af |= CF;
        }
        if res == 0 {
            self.af |= ZF;
        }
    }

    fn add_hl_rr(&mut self, bus: &mut impl Bus, op: u8) {
        let id = Self::pair_id(op);
        let hl = self.hl;
        let rr = self.get_rr(id);
        let (res, carry) = hl.overflowing_add(rr);
        self.hl = res;

        self.af &= !(NF | CF | HF);
        if (hl & 0xFFF) + (rr & 0xFFF) > 0xFFF {
            self.af |= HF;
        }
        if carry {
            self.af |= CF;
        }

        bus.tick();
    }

    fn add_sp_r8(&mut self, bus: &mut impl Bus) {
        let res = self.sp_plus_imm8(bus);
        bus.tick();
        bus.tick();
        self.sp = res;
    }

    fn bit_r(&mut self, bus: &mut impl Bus, op: u8) {
        let val = self.get_r(bus, op);
        let bit = 1 << ((op >> 3) & 7);
        match op & 0xC0 {
            0x40 => {
                // BIT
                self.af &= 0xFF00 | CF;
                self.af |= HF;
                if bit & val == 0 {
                    self.af |= ZF;
                }
            }
            0x80 => self.set_r(bus, op, val & !bit),
            _ => self.set_r(bus, op, val | bit),
        }
    }

    fn call_cc_a16(&mut self, bus: &mut impl Bus, op: u8) {
        if self.satisfies_branch_condition(op) {
            self.do_call(bus);
        } else {
            let _nn = self.imm16(bus);
        }
    }

    fn call_nn(&mut self, bus: &mut impl Bus) {
        self.do_call(bus);
    }

    const fn ccf(&mut self) {
        self.af ^= CF;
        self.af &= !(HF | NF);
    }

    const fn cpl(&mut self) {
        self.af ^= 0xFF00;
        self.af |= HF | NF;
    }

    const fn daa(&mut self) {
        let a = {
            let mut a = self.af >> 8;

            if self.af & NF == 0 {
                if self.af & HF != 0 || a & 0x0F > 0x09 {
                    a += 0x06;
                }
                if self.af & CF != 0 || a > 0x9F {
                    a += 0x60;
                }
            } else {
                if self.af & HF != 0 {
                    a = a.wrapping_sub(0x06) & 0xFF;
                }
                if self.af & CF != 0 {
                    a = a.wrapping_sub(0x60);
                }
            }

            a
        };

        self.af &= !(0xFF00 | ZF | HF);

        if a.trailing_zeros() >= 8 {
            self.af |= ZF;
        }

        if a & 0x100 == 0x100 {
            self.af |= CF;
        }

        self.af |= a << 8;
    }

    fn dec_dhl(&mut self, bus: &mut impl Bus) {
        let val = bus.read(self.hl).wrapping_sub(1);
        bus.write(self.hl, val);

        self.af &= !(ZF | HF);
        self.af |= NF;
        if (val & 0x0F) == 0x0F {
            self.af |= HF;
        }

        if val == 0 {
            self.af |= ZF;
        }
    }

    fn dec_high(&mut self, op: u8) {
        let id = Self::pair_id_af(op);
        let rr = self.get_rr(id).wrapping_sub(0x100);
        self.set_rr(id, rr);
        self.af &= !(ZF | HF);
        self.af |= NF;

        if rr & 0x0F00 == 0xF00 {
            self.af |= HF;
        }

        if rr & 0xFF00 == 0 {
            self.af |= ZF;
        }
    }

    fn dec_low(&mut self, op: u8) {
        let id = Self::pair_id(op);
        let val = self.get_rr(id).wrapping_sub(1) & 0xFF;
        let rr = self.get_rr(id) & 0xFF00 | val;
        self.set_rr(id, rr);

        self.af &= !(ZF | HF);
        self.af |= NF;

        if rr & 0x0F == 0xF {
            self.af |= HF;
        }

        if rr.trailing_zeros() >= 8 {
            self.af |= ZF;
        }
    }

    fn dec_rr(&mut self, bus: &mut impl Bus, op: u8) {
        let id = Self::pair_id(op);
        bus.tick_oam_bug(self.get_rr(id));
        self.set_rr(id, self.get_rr(id).wrapping_sub(1));
    }

    const fn di(&mut self) {
        // DI is NOT delayed, not even on a CGB.
        self.ime = false;
    }

    const fn ei(&mut self) {
        // EI disables interrupts for one more instruction, then enables them.
        if !self.ime && !self.ime_toggle {
            self.ime_toggle = true;
        }
    }

    fn halt(&mut self, bus: &mut impl Bus) {
        // A dummy read at PC flushes the fetch M-cycle before the interrupt
        // lines are sampled; the read's own M-cycle is not charged.
        let next = bus.read(self.pc);
        bus.drop_deferred();

        // The HALT bug also happens on a CGB, in both CGB and DMG modes.
        if bus.interrupts_pending() {
            self.is_halted = false;
            if self.ime {
                self.pc = self.pc.wrapping_sub(1);
            } else {
                self.halt_bug = true;
            }
        } else {
            // A transfer requested during the HALT's fetch has the next
            // opcode prefetched: it runs twice, as with the HALT bug.
            if bus.hdma_request_pending() {
                self.halt_bug = true;
                self.prefetched = Some(next);
                bus.note_halt_prefetch();
            }
            bus.dma_finish_before_halt();
            self.is_halted = true;
            bus.set_halted(true);
        }
        self.just_halted = true;
    }

    fn illegal(&mut self, bus: &mut impl Bus) {
        bus.clear_ie();
        self.is_halted = true;
        bus.set_halted(true);
        self.has_executed_illegal_opcode = true;
    }

    fn inc_dhl(&mut self, bus: &mut impl Bus) {
        let val = bus.read(self.hl).wrapping_add(1);
        bus.write(self.hl, val);

        self.af &= !(NF | ZF | HF);
        if val.trailing_zeros() >= 4 {
            self.af |= HF;
        }

        if val == 0 {
            self.af |= ZF;
        }
    }

    fn inc_high(&mut self, op: u8) {
        let id = Self::pair_id_af(op);
        let rr = self.get_rr(id).wrapping_add(0x100);
        self.set_rr(id, rr);
        self.af &= !(NF | ZF | HF);

        if rr & 0x0F00 == 0 {
            self.af |= HF;
        }

        if rr & 0xFF00 == 0 {
            self.af |= ZF;
        }
    }

    fn inc_low(&mut self, op: u8) {
        let id = Self::pair_id(op);
        let val = self.get_rr(id).wrapping_add(1) & 0xFF;
        let rr = self.get_rr(id) & 0xFF00 | val;
        self.set_rr(id, rr);

        self.af &= !(NF | ZF | HF);

        if rr.trailing_zeros() >= 4 {
            self.af |= HF;
        }

        if rr.trailing_zeros() >= 8 {
            self.af |= ZF;
        }
    }

    fn inc_rr(&mut self, bus: &mut impl Bus, op: u8) {
        let id = Self::pair_id(op);
        bus.tick_oam_bug(self.get_rr(id));
        self.set_rr(id, self.get_rr(id).wrapping_add(1));
    }

    fn jp_a16(&mut self, bus: &mut impl Bus) {
        self.do_jump_to_immediate(bus);
    }

    fn jp_cc(&mut self, bus: &mut impl Bus, op: u8) {
        if self.satisfies_branch_condition(op) {
            self.do_jump_to_immediate(bus);
        } else {
            let _nn = self.imm16(bus);
        }
    }

    const fn jp_hl(&mut self) {
        self.pc = self.hl;
    }

    fn jr_cc(&mut self, bus: &mut impl Bus, op: u8) {
        if self.satisfies_branch_condition(op) {
            self.do_jump_relative(bus);
        } else {
            let _discard_byte = self.imm8(bus);
        }
    }

    fn jr_d(&mut self, bus: &mut impl Bus) {
        self.do_jump_relative(bus);
    }

    fn ld(&mut self, bus: &mut impl Bus, op: u8) {
        let val = self.get_r(bus, op);
        self.set_r(bus, op >> 3, val);
    }

    fn ld16_sp_hl(&mut self, bus: &mut impl Bus) {
        let val = self.hl;
        bus.tick_oam_bug(val);
        self.sp = val;
    }

    fn ld_a_da16(&mut self, bus: &mut impl Bus) {
        let addr = self.imm16(bus);
        let val = bus.read(addr);
        self.set_a(val);
    }

    fn ld_a_dhld(&mut self, bus: &mut impl Bus) {
        let val = bus.read(self.hl);
        self.set_a(val);
        self.hl = self.hl.wrapping_sub(1);
    }

    fn ld_a_dhli(&mut self, bus: &mut impl Bus) {
        let val = bus.read(self.hl);
        self.set_a(val);
        self.hl = self.hl.wrapping_add(1);
    }

    fn ld_a_drr(&mut self, bus: &mut impl Bus, op: u8) {
        let addr = self.get_rr(Self::pair_id(op));
        let val = bus.read(addr);
        self.set_a(val);
    }

    // Sets the debug breakpoint flag. Test ROMs like cgb-acid2 and dmg-acid2
    // use this instruction as a breakpoint to signal test completion.
    const fn ld_b_b(&mut self) {
        self.ld_b_b_breakpoint = true;
        self.nop();
    }

    fn ld_da16_a(&mut self, bus: &mut impl Bus) {
        let addr = self.imm16(bus);
        bus.write(addr, self.a());
    }

    fn ld_da16_sp(&mut self, bus: &mut impl Bus) {
        let val = self.sp;
        let addr = self.imm16(bus);
        bus.write(addr, (val & 0xFF) as u8);
        bus.write(addr.wrapping_add(1), (val >> 8) as u8);
    }

    fn ld_dhl_d8(&mut self, bus: &mut impl Bus) {
        let tmp = self.imm8(bus);
        bus.write(self.hl, tmp);
    }

    fn ld_dhld_a(&mut self, bus: &mut impl Bus) {
        let addr = self.hl;
        bus.write(addr, self.a());
        self.hl = addr.wrapping_sub(1);
    }

    fn ld_dhli_a(&mut self, bus: &mut impl Bus) {
        let addr = self.hl;
        bus.write(addr, self.a());
        self.hl = addr.wrapping_add(1);
    }

    fn ld_drr_a(&self, bus: &mut impl Bus, op: u8) {
        let id = Self::pair_id(op);
        let addr = self.get_rr(id);
        bus.write(addr, self.a());
    }

    fn ld_hl_sp_r8(&mut self, bus: &mut impl Bus) {
        let res = self.sp_plus_imm8(bus);
        bus.tick();
        self.hl = res;
    }

    fn ld_high_d8(&mut self, bus: &mut impl Bus, op: u8) {
        let id = Self::pair_id_af(op);
        let hi = u16::from(self.imm8(bus));
        self.set_rr(id, (hi << 8) | self.get_rr(id) & 0xFF);
    }

    fn ld_low_d8(&mut self, bus: &mut impl Bus, op: u8) {
        let id = Self::pair_id(op);
        let lo = u16::from(self.imm8(bus));
        self.set_rr(id, self.get_rr(id) & 0xFF00 | lo);
    }

    fn ld_rr_d16(&mut self, bus: &mut impl Bus, op: u8) {
        let id = Self::pair_id(op);
        let imm = self.imm16(bus);
        self.set_rr(id, imm);
    }

    fn ldh_a_da8(&mut self, bus: &mut impl Bus) {
        let addr = IO_START | u16::from(self.imm8(bus));
        let val = bus.read(addr);
        self.set_a(val);
    }

    fn ldh_a_dc(&mut self, bus: &mut impl Bus) {
        let val = bus.read(IO_START | self.bc & 0xFF);
        self.set_a(val);
    }

    fn ldh_da8_a(&mut self, bus: &mut impl Bus) {
        let tmp = u16::from(self.imm8(bus));
        let a = self.a();
        bus.write(IO_START | tmp, a);
    }

    fn ldh_dc_a(&self, bus: &mut impl Bus) {
        bus.write(IO_START | self.bc & 0xFF, self.a());
    }

    #[expect(clippy::unused_self)]
    const fn nop(&self) {}

    fn pop_rr(&mut self, bus: &mut impl Bus, op: u8) {
        let val = self.pop(bus);
        let id = Self::pair_id_af(op);
        self.set_rr(id, val);
        self.af &= 0xFFF0;
    }

    fn push_rr(&mut self, bus: &mut impl Bus, op: u8) {
        let id = Self::pair_id_af(op);
        self.push(bus, self.get_rr(id));
    }

    fn ret(&mut self, bus: &mut impl Bus) {
        self.pc = self.pop(bus);
        bus.tick();
    }

    fn ret_cc(&mut self, bus: &mut impl Bus, op: u8) {
        bus.tick();

        if self.satisfies_branch_condition(op) {
            self.ret(bus);
        }
    }

    fn reti(&mut self, bus: &mut impl Bus) {
        self.ret(bus);
        self.ime = true;
    }

    fn rla(&mut self) {
        let bit7 = self.af & 0x8000 != 0;
        let carry = self.af & CF != 0;

        self.af = ((self.af & 0xFF00) << 1) | (u16::from(carry) << 8);

        if bit7 {
            self.af |= CF;
        }
    }

    const fn rlca(&mut self) {
        let carry = (self.af & 0x8000) != 0;

        self.af = (self.af & 0xFF00) << 1;
        if carry {
            self.af |= CF | 0x0100;
        }
    }

    fn rra(&mut self) {
        let bit1 = self.af & 0x0100 != 0;
        let carry = self.af & CF != 0;

        self.af = (self.af >> 1) & 0xFF00 | (u16::from(carry) << 15);
        if bit1 {
            self.af |= CF;
        }
    }

    const fn rrca(&mut self) {
        let carry = self.af & 0x100 != 0;
        self.af = (self.af >> 1) & 0xFF00;
        if carry {
            self.af |= CF | 0x8000;
        }
    }

    fn rst(&mut self, bus: &mut impl Bus, op: u8) {
        self.push(bus, self.pc);
        self.pc = u16::from(op) ^ 0xC7;
    }

    const fn scf(&mut self) {
        self.af |= CF;
        self.af &= !(HF | NF);
    }

    fn stop(&mut self, bus: &mut impl Bus) {
        // Port of SameBoy's `stop`.
        bus.flush();
        bus.peek(self.pc);

        let exit_by_joyp = bus.peek(io_addr(P1)) & 0xF != 0xF;
        let speed_switch = bus.speed_switch_requested() && !exit_by_joyp;
        let immediate_exit = speed_switch || exit_by_joyp;
        let interrupt_pending = bus.interrupts_pending();

        if !exit_by_joyp {
            if !immediate_exit {
                bus.dma_run(false);
            }
            bus.enter_stop(self.ime);
        }
        // A speed switch with an HBlank transfer requested prefetches the
        // second byte of STOP, which then runs as an opcode (gambatte).
        let prefetch = speed_switch && bus.hdma_request_pending();

        // When entering with IF&IE set, the second byte of STOP is actually
        // executed.
        if !interrupt_pending {
            let operand = bus.read(self.pc);
            if prefetch {
                self.prefetched = Some(operand);
            }
            if !prefetch {
                self.pc = self.pc.wrapping_add(1);
            }
        }

        if speed_switch {
            bus.begin_speed_switch(interrupt_pending);
        }

        if immediate_exit {
            bus.leave_stop();
            bus.dma_run(true);
            if interrupt_pending {
                bus.clear_speed_switch_halt();
            } else {
                bus.dma_run(false);
                self.is_halted = true;
                self.just_halted = true;
                bus.set_halted(true);
                if speed_switch {
                    // A transfer requested before the CPU stopped goes on
                    // during the wait of the speed switch.
                    bus.tick_hdma();
                }
            }
        }
    }
}

impl<A: AudioCallback> Gb<A> {
    /// Runs exactly one CPU step (one instruction, interrupt dispatch or
    /// HALT M-cycle), then flushes any bus time the step left deferred.
    /// The CPU is invoked via `mem::take` so `self` can be handed to it
    /// wholesale as its `Bus` without overlapping borrows.
    #[inline]
    pub fn run_cpu(&mut self) {
        let mut cpu = mem::take(&mut self.cpu);
        cpu.step(self);
        self.flush_deferred_time();
        self.cpu = cpu;
    }
}

impl<A: AudioCallback> Gb<A> {
    /// Advances the machine by any bus time a CPU step deferred but never
    /// consumed (only the interrupt dispatch's 2-T-cycle tail can survive
    /// across a step boundary).
    #[inline]
    fn flush_deferred_time(&mut self) {
        if self.time_deferred != 0 {
            self.advance_t_cycles(self.time_deferred);
            self.time_deferred = 0;
        }
    }
}

impl<A: AudioCallback> Bus for Gb<A> {
    #[inline]
    fn tick(&mut self) {
        self.time_deferred += 4;
    }

    #[inline]
    fn read(&mut self, addr: u16) -> u8 {
        self.flush_deferred_time();
        self.address_bus = addr;
        let val = self.cpu_read_mem(addr);
        self.time_deferred = 4;
        val
    }

    #[inline]
    #[expect(
        clippy::too_many_lines,
        reason = "One arm per register class, like SameBoy's `cycle_write`"
    )]
    fn write(&mut self, addr: u16, val: u8) {
        let conflict = conflict::get_conflict(self.model, self.key1.is_enabled(), addr);

        let pending = self.time_deferred;

        // Port of SameBoy's `cycle_write`: each class says when, relative to
        // the end of the pending M-cycles, the PPU sees the new value. The
        // write's own M-cycle is always 4 dots: what an arm advances past
        // `pending` plus what it leaves in `time_deferred` (a write landing a
        // dot early defers 5, one landing a dot late 3).
        match conflict {
            ConflictType::ReadOld => {
                self.flush_deferred_time();
                self.write_mem(addr, val);
                self.time_deferred = 4;
            }
            ConflictType::ReadNew => {
                self.advance_t_cycles(pending - 1);
                self.write_mem(addr, val);
                self.time_deferred = 5;
            }
            ConflictType::WriteCpu => {
                self.advance_t_cycles(pending + 1);
                // In double speed a write to IF lands after the LCD
                // interrupts of the next cycle (gambatte `updateIrqs(cc + 2)`).
                if addr == io_addr(IF) && self.key1.is_enabled() && self.ppu.gambatte_stat() {
                    self.ppu
                        .run_ahead(&mut self.ints, self.cgb_mode, true, 1, true);
                }
                self.write_mem(addr, val);
                self.time_deferred = 3;
            }
            // The DMG STAT-write bug is basically the STAT register being
            // read as FF for a single T-cycle.
            ConflictType::StatDmg if self.ppu.gambatte_irq() => {
                // The STAT write bug is in the PPU's STAT interrupt events.
                self.flush_deferred_time();
                self.write_mem(addr, val);
                self.time_deferred = 4;
            }
            ConflictType::StatDmg => {
                self.flush_deferred_time();
                // The write glitches the register for a dot (all the enables are
                // on) before the real value lands. The glitch is a pulse, the
                // real value is in place for what the PPU does in the next dot,
                // except at the edge between HBlank and OAM mode, where the OAM
                // interrupt seems to be blocked by HBlank interrupts.
                let stat = self.ppu.read_stat();
                if self.ppu.at_oam_scan_edge()
                    && stat & (STAT_IF_OAM_B | STAT_IF_HBLANK_B) == STAT_IF_HBLANK_B
                {
                    self.write_mem(addr, !STAT_IF_OAM_B);
                    self.advance_t_cycles(1);
                    self.write_mem(addr, val);
                } else {
                    self.write_mem(addr, 0xFF);
                    self.write_mem(addr, val);
                    self.advance_t_cycles(1);
                }
                self.time_deferred = 3;
            }
            ConflictType::StatCgb => {
                // The LYC and the VBlank enables reach the PPU a dot after the
                // others (the HBlank one too when it is turned off).
                const LATE: u8 = STAT_IF_LYC_B | STAT_IF_VBLANK_B;

                let old = self.ppu.read_stat();
                self.flush_deferred_time();
                let mut early = (old & LATE) | (val & !LATE);
                early |= old & !val & STAT_IF_HBLANK_B;
                if val & !old & STAT_IF_LYC_B != 0 {
                    // Enabling the LYC source: the enables this write clears go
                    // with it, so that no source drops out for a dot in between.
                    early |= old & (STAT_IF_HBLANK_B | STAT_IF_VBLANK_B | STAT_IF_OAM_B);
                }
                self.write_mem(addr, early);
                self.advance_t_cycles(1);
                self.write_mem(addr, val);
                self.time_deferred = 3;
            }
            ConflictType::StatCgbDouble => {
                let old = self.ppu.read_stat();
                self.flush_deferred_time();
                self.write_mem(addr, (val & !STAT_IF_HBLANK_B) | (old & STAT_IF_HBLANK_B));
                self.advance_t_cycles(1);
                self.write_mem(addr, val);
                self.time_deferred = 3;
            }
            ConflictType::PaletteDmg => {
                self.advance_t_cycles(pending - 2);
                let old = self.read_mem(addr);
                self.write_mem(addr, val | old);
                self.advance_t_cycles(1);
                self.write_mem(addr, val);
                self.time_deferred = 5;
            }
            ConflictType::PaletteCgb => {
                if matches!(self.model, Model::CgbD | Model::CgbE | Model::Agb) {
                    self.advance_t_cycles(pending - 2);
                    self.write_mem(addr, val);
                    self.time_deferred = 6;
                } else {
                    self.advance_t_cycles(pending - 1);
                    self.write_mem(addr, val);
                    self.time_deferred = 5;
                }
            }
            // LCDC.1 is read both by the FIFO when popping pixels and by the
            // object-fetching state machine, and the two behave differently
            // when it comes to access conflicts.
            ConflictType::DmgLcdc => {
                // Bits the tile fetcher consumes (BG_MAP, TILE_SEL, WIN_MAP)
                // are seen by the PPU one dot before the ones the pixel mixer
                // consumes (BG_EN, WIN_EN, OBJ_EN): measured on DMG against
                // the mealybug LCDC tests. OBJ_SIZE reaches the object fetch
                // early too, but the object search only sees it with the rest.
                const FETCHER_BITS: u8 = LCDC_BG_MAP_B | LCDC_TILE_SEL_B | LCDC_WIN_MAP_B;

                let mut old = self.read_mem(addr);
                self.advance_t_cycles(pending - 2);
                if (self.model != Model::Mgb && self.ppu.fifo_position() == 0
                    || self.ppu.is_fetching_sprite())
                    && val & LCDC_OBJ_EN_B == 0
                {
                    old &= !LCDC_OBJ_EN_B;
                }

                self.write_mem(addr, (old & !FETCHER_BITS) | (val & FETCHER_BITS));
                // The object fetch (not the object search) sees OBJ_SIZE early.
                self.ppu.set_obj_size_fetch(val & LCDC_OBJ_SIZE_B != 0);
                self.advance_t_cycles(1);
                self.write_mem(addr, val);

                self.ppu.note_window_disable(old, val);
                self.time_deferred = 5;
            }
            ConflictType::SgbLcdc => {
                // Simplified version of the above.
                let old = self.read_mem(addr);
                self.advance_t_cycles(pending - 2);
                // Hack to force aborting an object fetch.
                self.write_mem(addr, val);
                self.write_mem(addr, old);
                self.advance_t_cycles(1);
                self.write_mem(addr, val);
                self.time_deferred = 5;
            }
            ConflictType::WxDmg => {
                self.advance_t_cycles(pending);
                self.write_mem(addr, val);
                self.ppu.set_wx_just_changed(true);
                self.advance_t_cycles(1);
                self.ppu.set_wx_just_changed(false);
                self.time_deferred = 3;
            }
            ConflictType::LcdcCgb => {
                // OBJ_SIZE reaches the object fetcher one dot after the other
                // bits reach the PPU (measured on CGB-C).
                let old = self.ppu.read_lcdc();
                self.advance_t_cycles(pending);
                self.ppu.cgb_obj_size_write(val, 0);
                if self.ppu.gambatte_stat() {
                    self.ppu.gstat_write_lcdc(val, 0);
                }
                let delay_obj_size = self.ppu.read_scx() & 7 != 0;
                self.write_mem(
                    addr,
                    if delay_obj_size {
                        (val & !LCDC_OBJ_SIZE_B) | (old & LCDC_OBJ_SIZE_B)
                    } else {
                        val
                    },
                );
                // Changing TILE_SEL on the dot after the write can corrupt a
                // bitplane read in flight (see the PPU). The window start sees
                // the window being turned on a dot late.
                self.ppu
                    .set_tile_sel_glitch((val ^ old) & LCDC_TILE_SEL_B != 0);
                self.ppu.set_window_enable_pending(
                    old & LCDC_WIN_EN_B == 0 && val & LCDC_WIN_EN_B != 0,
                );
                self.advance_t_cycles(1);
                self.ppu.set_tile_sel_glitch(false);
                self.ppu.set_window_enable_pending(false);
                self.write_mem(addr, val);
                self.ppu.gstat_lcdc_write_done();
                self.time_deferred = 3;
            }
            ConflictType::LcdcCgbDouble => {
                let old = self.ppu.read_lcdc();
                self.advance_t_cycles(pending - 2);
                self.ppu.cgb_obj_size_write(val, 2);
                if self.ppu.gambatte_stat() {
                    self.ppu.gstat_write_lcdc(val, 2);
                }
                // Turning the window on waits for the end of the write too.
                let late = LCDC_ON_B | LCDC_BG_EN_B | (!old & LCDC_WIN_EN_B);
                self.write_mem(addr, (val & !late) | (old & late));
                self.ppu
                    .set_tile_sel_glitch((val ^ old) & LCDC_TILE_SEL_B != 0);
                self.advance_t_cycles(2);
                self.ppu.set_tile_sel_glitch(false);
                self.write_mem(addr, val);
                self.ppu.gstat_lcdc_write_done();
                self.time_deferred = 4;
            }
            // Registers the tile fetcher consumes land a dot before the ones the
            // pixel mixer consumes (see `DmgLcdc`).
            ConflictType::ScxDmgAndCgbDouble | ConflictType::ScyDmg => {
                // The tile fetcher sees these two dots before the pixel mixer.
                // On the DMG the mixer, which discards SCX's low bits, sees them
                // a dot before the end of the write.
                let old = self.ppu.read_scx();
                self.advance_t_cycles(pending - 2);
                if self.model.is_cgb_hardware() || addr != io_addr(SCX) {
                    self.write_mem(addr, val);
                    self.time_deferred = 6;
                } else {
                    self.write_mem(addr, (old & 7) | (val & !7));
                    self.advance_t_cycles(1);
                    self.write_mem(addr, val);
                    self.time_deferred = 5;
                }
            }
            ConflictType::Nr10CgbDouble => {
                self.advance_t_cycles(pending - 1);
                self.advance_t_cycles(1);
                self.write_mem(addr, val);
                self.time_deferred = 4;
            }
        }
        self.address_bus = addr;
    }

    #[inline]
    fn defer(&mut self, t_cycles: i32) {
        let flush = self.time_deferred - t_cycles;
        if flush > 0 {
            self.advance_t_cycles(flush);
        }
        self.time_deferred = t_cycles;
    }

    fn interrupts_pending(&self) -> bool {
        self.ints.is_any_requested()
    }

    fn read_if(&self) -> u8 {
        self.ints.read_if()
    }

    fn read_ie(&self) -> u8 {
        self.ints.read_ie()
    }

    fn ack_interrupt(&mut self, bit: u8) {
        // Gambatte runs the LCD two cycles ahead of an acknowledge: the LYC
        // interrupt that the compare around line 153 raises that soon counts
        // as already requested and is swallowed by the acknowledge. (In double
        // speed the compare itself comes early enough.)
        if !self.key1.is_enabled() {
            self.ppu
                .run_ahead(&mut self.ints, self.cgb_mode, false, 2, false);
        }
        // On the CGB a timer interrupt due within the next cycles counts as
        // already requested: acknowledging the timer bit swallows it.
        if self.model.is_cgb_hardware() {
            let headroom = 3 + self.time_deferred;
            if (1..=headroom).contains(&i32::from(self.clock.tima_irq_countdown)) {
                self.clock.tima_irq_countdown = 0;
                self.ints.request_timer();
            }
            // The serial port is looked ahead too.
            self.serial.complete_if_due(
                self.clock.div,
                u16::try_from(3 + self.time_deferred).unwrap_or(0),
                &mut self.ints,
            );
        }
        self.ints.acknowledge_interrupt(bit);
    }

    fn clear_ie(&mut self) {
        self.ints.illegal();
    }

    fn tick_hdma(&mut self) {
        if self.hdma.is_on() {
            if mem::take(&mut self.hdma_halt_prefetch) {
                self.time_deferred -= 4;
            }
            self.run_hdma();
        }
    }

    fn tick_oam_bug(&mut self, addr: u16) {
        self.flush_deferred_time();
        self.address_bus = addr;
        self.ppu.trigger_oam_bug(addr);
        self.time_deferred = 4;
    }

    fn trigger_oam_bug(&mut self, addr: u16) {
        self.ppu.trigger_oam_bug(addr);
    }

    fn dma_run(&mut self, wake: bool) {
        if wake {
            self.dma.set_cycles(4);
        }
        self.run_dma();
    }

    fn dma_finish_before_halt(&mut self) {
        if self.dma.is_in_last_step() {
            self.dma_run(true);
        }
    }

    fn drop_deferred(&mut self) {
        self.time_deferred = 0;
    }

    fn set_halted(&mut self, halted: bool) {
        let hblank = self.ppu.hdma_period();
        self.hdma.set_cpu_halted(halted, hblank);
        self.ppu
            .set_cpu_idle(self.hdma.cpu_halted() || self.clock.stopped);
    }

    fn advance(&mut self, t_cycles: i32) {
        self.advance_t_cycles(t_cycles);
    }

    fn is_cgb_hardware(&self) -> bool {
        self.model.is_cgb_hardware()
    }

    fn wake_from_stop(&mut self) {
        self.leave_stop();
        self.speed_switch.halt_countdown = 0;
    }

    fn leave_stop(&mut self) {
        self.ppu.leave_stop_mode();
        self.clock.stopped = false;
        let hblank = self.ppu.hdma_period();
        self.hdma.set_cpu_halted(false, hblank);
        // A speed switch does not request the HBlank transfer (gambatte).
        let switching = self.ppu.gambatte_stat() && self.speed_switch.halt_countdown != 0;
        if !switching {
            self.hdma.wake(hblank);
        }
        self.ppu.set_cpu_idle(false);
    }

    fn flush(&mut self) {
        self.flush_deferred_time();
    }

    fn peek(&self, addr: u16) -> u8 {
        self.read_mem(addr)
    }

    fn is_stopped(&self) -> bool {
        self.clock.stopped
    }

    fn clear_speed_switch_halt(&mut self) {
        self.speed_switch.halt_countdown = 0;
    }

    fn note_halt_prefetch(&mut self) {
        self.hdma_halt_prefetch = true;
    }

    fn hdma_request_pending(&self) -> bool {
        self.ppu.gambatte_stat() && self.hdma.hblank_requested()
    }

    fn take_unhalt(&mut self) -> bool {
        let unhalt = mem::take(&mut self.speed_switch.unhalt);
        if unhalt && self.ppu.gambatte_stat() {
            // The wake of a speed switch requests the HBlank transfer
            // (gambatte's `intevent_unhalt`).
            let period =
                self.hdma.hblank_enabled() && self.ppu.gstat_hdma_period(0).unwrap_or(false);
            if (period && self.hdma.switch_state() == SwitchHdma::Low)
                || self.hdma.switch_state() == SwitchHdma::Requested
            {
                self.hdma.request_hblank();
            }
            if self.hdma.switch_state() == SwitchHdma::Requested {
                // The transfer dropped at the switch runs in the time of the
                // opcode prefetched then.
                self.hdma_halt_prefetch = true;
            }
        }
        unhalt
    }

    fn enter_stop(&mut self, ime: bool) {
        if self.ppu.gambatte_stat() && self.key1.is_requested() {
            let period =
                self.hdma.hblank_enabled() && self.ppu.gstat_hdma_period(0).unwrap_or(false);
            self.hdma
                .set_switch_state(if self.hdma.hblank_requested() && self.key1.is_enabled() {
                    SwitchHdma::Requested
                } else if period {
                    SwitchHdma::High
                } else {
                    SwitchHdma::Low
                });
        }
        self.tima_speed_change_catch_up();
        self.write_div();
        if !ime {
            self.clock.div_cycles = -4;
        }
        self.ppu.enter_stop_mode();
        self.clock.stopped = true;
        self.ppu.set_cpu_idle(true);
        self.hdma.note_stop(matches!(self.ppu.mode(), Mode::HBlank));
    }

    fn speed_switch_requested(&self) -> bool {
        self.key1.is_requested()
    }

    fn begin_speed_switch(&mut self, interrupt_pending: bool) {
        self.flush_deferred_time();
        if self.ppu.gambatte_stat() && self.key1.is_enabled() {
            // gambatte's `Memory::stop`: a pending HBlank transfer survives a
            // switch to double speed (it runs during the halt); leaving double
            // speed drops it and requests it again at the wake.
            self.hdma.ack_hblank_request();
        }
        if self.key1.is_enabled() {
            self.key1.set_double_speed(false);
            self.left_double_speed();
        } else {
            self.speed_switch.countdown = 6;
            self.speed_switch.freeze = 1;
        }
        if !interrupt_pending {
            self.speed_switch.halt_countdown = 0x20008;
            self.speed_switch.freeze = 5;
        }
        self.key1.clear_request();
    }
}
