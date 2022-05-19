use {
    super::{
        operands::{Get, Set},
        registers::{
            Reg16::{self, HL},
            CF_FLAG, NF_FLAG, ZF_FLAG,
        },
    },
    crate::{Gb, KEY1_SPEED_B, KEY1_SWITCH_B},
};

impl Gb {
    pub(super) fn ld<G, S>(&mut self, lhs: S, rhs: G)
    where
        G: Get,
        S: Set,
    {
        let val = rhs.get(self);
        lhs.set(self, val);
    }

    pub(super) fn ld_dhli_a(&mut self) {
        let addr = self.rreg16(Reg16::HL);
        self.write_mem(addr, self.a());
        self.wreg16(Reg16::HL, addr.wrapping_add(1));
    }

    pub(super) fn ld_dhld_a(&mut self) {
        let addr = self.rreg16(Reg16::HL);
        self.write_mem(addr, self.a());
        self.wreg16(Reg16::HL, addr.wrapping_sub(1));
    }

    pub(super) fn ld_a_dhli(&mut self) {
        let addr = self.rreg16(Reg16::HL);
        let val = self.read_mem(addr);
        self.set_a(val);
        self.wreg16(Reg16::HL, addr.wrapping_add(1));
    }

    pub(super) fn ld_a_dhld(&mut self) {
        let addr = self.rreg16(Reg16::HL);
        let val = self.read_mem(addr);
        self.set_a(val);
        self.wreg16(Reg16::HL, addr.wrapping_sub(1));
    }

    pub(super) fn ld16_sp_hl(&mut self) {
        let val = self.rreg16(HL);
        self.sp = val;
        self.tick();
    }

    pub(super) fn add<G>(&mut self, rhs: G)
    where
        G: Get,
    {
        let a = self.a();
        let val = rhs.get(self);
        let (output, carry) = a.overflowing_add(val);
        self.set_zf(output == 0);
        self.set_nf(false);
        self.set_hf((a & 0xf) + (val & 0xf) > 0xf);
        self.set_cf(carry);
        self.set_a(output);
    }

    pub(super) fn adc<G>(&mut self, rhs: G)
    where
        G: Get,
    {
        let a = self.a();
        let cy = u8::from(self.cf());
        let val = rhs.get(self);
        let output = a.wrapping_add(val).wrapping_add(cy);
        self.set_zf(output == 0);
        self.set_nf(false);
        self.set_hf((a & 0xf) + (val & 0xf) + cy > 0xf);
        self.set_cf(u16::from(a) + u16::from(val) + u16::from(cy) > 0xff);
        self.set_a(output);
    }

    pub(super) fn sub<G>(&mut self, rhs: G)
    where
        G: Get,
    {
        let a = self.a();
        let val = rhs.get(self);
        let (output, carry) = a.overflowing_sub(val);
        self.set_zf(output == 0);
        self.set_nf(true);
        self.set_hf(a & 0xf < val & 0xf);
        self.set_cf(carry);
        self.set_a(output);
    }

    pub(super) fn sbc<G>(&mut self, rhs: G)
    where
        G: Get,
    {
        let a = self.a();
        let cy = u8::from(self.cf());
        let val = rhs.get(self);
        let output = a.wrapping_sub(val).wrapping_sub(cy);
        self.set_zf(output == 0);
        self.set_nf(true);
        self.set_hf(a & 0xf < (val & 0xf) + cy);
        self.set_cf(u16::from(a) < u16::from(val) + u16::from(cy));
        self.set_a(output);
    }

    pub(super) fn cp<G>(&mut self, rhs: G)
    where
        G: Get,
    {
        let a = self.a();
        let val = rhs.get(self);
        let result = a.wrapping_sub(val);
        self.set_zf(result == 0);
        self.set_nf(true);
        self.set_hf((a & 0xf).wrapping_sub(val & 0xf) & (0xf + 1) != 0);
        self.set_cf(a < val);
    }

    pub(super) fn and<G>(&mut self, rhs: G)
    where
        G: Get,
    {
        let val = rhs.get(self);
        let a = self.a() & val;
        self.set_a(a);
        self.set_zf(a == 0);
        self.set_nf(false);
        self.set_hf(true);
        self.set_cf(false);
    }

    pub(super) fn or<G>(&mut self, rhs: G)
    where
        G: Get,
    {
        let val = rhs.get(self);
        let a = self.a() | val;
        self.set_a(a);
        self.set_zf(a == 0);
        self.set_nf(false);
        self.set_hf(false);
        self.set_cf(false);
    }

    pub(super) fn xor<G>(&mut self, rhs: G)
    where
        G: Get,
    {
        let read = rhs.get(self);
        let a = self.a() ^ read;
        self.set_a(a);
        self.set_zf(a == 0);
        self.set_nf(false);
        self.set_hf(false);
        self.set_cf(false);
    }

    pub(super) fn inc<GS>(&mut self, rhs: GS)
    where
        GS: Get + Set,
    {
        let read = rhs.get(self);
        let val = read.wrapping_add(1);
        self.set_zf(val == 0);
        self.set_nf(false);
        self.set_hf(read & 0xf == 0xf);
        rhs.set(self, val);
    }

    pub(super) fn inc16(&mut self, reg: Reg16) {
        let read = self.rreg16(reg);
        let val = read.wrapping_add(1);
        self.wreg16(reg, val);
        self.tick();
    }

    pub(super) fn dec<GS>(&mut self, rhs: GS)
    where
        GS: Get + Set,
    {
        let read = rhs.get(self);
        let val = read.wrapping_sub(1);
        self.set_zf(val == 0);
        self.set_nf(true);
        self.set_hf(read.trailing_zeros() >= 4);
        rhs.set(self, val);
    }

    pub(super) fn dec16(&mut self, reg: Reg16) {
        let read = self.rreg16(reg);
        let val = read.wrapping_sub(1);
        self.wreg16(reg, val);
        self.tick();
    }

    pub(super) fn ld16_hl_sp_dd(&mut self) {
        let offset = self.imm8() as i8 as u16;
        let sp = self.sp;
        let val = sp.wrapping_add(offset);
        let tmp = sp ^ val ^ offset;
        self.wreg16(HL, val);
        self.set_zf(false);
        self.set_nf(false);
        self.set_hf((tmp & 0x10) == 0x10);
        self.set_cf((tmp & 0x100) == 0x100);
        self.tick();
    }

    pub(super) fn rlca(&mut self) {
        let val = self.internal_rlc(self.a());
        self.set_a(val);
        self.set_zf(false);
    }

    pub(super) fn rla(&mut self) {
        let val = self.internal_rl(self.a());
        self.set_a(val);
        self.set_zf(false);
    }

    pub(super) fn rrca(&mut self) {
        let val = self.internal_rrc(self.a());
        self.set_a(val);
        self.set_zf(false);
    }

    pub(super) fn rra(&mut self) {
        let val = self.internal_rr(self.a());
        self.set_a(val);
        self.set_zf(false);
    }

    pub(super) fn do_jump_to_immediate(&mut self) {
        let addr = self.imm16();
        self.pc = addr;
        self.tick();
    }

    pub(super) fn jp_nn(&mut self) {
        self.do_jump_to_immediate();
    }

    pub(super) fn jp_f(&mut self, opcode: u8) {
        if self.condition(opcode) {
            self.do_jump_to_immediate();
        } else {
            let pc = self.pc.wrapping_add(2);
            self.pc = pc;
            self.tick();
            self.tick();
        }
    }

    pub(super) fn jp_hl(&mut self) {
        let addr = self.rreg16(HL);
        self.pc = addr;
    }

    fn do_jump_relative(&mut self) {
        let relative_addr = self.imm8() as i8 as u16;
        let pc = self.pc.wrapping_add(relative_addr);
        self.pc = pc;
        self.tick();
    }

    pub(super) fn jr_d(&mut self) {
        self.do_jump_relative();
    }

    pub(super) fn jr_f(&mut self, opcode: u8) {
        if self.condition(opcode) {
            self.do_jump_relative();
        } else {
            self.inc_pc();
            self.tick();
        }
    }

    pub(super) fn do_call(&mut self) {
        let addr = self.imm16();
        let pc = self.pc;
        self.internal_push(pc);
        self.pc = addr;
    }

    pub(super) fn call_nn(&mut self) {
        self.do_call();
    }

    pub(super) fn call_f_nn(&mut self, opcode: u8) {
        if self.condition(opcode) {
            self.do_call();
        } else {
            let pc = self.pc.wrapping_add(2);
            self.pc = pc;
            self.tick();
            self.tick();
        }
    }

    fn do_return(&mut self) {
        let pc = self.internal_pop();
        self.pc = pc;
        self.tick();
    }

    pub(super) fn ret(&mut self) {
        self.do_return();
    }

    pub(super) fn ret_f(&mut self, opcode: u8) {
        self.tick();
        if self.condition(opcode) {
            self.do_return();
        }
    }

    fn condition(&self, opcode: u8) -> bool {
        match (opcode >> 3) & 0x3 {
            0 => self.af & ZF_FLAG == 0,
            1 => self.af & ZF_FLAG != 0,
            2 => self.af & CF_FLAG == 0,
            3 => self.af & CF_FLAG != 0,
            _ => unreachable!(),
        }
    }

    pub(super) fn reti(&mut self) {
        self.ret();
        self.ei();
    }

    pub(super) fn rst(&mut self, addr: u16) {
        let pc = self.pc;
        self.internal_push(pc);
        self.pc = addr;
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

        if self.key1 & KEY1_SWITCH_B == 0 {
            self.cpu_halted = true;
        } else {
            self.double_speed = !self.double_speed;

            self.key1 &= !KEY1_SWITCH_B;
            self.key1 ^= KEY1_SPEED_B;
        }
    }

    pub(super) fn di(&mut self) {
        self.ime = false;
    }

    pub(super) fn ei(&mut self) {
        self.cpu_ei_delay = true;
    }

    pub(super) fn ccf(&mut self) {
        self.set_nf(false);
        self.set_hf(false);
        self.set_cf(!self.cf());
    }

    pub(super) fn scf(&mut self) {
        self.set_nf(false);
        self.set_hf(false);
        self.set_cf(true);
    }

    pub(super) fn nop() {}

    pub(super) fn daa(&mut self) {
        // DAA table in page 110 of the official "Game Boy Programming Manual"
        let mut carry = false;
        if self.af & NF_FLAG == 0 {
            if self.cf() || self.a() > 0x99 {
                self.set_a(self.a().wrapping_add(0x60));
                carry = true;
            }
            if self.hf() || self.a() & 0x0f > 0x09 {
                self.set_a(self.a().wrapping_add(0x06));
            }
        } else if self.cf() {
            carry = true;
            self.set_a(self.a().wrapping_add(if self.hf() { 0x9a } else { 0xa0 }));
        } else if self.hf() {
            self.set_a(self.a().wrapping_add(0xfa));
        }

        self.set_zf(self.a() == 0);
        self.set_hf(false);
        self.set_cf(carry);
    }

    pub(super) fn cpl(&mut self) {
        let a = self.a();
        self.set_a(!a);
        self.set_nf(true);
        self.set_hf(true);
    }

    pub(super) fn push(&mut self, reg: Reg16) {
        let val = self.rreg16(reg);
        self.internal_push(val);
    }

    pub(super) fn pop(&mut self, reg: Reg16) {
        let val = self.internal_pop();
        self.wreg16(reg, val);
    }

    pub(super) fn ld16_nn(&mut self, reg: Reg16) {
        let val = self.imm16();
        self.wreg16(reg, val);
    }

    pub(super) fn ld16_nn_sp(&mut self) {
        let val = self.sp;
        let addr = self.imm16();
        self.write_mem(addr, (val & 0xff) as u8);
        self.write_mem(addr.wrapping_add(1), (val >> 8) as u8);
    }

    pub(super) fn add16_sp_dd(&mut self) {
        let offset = self.imm8() as i8 as u16;
        let sp = self.sp;
        let val = sp.wrapping_add(offset);
        let tmp = sp ^ val ^ offset;
        self.sp = val;
        self.set_zf(false);
        self.set_nf(false);
        self.set_hf((tmp & 0x10) == 0x10);
        self.set_cf((tmp & 0x100) == 0x100);
        self.tick();
        self.tick();
    }

    pub(super) fn add_hl(&mut self, reg: Reg16) {
        let hl = self.rreg16(HL);
        let val = self.rreg16(reg);
        let res = hl.wrapping_add(val);
        self.wreg16(HL, res);

        self.set_nf(false);

        // check carry from bit 11
        let mask = 0x7FF;
        let half_carry = (hl & mask) + (val & mask) > mask;

        self.set_hf(half_carry);
        self.set_cf(hl > 0xffff - val);

        self.tick();
    }

    pub(super) fn rlc<GS>(&mut self, rhs: GS)
    where
        GS: Get + Set,
    {
        let read = rhs.get(self);
        let val = self.internal_rlc(read);
        rhs.set(self, val);
    }

    pub(super) fn rl<GS>(&mut self, rhs: GS)
    where
        GS: Get + Set,
    {
        let read = rhs.get(self);
        let val = self.internal_rl(read);
        rhs.set(self, val);
    }

    pub(super) fn rrc<GS>(&mut self, rhs: GS)
    where
        GS: Get + Set,
    {
        let read = rhs.get(self);
        let val = self.internal_rrc(read);
        rhs.set(self, val);
    }

    pub(super) fn rr<GS>(&mut self, rhs: GS)
    where
        GS: Get + Set,
    {
        let read = rhs.get(self);
        let val = self.internal_rr(read);
        rhs.set(self, val);
    }

    pub(super) fn sla<GS>(&mut self, rhs: GS)
    where
        GS: Get + Set,
    {
        let read = rhs.get(self);
        let val = read << 1;
        let co = read & 0x80;
        self.set_zf(val == 0);
        self.set_nf(false);
        self.set_hf(false);
        self.set_cf(co != 0);
        rhs.set(self, val);
    }

    pub(super) fn sra<GS>(&mut self, rhs: GS)
    where
        GS: Get + Set,
    {
        let read = rhs.get(self);
        let hi = read & 0x80;
        let val = (read >> 1) | hi;
        let co = read & 0x01;
        self.set_zf(val == 0);
        self.set_nf(false);
        self.set_hf(false);
        self.set_cf(co != 0);

        rhs.set(self, val);
    }

    pub(super) fn srl<GS>(&mut self, rhs: GS)
    where
        GS: Get + Set,
    {
        let read = rhs.get(self);
        let val = read >> 1;
        let co = read & 0x01;
        self.set_zf(val == 0);
        self.set_nf(false);
        self.set_hf(false);
        self.set_cf(co != 0);
        rhs.set(self, val);
    }

    pub(super) fn swap<GS>(&mut self, rhs: GS)
    where
        GS: Get + Set,
    {
        let read = rhs.get(self);
        let val = (read >> 4) | (read << 4);
        self.set_zf(read == 0);
        self.set_nf(false);
        self.set_hf(false);
        self.set_cf(false);
        rhs.set(self, val);
    }

    pub(super) fn bit<G>(&mut self, rhs: G, bit: usize)
    where
        G: Get,
    {
        let read = rhs.get(self);
        let val = read & (1 << bit);
        self.set_zf(val == 0);
        self.set_nf(false);
        self.set_hf(true);
    }

    pub(super) fn set<GS>(&mut self, rhs: GS, bit: usize)
    where
        GS: Get + Set,
    {
        let read = rhs.get(self);
        let val = read | (1 << bit);
        rhs.set(self, val);
    }

    pub(super) fn res<GS>(&mut self, rhs: GS, bit: usize)
    where
        GS: Get + Set,
    {
        let read = rhs.get(self);
        let val = read & !(1 << bit);
        rhs.set(self, val);
    }
}
