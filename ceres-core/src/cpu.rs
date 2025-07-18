use crate::{AudioCallback, Gb};

const ZF: u16 = 0x80;
const NF: u16 = 0x40;
const HF: u16 = 0x20;
const CF: u16 = 0x10;

#[derive(Debug, Default)]
pub struct Cpu {
    af: u16,
    bc: u16,
    de: u16,
    hl: u16,
    sp: u16,
    pc: u16,

    has_ei_delay: bool,
    is_halted: bool,
    is_halt_bug_triggered: bool,
}

impl Cpu {
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

    pub const fn sp(&self) -> u16 {
        self.sp
    }

    pub const fn pc(&self) -> u16 {
        self.pc
    }

    pub const fn is_halted(&self) -> bool {
        self.is_halted
    }
}

impl<A: AudioCallback> Gb<A> {
    pub fn run_cpu(&mut self) {
        if self.cpu.has_ei_delay {
            self.ints.enable();
            self.cpu.has_ei_delay = false;
        }

        if self.cpu.is_halted {
            self.tick_m_cycle();
        } else {
            // println!("pc {:0x}", self.cpu.pc);

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

            if self.ints.are_enabled() {
                self.tick_m_cycle();
                self.tick_m_cycle();

                self.push(self.cpu.pc);

                self.ints.disable();
                self.cpu.pc = self.ints.handle();
            }
        }
    }

    fn cpu_write(&mut self, addr: u16, val: u8) {
        self.tick_m_cycle();
        self.write_mem(addr, val);
    }

    #[must_use]
    fn read(&mut self, addr: u16) -> u8 {
        self.tick_m_cycle();
        self.read_mem(addr)
    }

    fn tick_m_cycle(&mut self) {
        self.advance_t_cycles(4);
    }

    #[must_use]
    fn imm8(&mut self) -> u8 {
        let val = self.read(self.cpu.pc);
        self.cpu.pc = self.cpu.pc.wrapping_add(1);
        val
    }

    #[must_use]
    fn imm16(&mut self) -> u16 {
        let lo = u16::from(self.imm8());
        let hi = u16::from(self.imm8());
        (hi << 8) | lo
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
    fn get_r(&mut self, op: u8) -> u8 {
        let id = ((op >> 1) + 1) & 3;
        let lo = op & 1 != 0;
        if id == 0 {
            if lo {
                (self.cpu.af >> 8) as u8
            } else {
                self.read(self.cpu.hl)
            }
        } else if lo {
            (self.get_rr(id) & 0xFF) as u8
        } else {
            (self.get_rr(id) >> 8) as u8
        }
    }

    fn set_r(&mut self, op: u8, val: u8) {
        let id = ((op >> 1) + 1) & 3;
        let lo = op & 1 != 0;
        if id == 0 {
            if lo {
                self.cpu.af = (u16::from(val) << 8) | self.cpu.af & 0xFF;
            } else {
                self.cpu_write(self.cpu.hl, val);
            }
        } else if lo {
            self.set_rr(id, u16::from(val) | self.get_rr(id) & 0xFF00);
        } else {
            self.set_rr(id, (u16::from(val) << 8) | self.get_rr(id) & 0xFF);
        }
    }

    fn ld(&mut self, op: u8) {
        let val = self.get_r(op);
        self.set_r(op >> 3, val);
    }

    fn ld_a_drr(&mut self, op: u8) {
        let id = (op >> 4) + 1;
        self.cpu.af &= 0xFF;
        let addr = self.get_rr(id);
        self.cpu.af |= u16::from(self.read(addr)) << 8;
    }

    fn ld_drr_a(&mut self, op: u8) {
        let id = (op >> 4) + 1;
        let addr = self.get_rr(id);
        self.cpu_write(addr, (self.cpu.af >> 8) as u8);
    }

    fn ld_da16_a(&mut self) {
        let addr = self.imm16();
        self.cpu_write(addr, (self.cpu.af >> 8) as u8);
    }

    fn ld_a_da16(&mut self) {
        self.cpu.af &= 0xFF;
        let addr = self.imm16();
        self.cpu.af |= u16::from(self.read(addr)) << 8;
    }

    fn ld_dhli_a(&mut self) {
        let addr = self.cpu.hl;
        self.cpu_write(addr, (self.cpu.af >> 8) as u8);
        self.cpu.hl = addr.wrapping_add(1);
    }

    fn ld_dhld_a(&mut self) {
        let addr = self.cpu.hl;
        self.cpu_write(addr, (self.cpu.af >> 8) as u8);
        self.cpu.hl = addr.wrapping_sub(1);
    }

    fn ld_a_dhli(&mut self) {
        let addr = self.cpu.hl;
        let val = u16::from(self.read(addr));
        self.cpu.af &= 0xFF;
        self.cpu.af |= val << 8;
        self.cpu.hl = addr.wrapping_add(1);
    }

    fn ld_a_dhld(&mut self) {
        let addr = self.cpu.hl;
        let val = u16::from(self.read(addr));
        self.cpu.af &= 0xFF;
        self.cpu.af |= val << 8;
        self.cpu.hl = addr.wrapping_sub(1);
    }

    fn ldh_da8_a(&mut self) {
        let tmp = u16::from(self.imm8());
        let a = (self.cpu.af >> 8) as u8;
        self.cpu_write(0xFF00 | tmp, a);
    }

    fn ldh_a_da8(&mut self) {
        let tmp = u16::from(self.imm8());
        self.cpu.af &= 0xFF;
        self.cpu.af |= u16::from(self.read(0xFF00 | tmp)) << 8;
    }

    fn ldh_dc_a(&mut self) {
        self.cpu_write(0xFF00 | self.cpu.bc & 0xFF, (self.cpu.af >> 8) as u8);
    }

    fn ldh_a_dc(&mut self) {
        self.cpu.af &= 0xFF;
        self.cpu.af |= u16::from(self.read(0xFF00 | self.cpu.bc & 0xFF)) << 8;
    }

    fn ld_hr_d8(&mut self, op: u8) {
        let id = ((op >> 4) + 1) & 0x03;
        let hi = u16::from(self.imm8());
        self.set_rr(id, (hi << 8) | self.get_rr(id) & 0xFF);
    }

    fn ld_lr_d8(&mut self, op: u8) {
        let id = (op >> 4) + 1;
        let lo = u16::from(self.imm8());
        self.set_rr(id, self.get_rr(id) & 0xFF00 | lo);
    }

    fn ld_dhl_d8(&mut self) {
        let tmp = self.imm8();
        self.cpu_write(self.cpu.hl, tmp);
    }

    fn ld16_sp_hl(&mut self) {
        let val = self.cpu.hl;
        self.cpu.sp = val;
        self.tick_m_cycle();
    }

    const fn add(&mut self, val: u16) {
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

    fn add_a_r(&mut self, op: u8) {
        let val = u16::from(self.get_r(op));
        self.add(val);
    }

    fn add_a_d8(&mut self) {
        let val = u16::from(self.imm8());
        self.add(val);
    }

    const fn sub(&mut self, val: u16) {
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

    fn sub_a_r(&mut self, op: u8) {
        let val = u16::from(self.get_r(op));
        self.sub(val);
    }

    fn sub_a_d8(&mut self) {
        let val = u16::from(self.imm8());
        self.sub(val);
    }

    fn sbc(&mut self, val: u16) {
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

    fn sbc_a_r(&mut self, op: u8) {
        let val = u16::from(self.get_r(op));
        self.sbc(val);
    }

    fn sbc_a_d8(&mut self) {
        let val = u16::from(self.imm8());
        self.sbc(val);
    }

    fn adc(&mut self, val: u16) {
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

    fn adc_a_r(&mut self, op: u8) {
        let val = u16::from(self.get_r(op));
        self.adc(val);
    }

    fn adc_a_d8(&mut self) {
        let val = u16::from(self.imm8());
        self.adc(val);
    }

    const fn or(&mut self, val: u16) {
        let a = self.cpu.af >> 8;
        self.cpu.af = (a | val) << 8;
        if a | val == 0 {
            self.cpu.af |= ZF;
        }
    }

    fn or_a_r(&mut self, op: u8) {
        let val = u16::from(self.get_r(op));
        self.or(val);
    }

    fn or_a_d8(&mut self) {
        let val = u16::from(self.imm8());
        self.or(val);
    }

    const fn xor(&mut self, val: u16) {
        let a = self.cpu.af >> 8;
        let a = a ^ val;
        self.cpu.af = a << 8;
        if a == 0 {
            self.cpu.af |= ZF;
        }
    }

    fn xor_a_r(&mut self, op: u8) {
        let val = u16::from(self.get_r(op));
        self.xor(val);
    }

    fn xor_a_d8(&mut self) {
        let val = u16::from(self.imm8());
        self.xor(val);
    }

    const fn and(&mut self, val: u16) {
        let a = self.cpu.af >> 8;
        let a = a & val;
        self.cpu.af = (a << 8) | HF;
        if a == 0 {
            self.cpu.af |= ZF;
        }
    }

    fn and_a_r(&mut self, op: u8) {
        let val = u16::from(self.get_r(op));
        self.and(val);
    }

    fn and_a_d8(&mut self) {
        let val = u16::from(self.imm8());
        self.and(val);
    }

    const fn cp(&mut self, val: u16) {
        let a = self.cpu.af >> 8;
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

    fn cp_a_r(&mut self, op: u8) {
        let val = u16::from(self.get_r(op));
        self.cp(val);
    }

    fn cp_a_d8(&mut self) {
        let val = u16::from(self.imm8());
        self.cp(val);
    }

    fn inc_lr(&mut self, op: u8) {
        let id = (op >> 4) + 1;
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

    fn dec_lr(&mut self, op: u8) {
        let id = (op >> 4) + 1;
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

    fn inc_hr(&mut self, op: u8) {
        let id = ((op >> 4) + 1) & 0x03;
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

    fn dec_hr(&mut self, op: u8) {
        let id = ((op >> 4) + 1) & 0x03;
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

    fn inc_dhl(&mut self) {
        let val = self.read(self.cpu.hl).wrapping_add(1);
        self.cpu_write(self.cpu.hl, val);

        self.cpu.af &= !(NF | ZF | HF);
        if val.trailing_zeros() >= 4 {
            self.cpu.af |= HF;
        }

        if val == 0 {
            self.cpu.af |= ZF;
        }
    }

    fn dec_dhl(&mut self) {
        let val = self.read(self.cpu.hl).wrapping_sub(1);
        self.cpu_write(self.cpu.hl, val);

        self.cpu.af &= !(ZF | HF);
        self.cpu.af |= NF;
        if (val & 0x0F) == 0x0F {
            self.cpu.af |= HF;
        }

        if val == 0 {
            self.cpu.af |= ZF;
        }
    }

    fn inc_rr(&mut self, op: u8) {
        let id = (op >> 4) + 1;
        self.set_rr(id, self.get_rr(id).wrapping_add(1));
        self.tick_m_cycle();
    }

    fn dec_rr(&mut self, op: u8) {
        let id = (op >> 4) + 1;
        self.set_rr(id, self.get_rr(id).wrapping_sub(1));
        self.tick_m_cycle();
    }

    fn ld_hl_sp_r8(&mut self) {
        self.cpu.af &= 0xFF00;
        #[expect(clippy::cast_sign_loss, clippy::cast_possible_wrap)]
        let offset = self.imm8() as i8 as u16;
        self.tick_m_cycle();
        self.cpu.hl = self.cpu.sp.wrapping_add(offset);

        if (self.cpu.sp & 0xF) + (offset & 0xF) > 0xF {
            self.cpu.af |= HF;
        }

        if (self.cpu.sp & 0xFF) + (offset & 0xFF) > 0xFF {
            self.cpu.af |= CF;
        }
    }

    const fn rlca(&mut self) {
        let carry = (self.cpu.af & 0x8000) != 0;

        self.cpu.af = (self.cpu.af & 0xFF00) << 1;
        if carry {
            self.cpu.af |= CF | 0x0100;
        }
    }

    const fn rrca(&mut self) {
        let carry = self.cpu.af & 0x100 != 0;
        self.cpu.af = (self.cpu.af >> 1) & 0xFF00;
        if carry {
            self.cpu.af |= CF | 0x8000;
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

    fn do_jump_to_immediate(&mut self) {
        let addr = self.imm16();
        self.cpu.pc = addr;
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

    fn do_jump_relative(&mut self) {
        #[expect(clippy::cast_sign_loss, clippy::cast_possible_wrap)]
        let offset = self.imm8() as i8 as u16;
        self.cpu.pc = self.cpu.pc.wrapping_add(offset);
        self.tick_m_cycle();
    }

    fn jr_d(&mut self) {
        self.do_jump_relative();
    }

    fn jr_cc(&mut self, op: u8) {
        if self.satisfies_branch_condition(op) {
            self.do_jump_relative();
        } else {
            self.cpu.pc = self.cpu.pc.wrapping_add(1);
            self.tick_m_cycle();
        }
    }

    fn do_call(&mut self) {
        let addr = self.imm16();
        self.push(self.cpu.pc);
        self.cpu.pc = addr;
    }

    fn call_nn(&mut self) {
        self.do_call();
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

    fn ret(&mut self) {
        self.cpu.pc = self.pop();
        self.tick_m_cycle();
    }

    fn reti(&mut self) {
        self.ret();
        self.ints.enable();
    }

    fn ret_cc(&mut self, op: u8) {
        self.tick_m_cycle();

        if self.satisfies_branch_condition(op) {
            self.ret();
        }
    }

    #[must_use]
    const fn satisfies_branch_condition(&self, op: u8) -> bool {
        match (op >> 3) & 3 {
            0 => self.cpu.af & ZF == 0,
            1 => self.cpu.af & ZF != 0,
            2 => self.cpu.af & CF == 0,
            3 => self.cpu.af & CF != 0,
            _ => unreachable!(),
        }
    }

    fn rst(&mut self, op: u8) {
        self.push(self.cpu.pc);
        self.cpu.pc = u16::from(op) ^ 0xC7;
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

    fn stop(&mut self) {
        let _discard_byte = self.imm8();

        if self.key1.is_requested() {
            self.key1.change_speed();
            self.write_div();

            // TODO: div should not tick
            for _ in 0..2050 {
                self.tick_m_cycle();
            }
        } else {
            self.cpu.is_halted = true;
        }
    }

    const fn di(&mut self) {
        self.ints.disable();
    }

    const fn ei(&mut self) {
        self.cpu.has_ei_delay = true;
    }

    const fn ccf(&mut self) {
        self.cpu.af ^= CF;
        self.cpu.af &= !(HF | NF);
    }

    const fn scf(&mut self) {
        self.cpu.af |= CF;
        self.cpu.af &= !(HF | NF);
    }

    #[expect(clippy::unused_self)]
    const fn nop(&self) {}

    // TODO: debugger breakpoint
    #[expect(clippy::needless_pass_by_ref_mut)]
    const fn ld_b_b(&mut self) {
        self.nop();
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

    const fn cpl(&mut self) {
        self.cpu.af ^= 0xFF00;
        self.cpu.af |= HF | NF;
    }

    fn push(&mut self, val: u16) {
        self.cpu.sp = self.cpu.sp.wrapping_sub(1);
        self.cpu_write(self.cpu.sp, (val >> 8) as u8);
        self.cpu.sp = self.cpu.sp.wrapping_sub(1);
        self.cpu_write(self.cpu.sp, (val & 0xFF) as u8);
        self.tick_m_cycle();
    }

    fn push_rr(&mut self, op: u8) {
        let id = ((op >> 4) + 1) & 3;
        self.push(self.get_rr(id));
    }

    #[must_use]
    fn pop(&mut self) -> u16 {
        let val = u16::from(self.read(self.cpu.sp));
        self.cpu.sp = self.cpu.sp.wrapping_add(1);
        let val = val | (u16::from(self.read(self.cpu.sp)) << 8);
        self.cpu.sp = self.cpu.sp.wrapping_add(1);
        val
    }

    fn pop_rr(&mut self, op: u8) {
        let val = self.pop();
        let id = ((op >> 4) + 1) & 3;
        self.set_rr(id, val);
        self.cpu.af &= 0xFFF0;
    }

    fn ld_rr_d16(&mut self, op: u8) {
        let id = (op >> 4) + 1;
        let imm = self.imm16();
        self.set_rr(id, imm);
    }

    fn ld_da16_sp(&mut self) {
        let val = self.cpu.sp;
        let addr = self.imm16();
        self.cpu_write(addr, (val & 0xFF) as u8);
        self.cpu_write(addr.wrapping_add(1), (val >> 8) as u8);
    }

    fn add_sp_r8(&mut self) {
        let sp = self.cpu.sp;
        #[expect(clippy::cast_sign_loss, clippy::cast_possible_wrap)]
        let offset = self.imm8() as i8 as u16;
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

    fn add_hl_rr(&mut self, op: u8) {
        let id = (op >> 4) + 1;
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

    fn rra(&mut self) {
        let bit1 = self.cpu.af & 0x0100 != 0;
        let carry = self.cpu.af & CF != 0;

        self.cpu.af = (self.cpu.af >> 1) & 0xFF00 | (u16::from(carry) << 15);
        if bit1 {
            self.cpu.af |= CF;
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

    fn swap_r(&mut self, op: u8) {
        let val = self.get_r(op);
        self.cpu.af &= 0xFF00;
        self.set_r(op, val.rotate_left(4));
        if val == 0 {
            self.cpu.af |= ZF;
        }
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

    fn rla(&mut self) {
        let bit7 = self.cpu.af & 0x8000 != 0;
        let carry = self.cpu.af & CF != 0;

        self.cpu.af = ((self.cpu.af & 0xFF00) << 1) | (u16::from(carry) << 8);

        if bit7 {
            self.cpu.af |= CF;
        }
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

    const fn illegal(&mut self, _op: u8) {
        self.ints.illegal();
        self.cpu.is_halted = true;
    }

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
}
