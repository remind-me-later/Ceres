use crate::{AudioCallback, Gb};

const ZF: u16 = 0x80;
const NF: u16 = 0x40;
const HF: u16 = 0x20;
const CF: u16 = 0x10;

#[derive(Default)]
pub struct Sm83 {
    af: u16,
    bc: u16,
    de: u16,
    has_ei_delay: bool,
    hl: u16,
    is_halt_bug_triggered: bool,
    is_halted: bool,
    pc: u16,
    sp: u16,
}

impl Sm83 {
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

    pub const fn pc(&self) -> u16 {
        self.pc
    }

    pub const fn sp(&self) -> u16 {
        self.sp
    }
}

impl<A: AudioCallback> Gb<A> {
    fn exec(&mut self, op: u8) {
        match op {
            0x00 | 0x5B | 0x6D | 0x7F | 0x49 | 0x52 | 0x64 => self.nop(),
            0x01 | 0x11 | 0x21 | 0x31 => self.ld_rr_d16(op),
            0x02 | 0x12 => self.ld_drr_a(op),
            0x03 | 0x13 | 0x23 | 0x33 => self.inc_rr(op),
            0x04 | 0x14 | 0x24 | 0x3C => self.inc_hr(op),
            0x05 | 0x15 | 0x25 | 0x3D => self.dec_hr(op),
            0x06 | 0x16 | 0x26 | 0x3E => self.ld_hr_d8(op),
            0x07 => self.rlca(),
            0x08 => self.ld_da16_sp(),
            0x09 | 0x19 | 0x29 | 0x39 => self.add_hl_rr(op),
            0x0A | 0x1A => self.ld_a_drr(op),
            0x0B | 0x1B | 0x2B | 0x3B => self.dec_rr(op),
            0x0C | 0x1C | 0x2C => self.inc_lr(op),
            0x0D | 0x1D | 0x2D => self.dec_lr(op),
            0x0E | 0x1E | 0x2E => self.ld_lr_d8(op),
            0x0F => self.rrca(),
            0x10 => self.stop(),
            0x17 => self.rla(),
            0x18 => self.jr_d(),
            0x1F => self.rra(),
            0x20 | 0x28 | 0x30 | 0x38 => self.jr_cc(op),
            0x22 => self.ld_dhli_a(),
            0x27 => self.daa(),
            0x2A => self.ld_a_dhli(),
            0x2F => self.cpl(),
            0x32 => self.ld_dhld_a(),
            0x34 => self.inc_dhl(),
            0x35 => self.dec_dhl(),
            0x36 => self.ld_dhl_d8(),
            0x37 => self.scf(),
            0x3A => self.ld_a_dhld(),
            0x3F => self.ccf(),
            0x40 => self.ld_b_b(),
            0x41 | 0x42 | 0x43 | 0x44 | 0x45 | 0x46 | 0x47 | 0x4A | 0x4B | 0x4C | 0x4D | 0x4E
            | 0x4F | 0x48 | 0x50 | 0x51 | 0x53 | 0x54 | 0x55 | 0x56 | 0x57 | 0x5A | 0x5C | 0x5D
            | 0x5E | 0x5F | 0x58 | 0x59 | 0x60 | 0x61 | 0x62 | 0x63 | 0x65 | 0x66 | 0x67 | 0x6A
            | 0x6B | 0x6C | 0x6E | 0x6F | 0x68 | 0x69 | 0x7A | 0x7B | 0x7C | 0x7D | 0x7E | 0x78
            | 0x79 | 0x77 | 0x70 | 0x73 | 0x72 | 0x71 | 0x74 | 0x75 => self.ld(op),
            0x76 => self.halt(),
            0x80..=0x87 => self.add_a_r(op),
            0x88..=0x8F => self.adc_a_r(op),
            0x90..=0x97 => self.sub_a_r(op),
            0x98..=0x9F => self.sbc_a_r(op),
            0xA0..=0xA7 => self.and_a_r(op),
            0xA8..=0xAF => self.xor_a_r(op),
            0xB0..=0xB7 => self.or_a_r(op),
            0xB8..=0xBF => self.cp_a_r(op),
            0xC0 | 0xC8 | 0xD0 | 0xD8 => self.ret_cc(op),
            0xC1 | 0xD1 | 0xE1 | 0xF1 => self.pop_rr(op),
            0xC2 | 0xCA | 0xD2 | 0xDA => self.jp_cc(op),
            0xC3 => self.jp_a16(),
            0xC4 | 0xCC | 0xD4 | 0xDC => self.call_cc_a16(op),
            0xC5 | 0xD5 | 0xE5 | 0xF5 => self.push_rr(op),
            0xC6 => self.add_a_d8(),
            0xC7 | 0xCF | 0xD7 | 0xDF | 0xE7 | 0xEF | 0xF7 | 0xFF => self.rst(op),
            0xC9 => self.ret(),
            0xCB => self.exec_cb(),
            0xCD => self.call_nn(),
            0xCE => self.adc_a_d8(),
            0xD6 => self.sub_a_d8(),
            0xD9 => self.reti(),
            0xDE => self.sbc_a_d8(),
            0xE0 => self.ldh_da8_a(),
            0xE2 => self.ldh_dc_a(),
            0xE6 => self.and_a_d8(),
            0xE8 => self.add_sp_r8(),
            0xE9 => self.jp_hl(),
            0xEA => self.ld_da16_a(),
            0xEE => self.xor_a_d8(),
            0xF0 => self.ldh_a_da8(),
            0xF2 => self.ldh_a_dc(),
            0xF3 => self.di(),
            0xF6 => self.or_a_d8(),
            0xF8 => self.ld_hl_sp_r8(),
            0xF9 => self.ld16_sp_hl(),
            0xFA => self.ld_a_da16(),
            0xFB => self.ei(),
            0xFE => self.cp_a_d8(),
            _ => self.illegal(op),
        }
    }

    fn exec_cb(&mut self) {
        let op = self.imm8();
        match op >> 3 {
            0 => self.rlc_r(op),
            1 => self.rrc_r(op),
            2 => self.rl_r(op),
            3 => self.rr_r(op),
            4 => self.sla_r(op),
            5 => self.sra_r(op),
            6 => self.swap_r(op),
            7 => self.srl_r(op),
            _ => self.bit_r(op),
        }
    }

    #[inline]
    pub fn run_cpu(&mut self) {
        if self.cpu.has_ei_delay {
            self.ints.enable();
            self.cpu.has_ei_delay = false;
        }

        if self.cpu.is_halted {
            self.tick_m_cycle();
        } else {
            let op = self.imm8();
            self.run_hdma();

            if self.cpu.is_halt_bug_triggered {
                self.cpu.pc = self.cpu.pc.wrapping_sub(1);
                self.cpu.is_halt_bug_triggered = false;
            }

            self.exec(op);
        }

        if self.ints.is_any_requested() {
            self.cpu.is_halted = false;
            self.ppu.leave_stop_mode();

            if self.ints.are_enabled() {
                self.tick_m_cycle();
                self.tick_m_cycle();

                // Perform interrupt push with IE re-check during upper byte write
                // to handle the edge case where IE is modified mid-dispatch.
                //
                // Hardware behavior (verified by Mooneye ie_push test):
                // - If SP=$0000, upper byte push writes to $FFFF (IE register)
                // - If the write clears the interrupt bit, dispatch is cancelled (PC=$0000)
                // - If SP=$0001, lower byte push writes to IE, but it's too late to cancel
                let pc = self.cpu.pc;
                let [lo, hi] = pc.to_le_bytes();

                // Push upper byte
                self.cpu.sp = self.cpu.sp.wrapping_sub(1);
                self.write_cpu(self.cpu.sp, hi);

                // Re-check interrupt queue after upper byte push
                // IE may have been modified by the write to $FFFF
                let (new_int, new_vector) = self.ints.determine_interrupt();

                // Push lower byte
                self.cpu.sp = self.cpu.sp.wrapping_sub(1);
                self.write_cpu(self.cpu.sp, lo);
                self.tick_m_cycle();

                // Acknowledge the interrupt only if it's still pending
                // If IE was modified to clear the original interrupt, don't clear IF
                if new_int != 0 {
                    self.ints.acknowledge_interrupt(new_int);
                }

                self.ints.disable();
                self.cpu.pc = new_vector;
            }
        }
    }
}

// Internal
impl<A: AudioCallback> Gb<A> {
    fn do_call(&mut self) {
        let addr = self.imm16();
        self.push(self.cpu.pc);
        self.cpu.pc = addr;
    }

    fn do_jump_relative(&mut self) {
        #[expect(clippy::cast_sign_loss)]
        let offset = self.imm8().cast_signed() as u16;
        self.cpu.pc = self.cpu.pc.wrapping_add(offset);
        self.tick_m_cycle();
    }

    fn do_jump_to_immediate(&mut self) {
        let addr = self.imm16();
        self.cpu.pc = addr;
        self.tick_m_cycle();
    }

    #[must_use]
    fn get_r(&mut self, op: u8) -> u8 {
        let id = ((op >> 1) + 1) & 3;
        let lo = op & 1 != 0;
        if id == 0 {
            if lo {
                self.cpu.a()
            } else {
                self.read_cpu(self.cpu.hl)
            }
        } else if lo {
            (self.get_rr(id) & 0xFF) as u8
        } else {
            (self.get_rr(id) >> 8) as u8
        }
    }

    #[must_use]
    const fn get_rr(&self, id: u8) -> u16 {
        match id {
            0 => self.cpu.af,
            1 => self.cpu.bc,
            2 => self.cpu.de,
            3 => self.cpu.hl,
            4 => self.cpu.sp,
            _ => unreachable!(),
        }
    }

    #[must_use]
    fn imm16(&mut self) -> u16 {
        let lo = self.imm8();
        let hi = self.imm8();
        u16::from_le_bytes([lo, hi])
    }

    #[must_use]
    fn imm8(&mut self) -> u8 {
        let val = self.read_cpu(self.cpu.pc);
        self.cpu.pc = self.cpu.pc.wrapping_add(1);
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
    fn pop(&mut self) -> u16 {
        let lo = self.read_cpu(self.cpu.sp);
        self.cpu.sp = self.cpu.sp.wrapping_add(1);
        let hi = self.read_cpu(self.cpu.sp);
        self.cpu.sp = self.cpu.sp.wrapping_add(1);
        u16::from_le_bytes([lo, hi])
    }

    /// PUSH rr instruction timing (verified by Mooneye `push_timing` test):
    /// M=0: Instruction decode (implicit)
    /// M=1: Internal delay
    /// M=2: Memory write for high byte
    /// M=3: Memory write for low byte
    fn push(&mut self, val: u16) {
        let [lo, hi] = val.to_le_bytes();

        // M=1: Internal delay (where OAM bug handling would occur on DMG)
        self.tick_m_cycle();

        // M=2: Write high byte
        self.cpu.sp = self.cpu.sp.wrapping_sub(1);
        self.tick_m_cycle();
        self.write_mem(self.cpu.sp, hi);

        // M=3: Write low byte
        self.cpu.sp = self.cpu.sp.wrapping_sub(1);
        self.tick_m_cycle();
        self.write_mem(self.cpu.sp, lo);
    }

    #[must_use]
    fn read_cpu(&mut self, addr: u16) -> u8 {
        self.tick_m_cycle();
        self.read_mem(addr)
    }

    #[must_use]
    const fn satisfies_branch_condition(&self, op: u8) -> bool {
        match (op >> 3) & 3 {
            0 => self.cpu.af & ZF == 0,
            1 => self.cpu.af & ZF != 0,
            2 => self.cpu.af & CF == 0,
            _ => self.cpu.af & CF != 0,
        }
    }

    fn set_r(&mut self, op: u8, val: u8) {
        let id = ((op >> 1) + 1) & 3;
        let lo = op & 1 != 0;
        if id == 0 {
            if lo {
                self.cpu.af = u16::from_le_bytes([self.cpu.f(), val]);
            } else {
                self.write_cpu(self.cpu.hl, val);
            }
        } else if lo {
            self.set_rr(id, u16::from(val) | self.get_rr(id) & 0xFF00);
        } else {
            self.set_rr(id, (u16::from(val) << 8) | self.get_rr(id) & 0xFF);
        }
    }

    fn set_rr(&mut self, id: u8, val: u16) {
        match id {
            0 => self.cpu.af = val,
            1 => self.cpu.bc = val,
            2 => self.cpu.de = val,
            3 => self.cpu.hl = val,
            4 => self.cpu.sp = val,
            _ => unreachable!(),
        }
    }

    fn tick_m_cycle(&mut self) {
        self.advance_dots(4);
    }

    fn write_cpu(&mut self, addr: u16, val: u8) {
        // Capture timestamp before advancing time for DMA start logging
        if addr >= 0xFF00 {
            let io_addr = (addr & 0xFF) as u8;
            if io_addr == 0x46 {
                // DMA register
                self.dma_write_start_dots = self.total_dots;
            }
        }
        self.tick_m_cycle();
        self.write_mem(addr, val);
    }
}

// ALU
impl<A: AudioCallback> Gb<A> {
    fn adc(&mut self, val: u8) {
        let val = u16::from(val);
        let a = self.cpu.af >> 8;
        let carry = u16::from((self.cpu.af & CF) != 0);
        let res = a + val + carry;
        self.cpu.af = res << 8;
        if res.trailing_zeros() >= 8 {
            self.cpu.af |= ZF;
        }
        if (a & 0xF) + (val & 0xF) + carry > 0x0F {
            self.cpu.af |= HF;
        }
        if res > 0xFF {
            self.cpu.af |= CF;
        }
    }

    fn add(&mut self, val: u8) {
        let val = u16::from(val);
        let a = self.cpu.af >> 8;
        let res = a + val;
        self.cpu.af = res << 8;
        if res.trailing_zeros() >= 8 {
            self.cpu.af |= ZF;
        }
        if (a & 0xF) + (val & 0xF) > 0x0F {
            self.cpu.af |= HF;
        }
        if res > 0xFF {
            self.cpu.af |= CF;
        }
    }

    fn and(&mut self, val: u8) {
        let val = u16::from(val);
        let a = self.cpu.af >> 8;
        let a = a & val;
        self.cpu.af = (a << 8) | HF;
        if a == 0 {
            self.cpu.af |= ZF;
        }
    }

    const fn cp(&mut self, val: u8) {
        let a = self.cpu.a();
        self.cpu.af &= 0xFF00;
        self.cpu.af |= NF;
        if a == val {
            self.cpu.af |= ZF;
        }
        if a & 0xF < val & 0xF {
            self.cpu.af |= HF;
        }
        if a < val {
            self.cpu.af |= CF;
        }
    }

    fn or(&mut self, val: u8) {
        let val = u16::from(val);
        let a = self.cpu.af >> 8;
        self.cpu.af = (a | val) << 8;
        if a | val == 0 {
            self.cpu.af |= ZF;
        }
    }

    fn sbc(&mut self, val: u8) {
        let val = u16::from(val);
        let a = self.cpu.af >> 8;
        let carry = u16::from((self.cpu.af & CF) != 0);
        let res = a.wrapping_sub(val).wrapping_sub(carry);
        self.cpu.af = (res << 8) | NF;

        if res.trailing_zeros() >= 8 {
            self.cpu.af |= ZF;
        }
        if (a & 0xF) < (val & 0xF) + carry {
            self.cpu.af |= HF;
        }
        if res > 0xFF {
            self.cpu.af |= CF;
        }
    }

    fn sub(&mut self, val: u8) {
        let val = u16::from(val);
        let a = self.cpu.af >> 8;
        self.cpu.af = (a.wrapping_sub(val) << 8) | NF;
        if a == val {
            self.cpu.af |= ZF;
        }
        if (a & 0xF) < (val & 0xF) {
            self.cpu.af |= HF;
        }
        if a < val {
            self.cpu.af |= CF;
        }
    }

    fn xor(&mut self, val: u8) {
        let val = u16::from(val);
        let a = self.cpu.af >> 8;
        let a = a ^ val;
        self.cpu.af = a << 8;
        if a == 0 {
            self.cpu.af |= ZF;
        }
    }
}

// Instructions
impl<A: AudioCallback> Gb<A> {
    fn adc_a_d8(&mut self) {
        let val = self.imm8();
        self.adc(val);
    }

    fn adc_a_r(&mut self, op: u8) {
        let val = self.get_r(op);
        self.adc(val);
    }

    fn add_a_d8(&mut self) {
        let val = self.imm8();
        self.add(val);
    }

    fn add_a_r(&mut self, op: u8) {
        let val = self.get_r(op);
        self.add(val);
    }

    fn add_hl_rr(&mut self, op: u8) {
        let id = Self::opcode_to_reg_id(op);
        let hl = self.cpu.hl;
        let rr = self.get_rr(id);
        self.cpu.hl = hl.wrapping_add(rr);

        self.cpu.af &= !(NF | CF | HF);

        if ((hl & 0xFFF) + (rr & 0xFFF)) & 0x1000 != 0 {
            self.cpu.af |= HF;
        }

        if (u32::from(hl) + u32::from(rr)) & 0x10000 != 0 {
            self.cpu.af |= CF;
        }

        self.tick_m_cycle();
    }

    fn add_sp_r8(&mut self) {
        let sp = self.cpu.sp;
        #[expect(clippy::cast_sign_loss)]
        let offset = self.imm8().cast_signed() as u16;
        self.tick_m_cycle();
        self.tick_m_cycle();
        self.cpu.sp = self.cpu.sp.wrapping_add(offset);
        self.cpu.af &= 0xFF00;

        if (sp & 0xF) + (offset & 0xF) > 0xF {
            self.cpu.af |= HF;
        }

        if (sp & 0xFF) + (offset & 0xFF) > 0xFF {
            self.cpu.af |= CF;
        }
    }

    fn and_a_d8(&mut self) {
        let val = self.imm8();
        self.and(val);
    }

    fn and_a_r(&mut self, op: u8) {
        let val = self.get_r(op);
        self.and(val);
    }

    fn bit_r(&mut self, op: u8) {
        let val = self.get_r(op);
        let bit_no = (op >> 3) & 7;
        let bit = 1 << bit_no;
        if op & 0xC0 == 0x40 {
            // bit
            self.cpu.af &= 0xFF00 | CF;
            self.cpu.af |= HF;
            if bit & val == 0 {
                self.cpu.af |= ZF;
            }
        } else if op & 0xC0 == 0x80 {
            // res
            self.set_r(op, val & !bit);
        } else {
            // set
            self.set_r(op, val | bit);
        }
    }

    fn call_cc_a16(&mut self, op: u8) {
        if self.satisfies_branch_condition(op) {
            self.do_call();
        } else {
            let pc = self.cpu.pc.wrapping_add(2);
            self.cpu.pc = pc;
            self.tick_m_cycle();
            self.tick_m_cycle();
        }
    }

    fn call_nn(&mut self) {
        self.do_call();
    }

    const fn ccf(&mut self) {
        self.cpu.af ^= CF;
        self.cpu.af &= !(HF | NF);
    }

    fn cp_a_d8(&mut self) {
        let val = self.imm8();
        self.cp(val);
    }

    fn cp_a_r(&mut self, op: u8) {
        let val = self.get_r(op);
        self.cp(val);
    }

    const fn cpl(&mut self) {
        self.cpu.af ^= 0xFF00;
        self.cpu.af |= HF | NF;
    }

    const fn daa(&mut self) {
        let a = {
            let mut a = self.cpu.af >> 8;

            if self.cpu.af & NF == 0 {
                if self.cpu.af & HF != 0 || a & 0x0F > 0x09 {
                    a += 0x06;
                }
                if self.cpu.af & CF != 0 || a > 0x9F {
                    a += 0x60;
                }
            } else {
                if self.cpu.af & HF != 0 {
                    a = a.wrapping_sub(0x06) & 0xFF;
                }
                if self.cpu.af & CF != 0 {
                    a = a.wrapping_sub(0x60);
                }
            }

            a
        };

        self.cpu.af &= !(0xFF00 | ZF | HF);

        if a.trailing_zeros() >= 8 {
            self.cpu.af |= ZF;
        }

        if a & 0x100 == 0x100 {
            self.cpu.af |= CF;
        }

        self.cpu.af |= a << 8;
    }

    fn dec_dhl(&mut self) {
        let val = self.read_cpu(self.cpu.hl).wrapping_sub(1);
        self.write_cpu(self.cpu.hl, val);

        self.cpu.af &= !(ZF | HF);
        self.cpu.af |= NF;
        if (val & 0x0F) == 0x0F {
            self.cpu.af |= HF;
        }

        if val == 0 {
            self.cpu.af |= ZF;
        }
    }

    fn dec_hr(&mut self, op: u8) {
        let id = Self::opcode_to_reg_id_no_sp(op);
        let rr = self.get_rr(id).wrapping_sub(0x100);
        self.set_rr(id, rr);
        self.cpu.af &= !(ZF | HF);
        self.cpu.af |= NF;

        if rr & 0x0F00 == 0xF00 {
            self.cpu.af |= HF;
        }

        if rr & 0xFF00 == 0 {
            self.cpu.af |= ZF;
        }
    }

    fn dec_lr(&mut self, op: u8) {
        let id = Self::opcode_to_reg_id(op);
        let val = self.get_rr(id).wrapping_sub(1) & 0xFF;
        let rr = self.get_rr(id) & 0xFF00 | val;
        self.set_rr(id, rr);

        self.cpu.af &= !(ZF | HF);
        self.cpu.af |= NF;

        if rr & 0x0F == 0xF {
            self.cpu.af |= HF;
        }

        if rr.trailing_zeros() >= 8 {
            self.cpu.af |= ZF;
        }
    }

    fn dec_rr(&mut self, op: u8) {
        let id = Self::opcode_to_reg_id(op);
        self.set_rr(id, self.get_rr(id).wrapping_sub(1));
        self.tick_m_cycle();
    }

    const fn di(&mut self) {
        self.ints.disable();
    }

    const fn ei(&mut self) {
        self.cpu.has_ei_delay = true;
    }

    const fn halt(&mut self) {
        if !self.ints.is_any_requested() {
            self.cpu.is_halted = true;
        } else if self.ints.are_enabled() {
            self.cpu.is_halted = false;
            self.cpu.pc = self.cpu.pc.wrapping_sub(1);
        } else {
            self.cpu.is_halted = false;
            self.cpu.is_halt_bug_triggered = true;
        }
    }

    const fn illegal(&mut self, _op: u8) {
        self.ints.illegal();
        self.cpu.is_halted = true;
    }

    fn inc_dhl(&mut self) {
        let val = self.read_cpu(self.cpu.hl).wrapping_add(1);
        self.write_cpu(self.cpu.hl, val);

        self.cpu.af &= !(NF | ZF | HF);
        if val.trailing_zeros() >= 4 {
            self.cpu.af |= HF;
        }

        if val == 0 {
            self.cpu.af |= ZF;
        }
    }

    fn inc_hr(&mut self, op: u8) {
        let id = Self::opcode_to_reg_id_no_sp(op);
        let rr = self.get_rr(id).wrapping_add(0x100);
        self.set_rr(id, rr);
        self.cpu.af &= !(NF | ZF | HF);

        if rr & 0x0F00 == 0 {
            self.cpu.af |= HF;
        }

        if rr & 0xFF00 == 0 {
            self.cpu.af |= ZF;
        }
    }

    fn inc_lr(&mut self, op: u8) {
        let id = Self::opcode_to_reg_id(op);
        let val = self.get_rr(id).wrapping_add(1) & 0xFF;
        let rr = self.get_rr(id) & 0xFF00 | val;
        self.set_rr(id, rr);

        self.cpu.af &= !(NF | ZF | HF);

        if rr.trailing_zeros() >= 4 {
            self.cpu.af |= HF;
        }

        if rr.trailing_zeros() >= 8 {
            self.cpu.af |= ZF;
        }
    }

    fn inc_rr(&mut self, op: u8) {
        let id = Self::opcode_to_reg_id(op);
        self.set_rr(id, self.get_rr(id).wrapping_add(1));
        self.tick_m_cycle();
    }

    fn jp_a16(&mut self) {
        self.do_jump_to_immediate();
    }

    fn jp_cc(&mut self, op: u8) {
        if self.satisfies_branch_condition(op) {
            self.do_jump_to_immediate();
        } else {
            let pc = self.cpu.pc.wrapping_add(2);
            self.cpu.pc = pc;
            self.tick_m_cycle();
            self.tick_m_cycle();
        }
    }

    const fn jp_hl(&mut self) {
        self.cpu.pc = self.cpu.hl;
    }

    fn jr_cc(&mut self, op: u8) {
        if self.satisfies_branch_condition(op) {
            self.do_jump_relative();
        } else {
            self.cpu.pc = self.cpu.pc.wrapping_add(1);
            self.tick_m_cycle();
        }
    }

    fn jr_d(&mut self) {
        self.do_jump_relative();
    }

    fn ld(&mut self, op: u8) {
        let val = self.get_r(op);
        self.set_r(op >> 3, val);
    }

    fn ld16_sp_hl(&mut self) {
        let val = self.cpu.hl;
        self.cpu.sp = val;
        self.tick_m_cycle();
    }

    fn ld_a_da16(&mut self) {
        self.cpu.af &= 0xFF;
        let addr = self.imm16();
        self.cpu.af |= u16::from(self.read_cpu(addr)) << 8;
    }

    fn ld_a_dhld(&mut self) {
        let addr = self.cpu.hl;
        let val = u16::from(self.read_cpu(addr));
        self.cpu.af &= 0xFF;
        self.cpu.af |= val << 8;
        self.cpu.hl = addr.wrapping_sub(1);
    }

    fn ld_a_dhli(&mut self) {
        let addr = self.cpu.hl;
        let val = u16::from(self.read_cpu(addr));
        self.cpu.af &= 0xFF;
        self.cpu.af |= val << 8;
        self.cpu.hl = addr.wrapping_add(1);
    }

    fn ld_a_drr(&mut self, op: u8) {
        let id = Self::opcode_to_reg_id(op);
        self.cpu.af &= 0xFF;
        let addr = self.get_rr(id);
        self.cpu.af |= u16::from(self.read_cpu(addr)) << 8;
    }

    // Sets the debug breakpoint flag. Test ROMs like cgb-acid2 and dmg-acid2
    // use this instruction as a breakpoint to signal test completion.
    const fn ld_b_b(&mut self) {
        self.ld_b_b_breakpoint = true;
        self.nop();
    }

    fn ld_da16_a(&mut self) {
        let addr = self.imm16();
        self.write_cpu(addr, self.cpu.a());
    }

    fn ld_da16_sp(&mut self) {
        let val = self.cpu.sp;
        let addr = self.imm16();
        self.write_cpu(addr, (val & 0xFF) as u8);
        self.write_cpu(addr.wrapping_add(1), (val >> 8) as u8);
    }

    fn ld_dhl_d8(&mut self) {
        let tmp = self.imm8();
        self.write_cpu(self.cpu.hl, tmp);
    }

    fn ld_dhld_a(&mut self) {
        let addr = self.cpu.hl;
        self.write_cpu(addr, self.cpu.a());
        self.cpu.hl = addr.wrapping_sub(1);
    }

    fn ld_dhli_a(&mut self) {
        let addr = self.cpu.hl;
        self.write_cpu(addr, self.cpu.a());
        self.cpu.hl = addr.wrapping_add(1);
    }

    fn ld_drr_a(&mut self, op: u8) {
        let id = Self::opcode_to_reg_id(op);
        let addr = self.get_rr(id);
        self.write_cpu(addr, self.cpu.a());
    }

    fn ld_hl_sp_r8(&mut self) {
        self.cpu.af &= 0xFF00;
        #[expect(clippy::cast_sign_loss)]
        let offset = self.imm8().cast_signed() as u16;
        self.tick_m_cycle();
        self.cpu.hl = self.cpu.sp.wrapping_add(offset);

        if (self.cpu.sp & 0xF) + (offset & 0xF) > 0xF {
            self.cpu.af |= HF;
        }

        if (self.cpu.sp & 0xFF) + (offset & 0xFF) > 0xFF {
            self.cpu.af |= CF;
        }
    }

    fn ld_hr_d8(&mut self, op: u8) {
        let id = Self::opcode_to_reg_id_no_sp(op);
        let hi = u16::from(self.imm8());
        self.set_rr(id, (hi << 8) | self.get_rr(id) & 0xFF);
    }

    fn ld_lr_d8(&mut self, op: u8) {
        let id = Self::opcode_to_reg_id(op);
        let lo = u16::from(self.imm8());
        self.set_rr(id, self.get_rr(id) & 0xFF00 | lo);
    }

    fn ld_rr_d16(&mut self, op: u8) {
        let id = Self::opcode_to_reg_id(op);
        let imm = self.imm16();
        self.set_rr(id, imm);
    }

    fn ldh_a_da8(&mut self) {
        let tmp = u16::from(self.imm8());
        self.cpu.af &= 0xFF;
        self.cpu.af |= u16::from(self.read_cpu(0xFF00 | tmp)) << 8;
    }

    fn ldh_a_dc(&mut self) {
        self.cpu.af &= 0xFF;
        self.cpu.af |= u16::from(self.read_cpu(0xFF00 | self.cpu.bc & 0xFF)) << 8;
    }

    fn ldh_da8_a(&mut self) {
        let tmp = u16::from(self.imm8());
        let a = self.cpu.a();
        self.write_cpu(0xFF00 | tmp, a);
    }

    fn ldh_dc_a(&mut self) {
        self.write_cpu(0xFF00 | self.cpu.bc & 0xFF, self.cpu.a());
    }

    #[expect(clippy::unused_self)]
    const fn nop(&self) {}

    fn or_a_d8(&mut self) {
        let val = self.imm8();
        self.or(val);
    }

    fn or_a_r(&mut self, op: u8) {
        let val = self.get_r(op);
        self.or(val);
    }

    fn pop_rr(&mut self, op: u8) {
        let val = self.pop();
        let id = Self::opcode_to_reg_id_no_sp(op);
        self.set_rr(id, val);
        self.cpu.af &= 0xFFF0;
    }

    fn push_rr(&mut self, op: u8) {
        let id = Self::opcode_to_reg_id_no_sp(op);
        self.push(self.get_rr(id));
    }

    fn ret(&mut self) {
        self.cpu.pc = self.pop();
        self.tick_m_cycle();
    }

    fn ret_cc(&mut self, op: u8) {
        self.tick_m_cycle();

        if self.satisfies_branch_condition(op) {
            self.ret();
        }
    }

    fn reti(&mut self) {
        self.ret();
        self.ints.enable();
    }

    fn rl_r(&mut self, op: u8) {
        let val = self.get_r(op);
        let carry = self.cpu.af & CF != 0;
        let bit7 = val & 0x80 != 0;

        self.cpu.af &= 0xFF00;
        let val = (val << 1) | u8::from(carry);
        self.set_r(op, val);
        if bit7 {
            self.cpu.af |= CF;
        }
        if val == 0 {
            self.cpu.af |= ZF;
        }
    }

    fn rla(&mut self) {
        let bit7 = self.cpu.af & 0x8000 != 0;
        let carry = self.cpu.af & CF != 0;

        self.cpu.af = ((self.cpu.af & 0xFF00) << 1) | (u16::from(carry) << 8);

        if bit7 {
            self.cpu.af |= CF;
        }
    }

    fn rlc_r(&mut self, op: u8) {
        let val = self.get_r(op);
        let carry = val & 0x80 != 0;
        self.cpu.af &= 0xFF00;
        self.set_r(op, (val << 1) | u8::from(carry));
        if carry {
            self.cpu.af |= CF;
        }
        if val == 0 {
            self.cpu.af |= ZF;
        }
    }

    const fn rlca(&mut self) {
        let carry = (self.cpu.af & 0x8000) != 0;

        self.cpu.af = (self.cpu.af & 0xFF00) << 1;
        if carry {
            self.cpu.af |= CF | 0x0100;
        }
    }

    fn rr_r(&mut self, op: u8) {
        let val = self.get_r(op);
        let carry = self.cpu.af & CF != 0;
        let bit1 = val & 1 != 0;
        let val = (val >> 1) | (u8::from(carry) << 7);
        self.set_r(op, val);

        self.cpu.af &= 0xFF00;
        if bit1 {
            self.cpu.af |= CF;
        }
        if val == 0 {
            self.cpu.af |= ZF;
        }
    }

    fn rra(&mut self) {
        let bit1 = self.cpu.af & 0x0100 != 0;
        let carry = self.cpu.af & CF != 0;

        self.cpu.af = (self.cpu.af >> 1) & 0xFF00 | (u16::from(carry) << 15);
        if bit1 {
            self.cpu.af |= CF;
        }
    }

    fn rrc_r(&mut self, op: u8) {
        let val = self.get_r(op);
        let carry = (val & 0x01) != 0;
        self.cpu.af &= 0xFF00;
        let val = (val >> 1) | (u8::from(carry) << 7);
        self.set_r(op, val);
        if carry {
            self.cpu.af |= CF;
        }
        if val == 0 {
            self.cpu.af |= ZF;
        }
    }

    const fn rrca(&mut self) {
        let carry = self.cpu.af & 0x100 != 0;
        self.cpu.af = (self.cpu.af >> 1) & 0xFF00;
        if carry {
            self.cpu.af |= CF | 0x8000;
        }
    }

    fn rst(&mut self, op: u8) {
        self.push(self.cpu.pc);
        self.cpu.pc = u16::from(op) ^ 0xC7;
    }

    fn sbc_a_d8(&mut self) {
        let val = self.imm8();
        self.sbc(val);
    }

    fn sbc_a_r(&mut self, op: u8) {
        let val = self.get_r(op);
        self.sbc(val);
    }

    const fn scf(&mut self) {
        self.cpu.af |= CF;
        self.cpu.af &= !(HF | NF);
    }

    fn sla_r(&mut self, op: u8) {
        let val = self.get_r(op);
        let carry = val & 0x80 != 0;
        let res = val << 1;
        self.set_r(op, res);

        self.cpu.af &= 0xFF00;
        if carry {
            self.cpu.af |= CF;
        }
        if res == 0 {
            self.cpu.af |= ZF;
        }
    }

    fn sra_r(&mut self, op: u8) {
        let val = self.get_r(op);
        let bit7 = val & 0x80;
        self.cpu.af &= 0xFF00;
        if val & 1 != 0 {
            self.cpu.af |= CF;
        }
        let val = (val >> 1) | bit7;
        self.set_r(op, val);
        if val == 0 {
            self.cpu.af |= ZF;
        }
    }

    fn srl_r(&mut self, op: u8) {
        let val = self.get_r(op);
        self.cpu.af &= 0xFF00;
        self.set_r(op, val >> 1);
        if val & 1 != 0 {
            self.cpu.af |= CF;
        }
        if val >> 1 == 0 {
            self.cpu.af |= ZF;
        }
    }

    fn stop(&mut self) {
        let _discard_byte = self.imm8();

        if self.key1.is_requested() {
            self.key1.change_speed();
            self.write_div();

            for _ in 0..2050 {
                // TODO: div should not tick during speed change, check this
                self.advance_dots_no_timers(4);
            }
        } else {
            self.cpu.is_halted = true;
            self.ppu.enter_stop_mode();
        }
    }

    fn sub_a_d8(&mut self) {
        let val = self.imm8();
        self.sub(val);
    }

    fn sub_a_r(&mut self, op: u8) {
        let val = self.get_r(op);
        self.sub(val);
    }

    fn swap_r(&mut self, op: u8) {
        let val = self.get_r(op);
        self.cpu.af &= 0xFF00;
        self.set_r(op, val.rotate_left(4));
        if val == 0 {
            self.cpu.af |= ZF;
        }
    }

    fn xor_a_d8(&mut self) {
        let val = self.imm8();
        self.xor(val);
    }

    fn xor_a_r(&mut self, op: u8) {
        let val = self.get_r(op);
        self.xor(val);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AudioCallback, GbBuilder, Model, Sample};

    struct DummyAudio;
    impl AudioCallback for DummyAudio {
        fn audio_sample(&self, _l: Sample, _r: Sample) {}
    }

    fn setup_gb() -> crate::Gb<DummyAudio> {
        GbBuilder::new(44100, DummyAudio)
            .with_model(Model::DmgB)
            .build()
    }

    fn test_op_timing<A: AudioCallback>(
        gb: &mut crate::Gb<A>,
        opcode: u8,
        operands: &[u8],
        expected_m_cycles: u64,
    ) {
        let addr = 0xC000;
        gb.cpu.pc = addr;
        gb.write_mem(addr, opcode);
        for (i, &op) in operands.iter().enumerate() {
            gb.write_mem(addr + 1 + i as u16, op);
        }

        let start_dots = gb.total_dots;
        gb.run_cpu();
        let end_dots = gb.total_dots;
        let elapsed_dots = end_dots - start_dots;
        assert_eq!(
            elapsed_dots,
            expected_m_cycles * 4,
            "Opcode 0x{:02X} took {} dots, expected {}",
            opcode,
            elapsed_dots,
            expected_m_cycles * 4
        );
    }

    fn test_cb_timing<A: AudioCallback>(
        gb: &mut crate::Gb<A>,
        cb_opcode: u8,
        expected_m_cycles: u64,
    ) {
        let addr = 0xC000;
        gb.cpu.pc = addr;
        gb.write_mem(addr, 0xCB);
        gb.write_mem(addr + 1, cb_opcode);

        let start_dots = gb.total_dots;
        gb.run_cpu();
        let end_dots = gb.total_dots;
        let elapsed_dots = end_dots - start_dots;
        assert_eq!(
            elapsed_dots,
            expected_m_cycles * 4,
            "CB Opcode 0x{:02X} took {} dots, expected {}",
            cb_opcode,
            elapsed_dots,
            expected_m_cycles * 4
        );
    }

    #[test]
    fn test_add() {
        let mut gb = setup_gb();
        // 0x12 + 0x34 = 0x46
        gb.cpu.af = 0x1200;
        gb.add(0x34);
        assert_eq!(gb.cpu.a(), 0x46);
        assert_eq!(gb.cpu.f(), 0);

        // 0xFF + 0x01 = 0x00 (Carry, Half-Carry, Zero)
        gb.cpu.af = 0xFF00;
        gb.add(0x01);
        assert_eq!(gb.cpu.a(), 0x00);
        assert_eq!(gb.cpu.f(), (ZF | HF | CF) as u8);

        // 0x0F + 0x01 = 0x10 (Half-Carry)
        gb.cpu.af = 0x0F00;
        gb.add(0x01);
        assert_eq!(gb.cpu.a(), 0x10);
        assert_eq!(gb.cpu.f(), HF as u8);
    }

    #[test]
    fn test_adc() {
        let mut gb = setup_gb();
        // 0x12 + 0x34 + carry(0) = 0x46
        gb.cpu.af = 0x1200;
        gb.adc(0x34);
        assert_eq!(gb.cpu.a(), 0x46);
        assert_eq!(gb.cpu.f(), 0);

        // 0x12 + 0x34 + carry(1) = 0x47
        gb.cpu.af = 0x1200 | CF;
        gb.adc(0x34);
        assert_eq!(gb.cpu.a(), 0x47);
        assert_eq!(gb.cpu.f(), 0);

        // 0x0F + 0x00 + carry(1) = 0x10 (Half-Carry)
        gb.cpu.af = 0x0F00 | CF;
        gb.adc(0x00);
        assert_eq!(gb.cpu.a(), 0x10);
        assert_eq!(gb.cpu.f(), HF as u8);
    }

    #[test]
    fn test_sub() {
        let mut gb = setup_gb();
        // 0x34 - 0x12 = 0x22
        gb.cpu.af = 0x3400;
        gb.sub(0x12);
        assert_eq!(gb.cpu.a(), 0x22);
        assert_eq!(gb.cpu.f(), NF as u8);

        // 0x00 - 0x01 = 0xFF (Carry, Half-Carry)
        gb.cpu.af = 0x0000;
        gb.sub(0x01);
        assert_eq!(gb.cpu.a(), 0xFF);
        assert_eq!(gb.cpu.f(), (NF | HF | CF) as u8);

        // 0x10 - 0x01 = 0x0F (Half-Carry)
        gb.cpu.af = 0x1000;
        gb.sub(0x01);
        assert_eq!(gb.cpu.a(), 0x0F);
        assert_eq!(gb.cpu.f(), (NF | HF) as u8);
    }

    #[test]
    fn test_sbc() {
        let mut gb = setup_gb();
        // 0x34 - 0x12 - carry(0) = 0x22
        gb.cpu.af = 0x3400;
        gb.sbc(0x12);
        assert_eq!(gb.cpu.a(), 0x22);
        assert_eq!(gb.cpu.f(), NF as u8);

        // 0x34 - 0x12 - carry(1) = 0x21
        gb.cpu.af = 0x3400 | CF;
        gb.sbc(0x12);
        assert_eq!(gb.cpu.a(), 0x21);
        assert_eq!(gb.cpu.f(), NF as u8);

        // 0x10 - 0x00 - carry(1) = 0x0F (Half-Carry)
        gb.cpu.af = 0x1000 | CF;
        gb.sbc(0x00);
        assert_eq!(gb.cpu.a(), 0x0F);
        assert_eq!(gb.cpu.f(), (NF | HF) as u8);
    }

    #[test]
    fn test_logical() {
        let mut gb = setup_gb();
        // AND
        gb.cpu.af = 0xFF00;
        gb.and(0x0F);
        assert_eq!(gb.cpu.a(), 0x0F);
        assert_eq!(gb.cpu.f(), HF as u8);

        // OR
        gb.cpu.af = 0xF000;
        gb.or(0x0F);
        assert_eq!(gb.cpu.a(), 0xFF);
        assert_eq!(gb.cpu.f(), 0);

        // XOR
        gb.cpu.af = 0xAA00;
        gb.xor(0xFF);
        assert_eq!(gb.cpu.a(), 0x55);
        assert_eq!(gb.cpu.f(), 0);
    }

    #[test]
    fn test_daa() {
        let mut gb = setup_gb();
        // 0x45 + 0x38 = 0x7D -> DAA -> 0x83
        gb.cpu.af = 0x4500;
        gb.add(0x38);
        gb.daa();
        assert_eq!(gb.cpu.a(), 0x83);

        // 0x83 - 0x38 = 0x4B -> DAA -> 0x45
        gb.cpu.af = 0x8300;
        gb.sub(0x38);
        gb.daa();
        assert_eq!(gb.cpu.a(), 0x45);
    }

    #[test]
    fn test_inc_dec() {
        let mut gb = setup_gb();
        // INC B (0x04)
        gb.cpu.bc = 0x0000;
        gb.inc_hr(0x04);
        assert_eq!(gb.cpu.bc >> 8, 1);
        assert_eq!(gb.cpu.f() & (NF as u8), 0);

        // DEC B (0x05)
        gb.dec_hr(0x05);
        assert_eq!(gb.cpu.bc >> 8, 0);
        assert_eq!(gb.cpu.f() & (ZF as u8), ZF as u8);
        assert_eq!(gb.cpu.f() & (NF as u8), NF as u8);

        // INC BC (0x03) - 16-bit, no flags
        gb.cpu.af = 0;
        gb.cpu.bc = 0xFFFF;
        gb.inc_rr(0x03);
        assert_eq!(gb.cpu.bc, 0x0000);
        assert_eq!(gb.cpu.f(), 0);
    }

    #[test]
    fn test_rotates() {
        let mut gb = setup_gb();
        // RLCA
        gb.cpu.af = 0x8000;
        gb.rlca();
        assert_eq!(gb.cpu.a(), 0x01);
        assert_eq!(gb.cpu.f(), CF as u8);

        // RRCA
        gb.cpu.af = 0x0100;
        gb.rrca();
        assert_eq!(gb.cpu.a(), 0x80);
        assert_eq!(gb.cpu.f(), CF as u8);
    }

    #[test]
    fn test_shifts() {
        let mut gb = setup_gb();
        // SLA A (0x27)
        gb.cpu.af = 0x8000;
        gb.sla_r(0x27);
        assert_eq!(gb.cpu.a(), 0x00);
        assert_eq!(gb.cpu.f(), (ZF | CF) as u8);

        // SRA A (0x2F)
        gb.cpu.af = 0x8100;
        gb.sra_r(0x2F);
        assert_eq!(gb.cpu.a(), 0xC0);
        assert_eq!(gb.cpu.f(), CF as u8);

        // SRL A (0x3F)
        gb.cpu.af = 0x8100;
        gb.srl_r(0x3F);
        assert_eq!(gb.cpu.a(), 0x40);
        assert_eq!(gb.cpu.f(), CF as u8);

        // SWAP A (0x37)
        gb.cpu.af = 0x1200;
        gb.swap_r(0x37);
        assert_eq!(gb.cpu.a(), 0x21);
        assert_eq!(gb.cpu.f(), 0);
    }

    #[test]
    fn test_daa_edge_cases() {
        let mut gb = setup_gb();

        // Addition: 0x99 + 0x01 = 0x9A -> DAA -> 0x00 (Carry)
        gb.cpu.af = 0x9900;
        gb.add(0x01);
        gb.daa();
        assert_eq!(gb.cpu.a(), 0x00);
        assert_eq!(gb.cpu.f() & (CF as u8), CF as u8);
        assert_eq!(gb.cpu.f() & (ZF as u8), ZF as u8);

        // Subtraction: 0x00 - 0x01 = 0xFF (C, H, N) -> DAA -> 0x99 (Carry)
        gb.cpu.af = 0x0000;
        gb.sub(0x01);
        gb.daa();
        assert_eq!(gb.cpu.a(), 0x99);
        assert_eq!(gb.cpu.f() & (CF as u8), CF as u8);
    }

    #[test]
    fn test_add_hl() {
        let mut gb = setup_gb();
        // 0x1234 + 0x1111 = 0x2345
        gb.cpu.af = NF | CF | HF; // These should be cleared
        gb.cpu.hl = 0x1234;
        gb.cpu.bc = 0x1111;
        gb.add_hl_rr(0x09); // ADD HL, BC (opcode 0x09)
        assert_eq!(gb.cpu.hl, 0x2345);
        assert_eq!(gb.cpu.f() & (NF | CF | HF) as u8, 0);

        // 0x0FFF + 0x0001 = 0x1000 (Half-Carry)
        gb.cpu.hl = 0x0FFF;
        gb.cpu.bc = 0x0001;
        gb.add_hl_rr(0x09);
        assert_eq!(gb.cpu.hl, 0x1000);
        assert_eq!(gb.cpu.f() & (HF as u8), HF as u8);

        // 0xFFFF + 0x0001 = 0x0000 (Carry, Half-Carry)
        gb.cpu.hl = 0xFFFF;
        gb.cpu.bc = 0x0001;
        gb.add_hl_rr(0x09);
        assert_eq!(gb.cpu.hl, 0x0000);
        assert_eq!(gb.cpu.f() & (CF | HF) as u8, (CF | HF) as u8);
    }

    #[test]
    fn test_bit_ops() {
        let mut gb = setup_gb();
        // SET 7, A (0xFF) - using bit_r internal logic
        gb.cpu.af = 0x0000;
        gb.bit_r(0xFF); // SET 7, A
        assert_eq!(gb.cpu.a(), 0x80);

        // BIT 7, A (0x7F)
        gb.bit_r(0x7F);
        assert_eq!(gb.cpu.f() & (ZF as u8), 0);
        assert_eq!(gb.cpu.f() & (HF as u8), HF as u8);

        // BIT 6, A (0x77)
        gb.bit_r(0x40 | (6 << 3) | 7);
        assert_eq!(gb.cpu.f() & (ZF as u8), ZF as u8);

        // RES 7, A (0xBF)
        gb.bit_r(0xBF);
        assert_eq!(gb.cpu.a(), 0x00);
    }

    #[test]
    fn test_timing() {
        let mut gb = setup_gb();
        gb.cpu.sp = 0xD000; // Point stack to WRAM

        // Basic
        test_op_timing(&mut gb, 0x00, &[], 1); // NOP
        test_op_timing(&mut gb, 0x7F, &[], 1); // LD A, A
        test_op_timing(&mut gb, 0x06, &[0x42], 2); // LD B, d8
        test_op_timing(&mut gb, 0x46, &[], 2); // LD B, (HL)
        test_op_timing(&mut gb, 0x70, &[], 2); // LD (HL), B
        test_op_timing(&mut gb, 0x36, &[0x42], 3); // LD (HL), d8
        test_op_timing(&mut gb, 0x01, &[0x34, 0x12], 3); // LD BC, d16
        test_op_timing(&mut gb, 0x08, &[0x34, 0x12], 5); // LD (a16), SP

        // Arithmetic
        test_op_timing(&mut gb, 0x04, &[], 1); // INC B
        test_op_timing(&mut gb, 0x03, &[], 2); // INC BC
        test_op_timing(&mut gb, 0x34, &[], 3); // INC (HL)
        test_op_timing(&mut gb, 0x09, &[], 2); // ADD HL, BC
        test_op_timing(&mut gb, 0x80, &[], 1); // ADD A, B
        test_op_timing(&mut gb, 0xC6, &[0x42], 2); // ADD A, d8
        test_op_timing(&mut gb, 0x86, &[], 2); // ADD A, (HL)

        // Control flow
        test_op_timing(&mut gb, 0x18, &[0x02], 3); // JR e
        test_op_timing(&mut gb, 0xC3, &[0x00, 0xC1], 4); // JP a16
        test_op_timing(&mut gb, 0xE9, &[], 1); // JP (HL)

        // Stack
        test_op_timing(&mut gb, 0xC5, &[], 4); // PUSH BC
        test_op_timing(&mut gb, 0xC1, &[], 3); // POP BC

        // CB
        test_cb_timing(&mut gb, 0x00, 2); // RLC B
        test_cb_timing(&mut gb, 0x06, 4); // RLC (HL)
        test_cb_timing(&mut gb, 0x40, 2); // BIT 0, B
        test_cb_timing(&mut gb, 0x46, 3); // BIT 0, (HL)
    }

    #[test]
    fn test_timing_complex() {
        let mut gb = setup_gb();
        gb.cpu.sp = 0xD000;

        // PUSH BC (0xC5) -> 4
        test_op_timing(&mut gb, 0xC5, &[], 4);
        assert_eq!(gb.cpu.sp, 0xCFFE); // SP - 2

        // POP BC (0xC1) -> 3
        test_op_timing(&mut gb, 0xC1, &[], 3);
        assert_eq!(gb.cpu.sp, 0xD000);

        // CALL a16 (0xCD) -> 6
        test_op_timing(&mut gb, 0xCD, &[0x00, 0xE0], 6);
        assert_eq!(gb.cpu.pc, 0xE000);
        assert_eq!(gb.cpu.sp, 0xCFFE);

        // RET (0xC9) -> 4
        // Note: CALL pushed 0xC003 (C000 + 3 bytes of instruction)
        test_op_timing(&mut gb, 0xC9, &[], 4);
        assert_eq!(gb.cpu.pc, 0xC003);
        assert_eq!(gb.cpu.sp, 0xD000);

        // RST 0x00 (0xC7) -> 4
        test_op_timing(&mut gb, 0xC7, &[], 4);
        assert_eq!(gb.cpu.pc, 0x0000);
    }

    #[test]
    fn test_timing_conditional() {
        let mut gb = setup_gb();

        // JR NZ, r8 (0x20)
        // Not taken (ZF=1)
        gb.cpu.af = ZF;
        gb.cpu.pc = 0xC000;
        gb.write_mem(0xC000, 0x20);
        gb.write_mem(0xC001, 0x05);
        let start = gb.total_dots;
        gb.run_cpu();
        assert_eq!(gb.total_dots - start, 2 * 4);
        assert_eq!(gb.cpu.pc, 0xC002);

        // Taken (ZF=0)
        gb.cpu.af = 0;
        gb.cpu.pc = 0xC000;
        gb.write_mem(0xC000, 0x20);
        gb.write_mem(0xC001, 0x05);
        let start = gb.total_dots;
        gb.run_cpu();
        assert_eq!(gb.total_dots - start, 3 * 4);
        assert_eq!(gb.cpu.pc, 0xC007);
    }
}
