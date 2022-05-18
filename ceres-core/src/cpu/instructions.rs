use {
    super::{
        operands::{Get, JumpCondition, Set},
        registers::Reg16::{self, HL},
    },
    crate::{
        mem::{KEY1_SPEED, KEY1_SWITCH},
        Gb,
    },
};

impl Gb {
    pub(super) fn ld<G, S>(&mut self, lhs: S, rhs: G)
    where
        G: Get<u8>,
        S: Set<u8>,
    {
        let val = rhs.get(self);
        lhs.set(self, val);
    }

    pub(super) fn ld16_sp_hl(&mut self) {
        let val = self.reg.read16(HL);
        self.reg.sp = val;
        self.tick_t_cycle();
    }

    pub(super) fn add<G>(&mut self, rhs: G)
    where
        G: Get<u8>,
    {
        let a = self.reg.a;
        let val = rhs.get(self);
        let (output, carry) = a.overflowing_add(val);
        self.reg.set_zf(output == 0);
        self.reg.set_nf(false);
        self.reg.set_hf((a & 0xf) + (val & 0xf) > 0xf);
        self.reg.set_cf(carry);
        self.reg.a = output;
    }

    pub(super) fn adc<G>(&mut self, rhs: G)
    where
        G: Get<u8>,
    {
        let a = self.reg.a;
        let cy = u8::from(self.reg.cf());
        let val = rhs.get(self);
        let output = a.wrapping_add(val).wrapping_add(cy);
        self.reg.set_zf(output == 0);
        self.reg.set_nf(false);
        self.reg.set_hf((a & 0xf) + (val & 0xf) + cy > 0xf);
        self.reg
            .set_cf(u16::from(a) + u16::from(val) + u16::from(cy) > 0xff);
        self.reg.a = output;
    }

    pub(super) fn sub<G>(&mut self, rhs: G)
    where
        G: Get<u8>,
    {
        let a = self.reg.a;
        let val = rhs.get(self);
        let (output, carry) = a.overflowing_sub(val);
        self.reg.set_zf(output == 0);
        self.reg.set_nf(true);
        self.reg.set_hf(a & 0xf < val & 0xf);
        self.reg.set_cf(carry);
        self.reg.a = output;
    }

    pub(super) fn sbc<G>(&mut self, rhs: G)
    where
        G: Get<u8>,
    {
        let a = self.reg.a;
        let cy = u8::from(self.reg.cf());
        let val = rhs.get(self);
        let output = a.wrapping_sub(val).wrapping_sub(cy);
        self.reg.set_zf(output == 0);
        self.reg.set_nf(true);
        self.reg.set_hf(a & 0xf < (val & 0xf) + cy);
        self.reg
            .set_cf(u16::from(a) < u16::from(val) + u16::from(cy));
        self.reg.a = output;
    }

    pub(super) fn cp<G>(&mut self, rhs: G)
    where
        G: Get<u8>,
    {
        let a = self.reg.a;
        let val = rhs.get(self);
        let result = a.wrapping_sub(val);
        self.reg.set_zf(result == 0);
        self.reg.set_nf(true);
        self.reg
            .set_hf((a & 0xf).wrapping_sub(val & 0xf) & (0xf + 1) != 0);
        self.reg.set_cf(a < val);
    }

    pub(super) fn and<G>(&mut self, rhs: G)
    where
        G: Get<u8>,
    {
        let val = rhs.get(self);
        let a = self.reg.a & val;
        self.reg.a = a;
        self.reg.set_zf(a == 0);
        self.reg.set_nf(false);
        self.reg.set_hf(true);
        self.reg.set_cf(false);
    }

    pub(super) fn or<G>(&mut self, rhs: G)
    where
        G: Get<u8>,
    {
        let val = rhs.get(self);
        let a = self.reg.a | val;
        self.reg.a = a;
        self.reg.set_zf(a == 0);
        self.reg.set_nf(false);
        self.reg.set_hf(false);
        self.reg.set_cf(false);
    }

    pub(super) fn xor<G>(&mut self, rhs: G)
    where
        G: Get<u8>,
    {
        let read = rhs.get(self);
        let a = self.reg.a ^ read;
        self.reg.a = a;
        self.reg.set_zf(a == 0);
        self.reg.set_nf(false);
        self.reg.set_hf(false);
        self.reg.set_cf(false);
    }

    pub(super) fn inc<GS>(&mut self, rhs: GS)
    where
        GS: Get<u8> + Set<u8>,
    {
        let read = rhs.get(self);
        let val = read.wrapping_add(1);
        self.reg.set_zf(val == 0);
        self.reg.set_nf(false);
        self.reg.set_hf(read & 0xf == 0xf);
        rhs.set(self, val);
    }

    pub(super) fn inc16(&mut self, reg: Reg16) {
        let read = self.reg.read16(reg);
        let val = read.wrapping_add(1);
        self.reg.write16(reg, val);
        self.tick_t_cycle();
    }

    pub(super) fn dec<GS>(&mut self, rhs: GS)
    where
        GS: Get<u8> + Set<u8>,
    {
        let read = rhs.get(self);
        let val = read.wrapping_sub(1);
        self.reg.set_zf(val == 0);
        self.reg.set_nf(true);
        self.reg.set_hf(read.trailing_zeros() >= 4);
        rhs.set(self, val);
    }

    pub(super) fn dec16(&mut self, reg: Reg16) {
        let read = self.reg.read16(reg);
        let val = read.wrapping_sub(1);
        self.reg.write16(reg, val);
        self.tick_t_cycle();
    }

    pub(super) fn ld16_hl_sp_dd(&mut self) {
        let offset = self.imm8() as i8 as u16;
        let sp = self.reg.sp;
        let val = sp.wrapping_add(offset);
        let tmp = sp ^ val ^ offset;
        self.reg.write16(HL, val);
        self.reg.set_zf(false);
        self.reg.set_nf(false);
        self.reg.set_hf((tmp & 0x10) == 0x10);
        self.reg.set_cf((tmp & 0x100) == 0x100);
        self.tick_t_cycle();
    }

    pub(super) fn rlca(&mut self) {
        let val = self.internal_rlc(self.reg.a);
        self.reg.a = val;
        self.reg.set_zf(false);
    }

    pub(super) fn rla(&mut self) {
        let val = self.internal_rl(self.reg.a);
        self.reg.a = val;
        self.reg.set_zf(false);
    }

    pub(super) fn rrca(&mut self) {
        let val = self.internal_rrc(self.reg.a);
        self.reg.a = val;
        self.reg.set_zf(false);
    }

    pub(super) fn rra(&mut self) {
        let val = self.internal_rr(self.reg.a);
        self.reg.a = val;
        self.reg.set_zf(false);
    }

    pub(super) fn do_jump_to_immediate(&mut self) {
        let addr = self.imm16();
        self.reg.pc = addr;
        self.tick_t_cycle();
    }

    pub(super) fn jp_nn(&mut self) {
        self.do_jump_to_immediate();
    }

    pub(super) fn jp_f(&mut self, condition: JumpCondition) {
        if condition.check(self) {
            self.do_jump_to_immediate();
        } else {
            let pc = self.reg.pc.wrapping_add(2);
            self.reg.pc = pc;
            self.tick_t_cycle();
            self.tick_t_cycle();
        }
    }

    pub(super) fn jp_hl(&mut self) {
        let addr = self.reg.read16(HL);
        self.reg.pc = addr;
    }

    fn do_jump_relative(&mut self) {
        let relative_addr = self.imm8() as i8 as u16;
        let pc = self.reg.pc.wrapping_add(relative_addr);
        self.reg.pc = pc;
        self.tick_t_cycle();
    }

    pub(super) fn jr_d(&mut self) {
        self.do_jump_relative();
    }

    pub(super) fn jr_f(&mut self, condition: JumpCondition) {
        if condition.check(self) {
            self.do_jump_relative();
        } else {
            self.reg.inc_pc();
            self.tick_t_cycle();
        }
    }

    pub(super) fn do_call(&mut self) {
        let addr = self.imm16();
        let pc = self.reg.pc;
        self.internal_push(pc);
        self.reg.pc = addr;
    }

    pub(super) fn call_nn(&mut self) {
        self.do_call();
    }

    pub(super) fn call_f_nn(&mut self, condition: JumpCondition) {
        if condition.check(self) {
            self.do_call();
        } else {
            let pc = self.reg.pc.wrapping_add(2);
            self.reg.pc = pc;
            self.tick_t_cycle();
            self.tick_t_cycle();
        }
    }

    fn do_return(&mut self) {
        let pc = self.internal_pop();
        self.reg.pc = pc;
        self.tick_t_cycle();
    }

    pub(super) fn ret(&mut self) {
        self.do_return();
    }

    pub(super) fn ret_f(&mut self, condition: JumpCondition) {
        self.tick_t_cycle();
        if condition.check(self) {
            self.do_return();
        }
    }

    pub(super) fn reti(&mut self) {
        self.ret();
        self.ei();
    }

    pub(super) fn rst(&mut self, addr: u16) {
        let pc = self.reg.pc;
        self.internal_push(pc);
        self.reg.pc = addr;
    }

    pub(super) fn halt(&mut self) {
        self.cpu_halted = true;

        if self.any_interrrupt() && !self.ime {
            self.cpu_halted = false;
            self.cpu_halt_bug = true;
        }
    }

    pub(super) fn stop(&mut self) {
        self.imm8();

        if self.key1 & KEY1_SWITCH == 0 {
            self.cpu_halted = true;
        } else {
            self.double_speed = !self.double_speed;

            self.key1 &= !KEY1_SWITCH;
            self.key1 ^= KEY1_SPEED;
        }
    }

    pub(super) fn di(&mut self) {
        self.ime = false;
    }

    pub(super) fn ei(&mut self) {
        self.cpu_ei_delay = true;
    }

    pub(super) fn ccf(&mut self) {
        self.reg.set_nf(false);
        self.reg.set_hf(false);
        self.reg.set_cf(!self.reg.cf());
    }

    pub(super) fn scf(&mut self) {
        self.reg.set_nf(false);
        self.reg.set_hf(false);
        self.reg.set_cf(true);
    }

    pub(super) fn nop() {}

    pub(super) fn daa(&mut self) {
        // DAA table in page 110 of the official "Game Boy Programming Manual"
        let mut carry = false;
        if !self.reg.nf() {
            if self.reg.cf() || self.reg.a > 0x99 {
                self.reg.a = self.reg.a.wrapping_add(0x60);
                carry = true;
            }
            if self.reg.hf() || self.reg.a & 0x0f > 0x09 {
                self.reg.a = self.reg.a.wrapping_add(0x06);
            }
        } else if self.reg.cf() {
            carry = true;
            self.reg.a = self
                .reg
                .a
                .wrapping_add(if self.reg.hf() { 0x9a } else { 0xa0 });
        } else if self.reg.hf() {
            self.reg.a = self.reg.a.wrapping_add(0xfa);
        }

        self.reg.set_zf(self.reg.a == 0);
        self.reg.set_hf(false);
        self.reg.set_cf(carry);
    }

    pub(super) fn cpl(&mut self) {
        let a = self.reg.a;
        self.reg.a = !a;
        self.reg.set_nf(true);
        self.reg.set_hf(true);
    }

    pub(super) fn push(&mut self, reg: Reg16) {
        let val = self.reg.read16(reg);
        self.internal_push(val);
    }

    pub(super) fn pop(&mut self, reg: Reg16) {
        let val = self.internal_pop();
        self.reg.write16(reg, val);
    }

    pub(super) fn ld16_nn(&mut self, reg: Reg16) {
        let val = self.imm16();
        self.reg.write16(reg, val);
    }

    pub(super) fn ld16_nn_sp(&mut self) {
        let val = self.reg.sp;
        let addr = self.imm16();
        self.write_mem(addr, (val & 0xff) as u8);
        self.write_mem(addr.wrapping_add(1), (val >> 8) as u8);
    }

    pub(super) fn add16_sp_dd(&mut self) {
        let offset = self.imm8() as i8 as u16;
        let sp = self.reg.sp;
        let val = sp.wrapping_add(offset);
        let tmp = sp ^ val ^ offset;
        self.reg.sp = val;
        self.reg.set_zf(false);
        self.reg.set_nf(false);
        self.reg.set_hf((tmp & 0x10) == 0x10);
        self.reg.set_cf((tmp & 0x100) == 0x100);
        self.tick_t_cycle();
        self.tick_t_cycle();
    }

    pub(super) fn add_hl(&mut self, reg: Reg16) {
        let hl = self.reg.read16(HL);
        let val = self.reg.read16(reg);
        let res = hl.wrapping_add(val);
        self.reg.write16(HL, res);

        self.reg.set_nf(false);

        // check carry from bit 11
        let mask = 0x7FF;
        let half_carry = (hl & mask) + (val & mask) > mask;

        self.reg.set_hf(half_carry);
        self.reg.set_cf(hl > 0xffff - val);

        self.tick_t_cycle();
    }

    pub(super) fn rlc<GS>(&mut self, rhs: GS)
    where
        GS: Get<u8> + Set<u8>,
    {
        let read = rhs.get(self);
        let val = self.internal_rlc(read);
        rhs.set(self, val);
    }

    pub(super) fn rl<GS>(&mut self, rhs: GS)
    where
        GS: Get<u8> + Set<u8>,
    {
        let read = rhs.get(self);
        let val = self.internal_rl(read);
        rhs.set(self, val);
    }

    pub(super) fn rrc<GS>(&mut self, rhs: GS)
    where
        GS: Get<u8> + Set<u8>,
    {
        let read = rhs.get(self);
        let val = self.internal_rrc(read);
        rhs.set(self, val);
    }

    pub(super) fn rr<GS>(&mut self, rhs: GS)
    where
        GS: Get<u8> + Set<u8>,
    {
        let read = rhs.get(self);
        let val = self.internal_rr(read);
        rhs.set(self, val);
    }

    pub(super) fn sla<GS>(&mut self, rhs: GS)
    where
        GS: Get<u8> + Set<u8>,
    {
        let read = rhs.get(self);
        let val = read << 1;
        let co = read & 0x80;
        self.reg.set_zf(val == 0);
        self.reg.set_nf(false);
        self.reg.set_hf(false);
        self.reg.set_cf(co != 0);
        rhs.set(self, val);
    }

    pub(super) fn sra<GS>(&mut self, rhs: GS)
    where
        GS: Get<u8> + Set<u8>,
    {
        let read = rhs.get(self);
        let hi = read & 0x80;
        let val = (read >> 1) | hi;
        let co = read & 0x01;
        self.reg.set_zf(val == 0);
        self.reg.set_nf(false);
        self.reg.set_hf(false);
        self.reg.set_cf(co != 0);

        rhs.set(self, val);
    }

    pub(super) fn srl<GS>(&mut self, rhs: GS)
    where
        GS: Get<u8> + Set<u8>,
    {
        let read = rhs.get(self);
        let val = read >> 1;
        let co = read & 0x01;
        self.reg.set_zf(val == 0);
        self.reg.set_nf(false);
        self.reg.set_hf(false);
        self.reg.set_cf(co != 0);
        rhs.set(self, val);
    }

    pub(super) fn swap<GS>(&mut self, rhs: GS)
    where
        GS: Get<u8> + Set<u8>,
    {
        let read = rhs.get(self);
        let val = (read >> 4) | (read << 4);
        self.reg.set_zf(read == 0);
        self.reg.set_nf(false);
        self.reg.set_hf(false);
        self.reg.set_cf(false);
        rhs.set(self, val);
    }

    pub(super) fn bit<G>(&mut self, rhs: G, bit: usize)
    where
        G: Get<u8>,
    {
        let read = rhs.get(self);
        let val = read & (1 << bit);
        self.reg.set_zf(val == 0);
        self.reg.set_nf(false);
        self.reg.set_hf(true);
    }

    pub(super) fn set<GS>(&mut self, rhs: GS, bit: usize)
    where
        GS: Get<u8> + Set<u8>,
    {
        let read = rhs.get(self);
        let val = read | (1 << bit);
        rhs.set(self, val);
    }

    pub(super) fn res<GS>(&mut self, rhs: GS, bit: usize)
    where
        GS: Get<u8> + Set<u8>,
    {
        let read = rhs.get(self);
        let val = read & !(1 << bit);
        rhs.set(self, val);
    }
}
