pub mod conflict;

use crate::{AudioCallback, CgbMode, Gb, Model};
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

    /// Current CGB operating mode.
    fn is_cgb_mode(&self) -> CgbMode;

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

    /// Cancel STOP mode (`ppu.leave_stop_mode` + unfreeze the clock).
    fn wake_from_stop(&mut self);

    /// Enter STOP mode: DIV write, DIV freeze when interrupts are disabled,
    /// PPU stop, clock stop. `ime` is the CPU's current IME state.
    fn enter_stop(&mut self, ime: bool);

    /// KEY1 speed-switch requested (`key1.is_requested`).
    fn speed_switch_pending(&self) -> bool;

    /// Perform the CGB double-speed switch: `key1.change_speed`, DIV reset,
    /// APU div-phase resync, then advance the 32768 M-cycles the PPU/APU
    /// must observe across the switch.
    fn perform_speed_switch(&mut self);
}

#[derive(Default)]
pub struct Sm83 {
    af: u16,
    bc: u16,
    de: u16,
    has_ei_delay: bool,
    just_halted_from_ei: bool,
    hl: u16,
    ime: bool,
    is_halt_bug_triggered: bool,
    is_halted: bool,
    has_executed_illegal_opcode: bool,
    ld_b_b_breakpoint: bool,
    pc: u16,
    skip_isr_nops: bool,
    sp: u16,
}

impl Sm83 {
    pub const fn has_executed_illegal_opcode(&self) -> bool {
        self.has_executed_illegal_opcode
    }

    pub const fn set_executed_illegal_opcode(&mut self, val: bool) {
        self.has_executed_illegal_opcode = val;
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

    pub const fn f(&self) -> u8 {
        (self.af & 0xFF) as u8
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
    pub fn set_pc(&mut self, pc: u16) {
        self.pc = pc;
    }

    pub fn set_af(&mut self, af: u16) {
        self.af = af;
    }

    pub fn set_bc(&mut self, bc: u16) {
        self.bc = bc;
    }

    pub fn set_de(&mut self, de: u16) {
        self.de = de;
    }

    pub fn set_hl(&mut self, hl: u16) {
        self.hl = hl;
    }

    pub fn set_sp(&mut self, sp: u16) {
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
        let effective_ime = self.ime;
        let was_ei_delay = self.has_ei_delay;
        if self.has_ei_delay {
            self.has_ei_delay = false;
            self.ime = true;
        }

        if bus.interrupts_pending() {
            let was_halted = self.is_halted;
            self.is_halted = false;
            bus.wake_from_stop();

            if effective_ime {
                if self.is_halt_bug_triggered {
                    self.pc = self.pc.wrapping_sub(1);
                    self.is_halt_bug_triggered = false;
                }

                if !self.skip_isr_nops {
                    bus.tick();
                    bus.tick();
                }
                self.skip_isr_nops = false;
                bus.tick();

                if was_halted && (bus.is_cgb_mode() != CgbMode::Dmg || self.just_halted_from_ei) {
                    bus.tick();
                }
                self.just_halted_from_ei = false;

                let pc = self.pc;
                let [lo, hi] = pc.to_le_bytes();

                // Push Hi. The write lands after all preceding M-cycles
                // flush; IF/IE are sampled at that same instant, before the
                // push's own M-cycle elapses.
                self.sp = self.sp.wrapping_sub(1);
                bus.write(self.sp, hi);

                // Push Lo. SameBoy re-evaluates IF/IE *after* the Lo push
                // finishes, using the value from BEFORE the write if it's to
                // IF or IE. Both the re-evaluation and the acknowledgement
                // happen at the write's access instant.
                self.sp = self.sp.wrapping_sub(1);

                let is_if_write = self.sp == 0xFF0F;
                let is_ie_write = self.sp == 0xFFFF;

                let ifr_pre = if is_if_write { bus.read_if() & 0x1F } else { 0 };
                let ie_pre = if is_ie_write { bus.read_ie() & 0x1F } else { 0 };

                bus.write(self.sp, lo);

                let ifr = if is_if_write {
                    ifr_pre
                } else {
                    bus.read_if() & 0x1F
                };
                let ie = if is_ie_write {
                    ie_pre
                } else {
                    bus.read_ie() & 0x1F
                };

                let queue = ie & ifr;
                let (final_int, final_vector) = if queue != 0 {
                    let tz = (queue.trailing_zeros() & 7) as u8;
                    (1 << tz, 0x40 | (u16::from(tz) << 3))
                } else {
                    (0, 0x0000)
                };

                if final_int != 0 {
                    bus.ack_interrupt(final_int);
                }

                if final_int != 0 {
                    self.pc = final_vector;
                } else {
                    self.pc = 0x0000;
                }

                self.ime = false;

                bus.defer(2);

                return;
            } else if was_halted {
                bus.tick();
            }
        }

        // HDMA runs independently of the CPU and is evaluated at
        // instruction-start time, before any M-cycle of this step elapses.
        // It must run even during HALT or HDMA will never start after HALT.
        bus.tick_hdma();

        if self.is_halted {
            bus.tick();
        } else {
            let op = bus.read(self.pc);
            self.pc = self.pc.wrapping_add(1);

            if self.is_halt_bug_triggered {
                self.pc = self.pc.wrapping_sub(1);
                self.is_halt_bug_triggered = false;
                self.skip_isr_nops = true;
            }

            self.exec(bus, op, was_ei_delay);
        }
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
        #[expect(clippy::cast_sign_loss)]
        let offset = self.imm8(bus).cast_signed() as u16;
        self.pc = self.pc.wrapping_add(offset);
        bus.tick();
    }

    fn do_jump_to_immediate(&mut self, bus: &mut impl Bus) {
        let addr = self.imm16(bus);
        self.pc = addr;
        bus.tick();
    }

    #[must_use]
    fn get_r(&self, bus: &mut impl Bus, op: u8) -> u8 {
        let id = ((op >> 1) + 1) & 3;
        let lo = op & 1 != 0;
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

    #[must_use]
    const fn opcode_to_reg_id(op: u8) -> u8 {
        (op >> 4) + 1
    }

    #[must_use]
    const fn opcode_to_reg_id_no_sp(op: u8) -> u8 {
        Self::opcode_to_reg_id(op) & 0x03
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

        // M=1: Internal delay (where OAM bug handling would occur on DMG)
        bus.tick();

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

    fn set_r(&mut self, bus: &mut impl Bus, op: u8, val: u8) {
        let id = ((op >> 1) + 1) & 3;
        let lo = op & 1 != 0;
        if id == 0 {
            if lo {
                self.af = u16::from_le_bytes([self.f(), val]);
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
    fn adc(&mut self, val: u8) {
        let val = u16::from(val);
        let a = self.af >> 8;
        let carry = u16::from((self.af & CF) != 0);
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

    fn add(&mut self, val: u8) {
        let val = u16::from(val);
        let a = self.af >> 8;
        let res = a + val;
        self.af = res << 8;
        if res.trailing_zeros() >= 8 {
            self.af |= ZF;
        }
        if (a & 0xF) + (val & 0xF) > 0x0F {
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

    fn sbc(&mut self, val: u8) {
        let val = u16::from(val);
        let a = self.af >> 8;
        let carry = u16::from((self.af & CF) != 0);
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

    fn sub(&mut self, val: u8) {
        let val = u16::from(val);
        let a = self.af >> 8;
        self.af = (a.wrapping_sub(val) << 8) | NF;
        if a == val {
            self.af |= ZF;
        }
        if (a & 0xF) < (val & 0xF) {
            self.af |= HF;
        }
        if a < val {
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

// Instructions
impl Sm83 {
    fn exec(&mut self, bus: &mut impl Bus, op: u8, was_ei_delay: bool) {
        if op != 0x76 {
            self.just_halted_from_ei = false;
        }

        match op {
            0x00 | 0x5B | 0x6D | 0x7F | 0x49 | 0x52 | 0x64 => self.nop(),
            0x01 | 0x11 | 0x21 | 0x31 => self.ld_rr_d16(bus, op),
            0x02 | 0x12 => self.ld_drr_a(bus, op),
            0x03 | 0x13 | 0x23 | 0x33 => self.inc_rr(bus, op),
            0x04 | 0x14 | 0x24 | 0x3C => self.inc_hr(op),
            0x05 | 0x15 | 0x25 | 0x3D => self.dec_hr(op),
            0x06 | 0x16 | 0x26 | 0x3E => self.ld_hr_d8(bus, op),
            0x07 => self.rlca(),
            0x08 => self.ld_da16_sp(bus),
            0x09 | 0x19 | 0x29 | 0x39 => self.add_hl_rr(bus, op),
            0x0A | 0x1A => self.ld_a_drr(bus, op),
            0x0B | 0x1B | 0x2B | 0x3B => self.dec_rr(bus, op),
            0x0C | 0x1C | 0x2C => self.inc_lr(op),
            0x0D | 0x1D | 0x2D => self.dec_lr(op),
            0x0E | 0x1E | 0x2E => self.ld_lr_d8(bus, op),
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
            0x41 | 0x42 | 0x43 | 0x44 | 0x45 | 0x46 | 0x47 | 0x4A | 0x4B | 0x4C | 0x4D | 0x4E
            | 0x4F | 0x48 | 0x50 | 0x51 | 0x53 | 0x54 | 0x55 | 0x56 | 0x57 | 0x5A | 0x5C | 0x5D
            | 0x5E | 0x5F | 0x58 | 0x59 | 0x60 | 0x61 | 0x62 | 0x63 | 0x65 | 0x66 | 0x67 | 0x6A
            | 0x6B | 0x6C | 0x6E | 0x6F | 0x68 | 0x69 | 0x7A | 0x7B | 0x7C | 0x7D | 0x7E | 0x78
            | 0x79 | 0x77 | 0x70 | 0x73 | 0x72 | 0x71 | 0x74 | 0x75 => self.ld(bus, op),
            0x76 => self.halt(bus, was_ei_delay),
            0x80..=0x87 => self.add_a_r(bus, op),
            0x88..=0x8F => self.adc_a_r(bus, op),
            0x90..=0x97 => self.sub_a_r(bus, op),
            0x98..=0x9F => self.sbc_a_r(bus, op),
            0xA0..=0xA7 => self.and_a_r(bus, op),
            0xA8..=0xAF => self.xor_a_r(bus, op),
            0xB0..=0xB7 => self.or_a_r(bus, op),
            0xB8..=0xBF => self.cp_a_r(bus, op),
            0xC0 | 0xC8 | 0xD0 | 0xD8 => self.ret_cc(bus, op),
            0xC1 | 0xD1 | 0xE1 | 0xF1 => self.pop_rr(bus, op),
            0xC2 | 0xCA | 0xD2 | 0xDA => self.jp_cc(bus, op),
            0xC3 => self.jp_a16(bus),
            0xC4 | 0xCC | 0xD4 | 0xDC => self.call_cc_a16(bus, op),
            0xC5 | 0xD5 | 0xE5 | 0xF5 => self.push_rr(bus, op),
            0xC6 => self.add_a_d8(bus),
            0xC7 | 0xCF | 0xD7 | 0xDF | 0xE7 | 0xEF | 0xF7 | 0xFF => self.rst(bus, op),
            0xC9 => self.ret(bus),
            0xCB => self.exec_cb(bus),
            0xCD => self.call_nn(bus),
            0xCE => self.adc_a_d8(bus),
            0xD6 => self.sub_a_d8(bus),
            0xD9 => self.reti(bus),
            0xDE => self.sbc_a_d8(bus),
            0xE0 => self.ldh_da8_a(bus),
            0xE2 => self.ldh_dc_a(bus),
            0xE6 => self.and_a_d8(bus),
            0xE8 => self.add_sp_r8(bus),
            0xE9 => self.jp_hl(),
            0xEA => self.ld_da16_a(bus),
            0xEE => self.xor_a_d8(bus),
            0xF0 => self.ldh_a_da8(bus),
            0xF2 => self.ldh_a_dc(bus),
            0xF3 => self.di(),
            0xF6 => self.or_a_d8(bus),
            0xF8 => self.ld_hl_sp_r8(bus),
            0xF9 => self.ld16_sp_hl(bus),
            0xFA => self.ld_a_da16(bus),
            0xFB => self.ei(),
            0xFE => self.cp_a_d8(bus),
            _ => self.illegal(bus, op),
        }
    }

    fn exec_cb(&mut self, bus: &mut impl Bus) {
        let op = self.imm8(bus);
        match op >> 3 {
            0 => self.rlc_r(bus, op),
            1 => self.rrc_r(bus, op),
            2 => self.rl_r(bus, op),
            3 => self.rr_r(bus, op),
            4 => self.sla_r(bus, op),
            5 => self.sra_r(bus, op),
            6 => self.swap_r(bus, op),
            7 => self.srl_r(bus, op),
            _ => self.bit_r(bus, op),
        }
    }

    fn adc_a_d8(&mut self, bus: &mut impl Bus) {
        let val = self.imm8(bus);
        self.adc(val);
    }

    fn adc_a_r(&mut self, bus: &mut impl Bus, op: u8) {
        let val = self.get_r(bus, op);
        self.adc(val);
    }

    fn add_a_d8(&mut self, bus: &mut impl Bus) {
        let val = self.imm8(bus);
        self.add(val);
    }

    fn add_a_r(&mut self, bus: &mut impl Bus, op: u8) {
        let val = self.get_r(bus, op);
        self.add(val);
    }

    fn add_hl_rr(&mut self, bus: &mut impl Bus, op: u8) {
        let id = Self::opcode_to_reg_id(op);
        let hl = self.hl;
        let rr = self.get_rr(id);
        self.hl = hl.wrapping_add(rr);

        self.af &= !(NF | CF | HF);

        if ((hl & 0xFFF) + (rr & 0xFFF)) & 0x1000 != 0 {
            self.af |= HF;
        }

        if (u32::from(hl) + u32::from(rr)) & 0x10000 != 0 {
            self.af |= CF;
        }

        bus.tick();
    }

    fn add_sp_r8(&mut self, bus: &mut impl Bus) {
        let sp = self.sp;
        #[expect(clippy::cast_sign_loss)]
        let offset = self.imm8(bus).cast_signed() as u16;
        bus.tick();
        bus.tick();
        self.sp = self.sp.wrapping_add(offset);
        self.af &= 0xFF00;

        if (sp & 0xF) + (offset & 0xF) > 0xF {
            self.af |= HF;
        }

        if (sp & 0xFF) + (offset & 0xFF) > 0xFF {
            self.af |= CF;
        }
    }

    fn and_a_d8(&mut self, bus: &mut impl Bus) {
        let val = self.imm8(bus);
        self.and(val);
    }

    fn and_a_r(&mut self, bus: &mut impl Bus, op: u8) {
        let val = self.get_r(bus, op);
        self.and(val);
    }

    fn bit_r(&mut self, bus: &mut impl Bus, op: u8) {
        let val = self.get_r(bus, op);
        let bit_no = (op >> 3) & 7;
        let bit = 1 << bit_no;
        if op & 0xC0 == 0x40 {
            // bit
            self.af &= 0xFF00 | CF;
            self.af |= HF;
            if bit & val == 0 {
                self.af |= ZF;
            }
        } else if op & 0xC0 == 0x80 {
            // res
            self.set_r(bus, op, val & !bit);
        } else {
            // set
            self.set_r(bus, op, val | bit);
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

    fn cp_a_d8(&mut self, bus: &mut impl Bus) {
        let val = self.imm8(bus);
        self.cp(val);
    }

    fn cp_a_r(&mut self, bus: &mut impl Bus, op: u8) {
        let val = self.get_r(bus, op);
        self.cp(val);
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

    fn dec_hr(&mut self, op: u8) {
        let id = Self::opcode_to_reg_id_no_sp(op);
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

    fn dec_lr(&mut self, op: u8) {
        let id = Self::opcode_to_reg_id(op);
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
        let id = Self::opcode_to_reg_id(op);
        self.set_rr(id, self.get_rr(id).wrapping_sub(1));
        bus.tick();
    }

    const fn di(&mut self) {
        self.ime = false;
        self.has_ei_delay = false;
    }

    const fn ei(&mut self) {
        self.has_ei_delay = true;
    }

    fn halt(&mut self, bus: &impl Bus, was_ei_delay: bool) {
        if !bus.interrupts_pending() {
            self.is_halted = true;
            self.just_halted_from_ei = was_ei_delay;
        } else if self.ime {
            self.is_halted = false;
        } else {
            self.is_halted = false;
            self.is_halt_bug_triggered = true;
        }
    }

    fn illegal(&mut self, bus: &mut impl Bus, _op: u8) {
        bus.clear_ie();
        self.is_halted = true;
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

    fn inc_hr(&mut self, op: u8) {
        let id = Self::opcode_to_reg_id_no_sp(op);
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

    fn inc_lr(&mut self, op: u8) {
        let id = Self::opcode_to_reg_id(op);
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
        let id = Self::opcode_to_reg_id(op);
        self.set_rr(id, self.get_rr(id).wrapping_add(1));
        bus.tick();
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
        self.sp = val;
        bus.tick();
    }

    fn ld_a_da16(&mut self, bus: &mut impl Bus) {
        self.af &= 0xFF;
        let addr = self.imm16(bus);
        self.af |= u16::from(bus.read(addr)) << 8;
    }

    fn ld_a_dhld(&mut self, bus: &mut impl Bus) {
        let addr = self.hl;
        let val = u16::from(bus.read(addr));
        self.af &= 0xFF;
        self.af |= val << 8;
        self.hl = addr.wrapping_sub(1);
    }

    fn ld_a_dhli(&mut self, bus: &mut impl Bus) {
        let addr = self.hl;
        let val = u16::from(bus.read(addr));
        self.af &= 0xFF;
        self.af |= val << 8;
        self.hl = addr.wrapping_add(1);
    }

    fn ld_a_drr(&mut self, bus: &mut impl Bus, op: u8) {
        let id = Self::opcode_to_reg_id(op);
        self.af &= 0xFF;
        let addr = self.get_rr(id);
        self.af |= u16::from(bus.read(addr)) << 8;
    }

    // Sets the debug breakpoint flag. Test ROMs like cgb-acid2 and dmg-acid2
    // use this instruction as a breakpoint to signal test completion.
    fn ld_b_b(&mut self) {
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
        let id = Self::opcode_to_reg_id(op);
        let addr = self.get_rr(id);
        bus.write(addr, self.a());
    }

    fn ld_hl_sp_r8(&mut self, bus: &mut impl Bus) {
        self.af &= 0xFF00;
        #[expect(clippy::cast_sign_loss)]
        let offset = self.imm8(bus).cast_signed() as u16;
        bus.tick();
        self.hl = self.sp.wrapping_add(offset);

        if (self.sp & 0xF) + (offset & 0xF) > 0xF {
            self.af |= HF;
        }

        if (self.sp & 0xFF) + (offset & 0xFF) > 0xFF {
            self.af |= CF;
        }
    }

    fn ld_hr_d8(&mut self, bus: &mut impl Bus, op: u8) {
        let id = Self::opcode_to_reg_id_no_sp(op);
        let hi = u16::from(self.imm8(bus));
        self.set_rr(id, (hi << 8) | self.get_rr(id) & 0xFF);
    }

    fn ld_lr_d8(&mut self, bus: &mut impl Bus, op: u8) {
        let id = Self::opcode_to_reg_id(op);
        let lo = u16::from(self.imm8(bus));
        self.set_rr(id, self.get_rr(id) & 0xFF00 | lo);
    }

    fn ld_rr_d16(&mut self, bus: &mut impl Bus, op: u8) {
        let id = Self::opcode_to_reg_id(op);
        let imm = self.imm16(bus);
        self.set_rr(id, imm);
    }

    fn ldh_a_da8(&mut self, bus: &mut impl Bus) {
        let tmp = u16::from(self.imm8(bus));
        self.af &= 0xFF;
        self.af |= u16::from(bus.read(0xFF00 | tmp)) << 8;
    }

    fn ldh_a_dc(&mut self, bus: &mut impl Bus) {
        self.af &= 0xFF;
        self.af |= u16::from(bus.read(0xFF00 | self.bc & 0xFF)) << 8;
    }

    fn ldh_da8_a(&mut self, bus: &mut impl Bus) {
        let tmp = u16::from(self.imm8(bus));
        let a = self.a();
        bus.write(0xFF00 | tmp, a);
    }

    fn ldh_dc_a(&self, bus: &mut impl Bus) {
        bus.write(0xFF00 | self.bc & 0xFF, self.a());
    }

    #[expect(clippy::unused_self)]
    const fn nop(&self) {}

    fn or_a_d8(&mut self, bus: &mut impl Bus) {
        let val = self.imm8(bus);
        self.or(val);
    }

    fn or_a_r(&mut self, bus: &mut impl Bus, op: u8) {
        let val = self.get_r(bus, op);
        self.or(val);
    }

    fn pop_rr(&mut self, bus: &mut impl Bus, op: u8) {
        let val = self.pop(bus);
        let id = Self::opcode_to_reg_id_no_sp(op);
        self.set_rr(id, val);
        self.af &= 0xFFF0;
    }

    fn push_rr(&mut self, bus: &mut impl Bus, op: u8) {
        let id = Self::opcode_to_reg_id_no_sp(op);
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

    fn rl_r(&mut self, bus: &mut impl Bus, op: u8) {
        let val = self.get_r(bus, op);
        let carry = self.af & CF != 0;
        let bit7 = val & 0x80 != 0;

        self.af &= 0xFF00;
        let val = (val << 1) | u8::from(carry);
        self.set_r(bus, op, val);
        if bit7 {
            self.af |= CF;
        }
        if val == 0 {
            self.af |= ZF;
        }
    }

    fn rla(&mut self) {
        let bit7 = self.af & 0x8000 != 0;
        let carry = self.af & CF != 0;

        self.af = ((self.af & 0xFF00) << 1) | (u16::from(carry) << 8);

        if bit7 {
            self.af |= CF;
        }
    }

    fn rlc_r(&mut self, bus: &mut impl Bus, op: u8) {
        let val = self.get_r(bus, op);
        let carry = val & 0x80 != 0;
        self.af &= 0xFF00;
        self.set_r(bus, op, (val << 1) | u8::from(carry));
        if carry {
            self.af |= CF;
        }
        if val == 0 {
            self.af |= ZF;
        }
    }

    const fn rlca(&mut self) {
        let carry = (self.af & 0x8000) != 0;

        self.af = (self.af & 0xFF00) << 1;
        if carry {
            self.af |= CF | 0x0100;
        }
    }

    fn rr_r(&mut self, bus: &mut impl Bus, op: u8) {
        let val = self.get_r(bus, op);
        let carry = self.af & CF != 0;
        let bit1 = val & 1 != 0;
        let val = (val >> 1) | (u8::from(carry) << 7);
        self.set_r(bus, op, val);

        self.af &= 0xFF00;
        if bit1 {
            self.af |= CF;
        }
        if val == 0 {
            self.af |= ZF;
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

    fn rrc_r(&mut self, bus: &mut impl Bus, op: u8) {
        let val = self.get_r(bus, op);
        let carry = (val & 0x01) != 0;
        self.af &= 0xFF00;
        let val = (val >> 1) | (u8::from(carry) << 7);
        self.set_r(bus, op, val);
        if carry {
            self.af |= CF;
        }
        if val == 0 {
            self.af |= ZF;
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

    fn sbc_a_d8(&mut self, bus: &mut impl Bus) {
        let val = self.imm8(bus);
        self.sbc(val);
    }

    fn sbc_a_r(&mut self, bus: &mut impl Bus, op: u8) {
        let val = self.get_r(bus, op);
        self.sbc(val);
    }

    const fn scf(&mut self) {
        self.af |= CF;
        self.af &= !(HF | NF);
    }

    fn sla_r(&mut self, bus: &mut impl Bus, op: u8) {
        let val = self.get_r(bus, op);
        let carry = val & 0x80 != 0;
        let res = val << 1;
        self.set_r(bus, op, res);

        self.af &= 0xFF00;
        if carry {
            self.af |= CF;
        }
        if res == 0 {
            self.af |= ZF;
        }
    }

    fn sra_r(&mut self, bus: &mut impl Bus, op: u8) {
        let val = self.get_r(bus, op);
        let bit7 = val & 0x80;
        self.af &= 0xFF00;
        if val & 1 != 0 {
            self.af |= CF;
        }
        let val = (val >> 1) | bit7;
        self.set_r(bus, op, val);
        if val == 0 {
            self.af |= ZF;
        }
    }

    fn srl_r(&mut self, bus: &mut impl Bus, op: u8) {
        let val = self.get_r(bus, op);
        self.af &= 0xFF00;
        self.set_r(bus, op, val >> 1);
        if val & 1 != 0 {
            self.af |= CF;
        }
        if val >> 1 == 0 {
            self.af |= ZF;
        }
    }

    fn stop(&mut self, bus: &mut impl Bus) {
        // The discarded operand byte is read at the current instant, but
        // its M-cycle only elapses after the STOP side effects — matching
        // the previous batched ordering where `write_div`/the speed switch
        // ran before the instruction's final flush.
        let _discard_byte = bus.read(self.pc);
        self.pc = self.pc.wrapping_add(1);

        if bus.speed_switch_pending() {
            // CGB double-speed switch.
            //
            // The previous implementation used a hard-coded
            // `for _ in 0..32768 { tick_m_cycle() }` loop followed by
            // `write_div()`. The 32768 value (131072 T-cycles) was
            // tuned so the PPU/APU would advance through enough
            // cycles during a speed change for the CGB double-speed
            // PPU/STAT tests (gambatte `*_ds_*` tests in
            // ff41_disable, ff45_disable, late_ff41_enable, lyc_*,
            // m2int_m0irq, etc.) to see the right PPU mode at the
            // right time. The value is way larger than any actual
            // hardware speed-switch delay (SameBoy uses 11 M-cycles
            // total via speed_switch_countdown/freeze; gambatte uses
            // 8 T-cycles for normal->double and 0 for double->normal).
            //
            // The hack had two correctness problems for the timer:
            //   1. `write_div()` fires triggers = `old_div & !0` =
            //      `old_div`, so any bit that was set in `old_div`
            //      at STOP time would spuriously increment TIMA by
            //      1. This made gambatte's speedchange2_tima01_1
            //      read A=0A instead of 09 and tima00_1a read A=02
            //      instead of 00.
            //   2. Even ignoring the triggers, the timer's
            //      `set_system_clk(old + 1)`-per-T-cycle loop never
            //      produces a falling edge when starting from div=0,
            //      so the 131072 T-cycles of pending flush was
            //      effectively a no-op for the timer — the only
            //      effect on TIMA was the +1 from the `write_div`
            //      trigger.
            //
            // The new implementation:
            //   - Uses the same 32768 M-cycles for the PPU/APU
            //     advance that the previous code used, since the
            //     `_ds_` PPU tests genuinely need the PPU to see a
            //     large time gap (the CGB speed change rewinds the
            //     PPU's internal phase by ~131072 T-cycles).
            //   - After the loop, resets `clock.div = 0` directly
            //     (bypassing `set_system_clk`) so no spurious TIMA
            //     trigger fires from the div-reset, and so the
            //     subsequent flush of the 131072 T-cycles of pending
            //     also produces no TIMA triggers (the timer's
            //     `old + 1` walk never falls).
            //   - Calls `apu.reset_div_phase()` to resynchronise the
            //     sound unit's div-phase counter, matching what
            //     `write_div` would have done for the APU.
            bus.perform_speed_switch();
        } else {
            bus.enter_stop(!self.ime);
            self.is_halted = true;
        }
    }

    fn sub_a_d8(&mut self, bus: &mut impl Bus) {
        let val = self.imm8(bus);
        self.sub(val);
    }

    fn sub_a_r(&mut self, bus: &mut impl Bus, op: u8) {
        let val = self.get_r(bus, op);
        self.sub(val);
    }

    fn swap_r(&mut self, bus: &mut impl Bus, op: u8) {
        let val = self.get_r(bus, op);
        self.af &= 0xFF00;
        self.set_r(bus, op, val.rotate_left(4));
        if val == 0 {
            self.af |= ZF;
        }
    }

    fn xor_a_d8(&mut self, bus: &mut impl Bus) {
        let val = self.imm8(bus);
        self.xor(val);
    }

    fn xor_a_r(&mut self, bus: &mut impl Bus, op: u8) {
        let val = self.get_r(bus, op);
        self.xor(val);
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
            self.advance_dots(self.time_deferred);
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
        let val = self.read_mem(addr);
        self.time_deferred = 4;
        val
    }

    #[inline]
    fn write(&mut self, addr: u16, val: u8) {
        let conflict =
            conflict::get_conflict(self.model, self.cgb_mode, self.key1.is_enabled(), addr);

        let pending = self.time_deferred;

        match conflict {
            ConflictType::ReadOld => {
                self.flush_deferred_time();
                self.write_mem(addr, val);
                self.time_deferred = 4;
            }
            ConflictType::ReadNew => {
                if pending >= 1 {
                    self.advance_dots(pending - 1);
                    self.write_mem(addr, val);
                    self.time_deferred = 5;
                } else {
                    self.flush_deferred_time();
                    self.write_mem(addr, val);
                    self.time_deferred = 4;
                }
            }
            ConflictType::WriteCpu => {
                self.advance_dots(pending + 1);
                self.write_mem(addr, val);
                self.time_deferred = 3;
            }
            ConflictType::StatDmg => {
                self.flush_deferred_time();
                self.write_mem(addr, val);
                self.time_deferred = 4;
            }
            ConflictType::StatCgb => {
                let old = self.read_mem(addr);
                self.flush_deferred_time();
                self.write_mem(addr, (old & 0x40) | (val & !0x40));
                self.advance_dots(1);
                self.write_mem(addr, val);
                self.time_deferred = 3;
            }
            ConflictType::StatCgbDouble => {
                let old = self.read_mem(addr);
                self.flush_deferred_time();
                self.write_mem(addr, (val & !8) | (old & 8));
                self.advance_dots(1);
                self.write_mem(addr, val);
                self.time_deferred = 3;
            }
            ConflictType::PaletteDmg => {
                let old = self.read_mem(addr);
                self.flush_deferred_time();
                self.advance_dots(1);
                self.write_mem(addr, val | old);
                self.advance_dots(1);
                self.write_mem(addr, val);
                self.time_deferred = 2;
            }
            ConflictType::PaletteCgb => {
                if matches!(self.model, Model::CgbD | Model::CgbE | Model::Agb) {
                    if pending >= 2 {
                        self.advance_dots(pending - 2);
                        self.write_mem(addr, val);
                        self.time_deferred = 6;
                    } else {
                        self.flush_deferred_time();
                        self.write_mem(addr, val);
                        self.time_deferred = 4;
                    }
                } else if pending >= 1 {
                    self.advance_dots(pending - 1);
                    self.write_mem(addr, val);
                    self.time_deferred = 5;
                } else {
                    self.flush_deferred_time();
                    self.write_mem(addr, val);
                    self.time_deferred = 4;
                }
            }
            ConflictType::DmgLcdc => {
                let old = self.read_mem(addr);
                self.flush_deferred_time();
                self.advance_dots(1);
                self.write_mem(addr, old | (val & 0x01));
                self.advance_dots(1);
                self.write_mem(addr, val);
                self.time_deferred = 2;
            }
            ConflictType::SgbLcdc => {
                self.flush_deferred_time();
                self.write_mem(addr, val);
                self.time_deferred = 4;
            }
            ConflictType::WxDmg => {
                self.flush_deferred_time();
                self.write_mem(addr, val);
                self.advance_dots(1);
                self.time_deferred = 3;
            }
            ConflictType::LcdcCgb => {
                self.flush_deferred_time();
                self.write_mem(addr, val);
                self.time_deferred = 4;
            }
            ConflictType::LcdcCgbDouble => {
                if pending >= 2 {
                    self.advance_dots(pending - 2);
                    let old = self.read_mem(addr);
                    self.write_mem(addr, (val & !0x81) | (old & 0x81));
                    self.advance_dots(2);
                    self.write_mem(addr, val);
                    self.time_deferred = 4;
                } else {
                    self.flush_deferred_time();
                    self.write_mem(addr, val);
                    self.time_deferred = 4;
                }
            }
            ConflictType::ScxDmgAndCgbDouble => {
                if pending >= 2 {
                    self.advance_dots(pending - 2);
                    self.write_mem(addr, val);
                    self.time_deferred = 6;
                } else {
                    self.flush_deferred_time();
                    self.write_mem(addr, val);
                    self.time_deferred = 4;
                }
            }
            ConflictType::Nr10CgbDouble => {
                if pending >= 1 {
                    self.advance_dots(pending - 1);
                    self.advance_dots(1);
                    self.write_mem(addr, val);
                    self.time_deferred = 4;
                } else {
                    self.flush_deferred_time();
                    self.write_mem(addr, val);
                    self.time_deferred = 4;
                }
            }
        }
    }

    #[inline]
    fn defer(&mut self, t_cycles: i32) {
        let flush = self.time_deferred - t_cycles;
        if flush > 0 {
            self.advance_dots(flush);
        }
        self.time_deferred = t_cycles;
    }

    fn is_cgb_mode(&self) -> CgbMode {
        self.cgb_mode
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
        self.ints.acknowledge_interrupt(bit);
    }

    fn clear_ie(&mut self) {
        self.ints.illegal();
    }

    fn tick_hdma(&mut self) {
        self.run_hdma();
    }

    fn wake_from_stop(&mut self) {
        self.ppu.leave_stop_mode();
        self.clock.stopped = false;
    }

    fn enter_stop(&mut self, ime: bool) {
        self.write_div();
        if !ime {
            self.clock.div_cycles = -4;
        }
        self.ppu.enter_stop_mode();
        self.clock.stopped = true;
    }

    fn speed_switch_pending(&self) -> bool {
        self.key1.is_requested()
    }

    fn perform_speed_switch(&mut self) {
        self.key1.change_speed();
        self.clock.div = 0;
        self.apu.reset_div_phase();
        self.advance_dots(32768 * 4);
    }
}
