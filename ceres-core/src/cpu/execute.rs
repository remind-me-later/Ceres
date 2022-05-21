use {
    super::{
        operands::{Get, Imm, Indirect, Set},
        registers::Reg8::{A, B, C, D, E, H, L},
        CF_B, HF_B, NF_B, ZF_B,
    },
    crate::{Gb, KEY1_SPEED_B, KEY1_SWITCH_B},
};

impl Gb {
    fn get_src_value(&mut self, opcode: u8) -> u8 {
        let reg_id = ((opcode >> 1) + 1) & 3;
        let lo = opcode & 1 != 0;
        if reg_id == 0 {
            if lo {
                return (self.af >> 8) as u8;
            }
            return self.read_mem(self.hl);
        }
        if lo {
            return *self.rid16(reg_id) as u8;
        }
        return (*self.rid16(reg_id) >> 8) as u8;
    }

    fn set_src_value(&mut self, opcode: u8, value: u8) {
        let reg_id = ((opcode >> 1) + 1) & 3;
        let lo = opcode & 1 != 0;

        if reg_id == 0 {
            if lo {
                self.af &= 0xFF;
                self.af |= (value as u16) << 8;
            } else {
                self.write_mem(self.hl, value);
            }
        } else if lo {
            *self.rid16(reg_id) &= 0xFF00;
            *self.rid16(reg_id) |= value as u16;
        } else {
            *self.rid16(reg_id) &= 0xFF;
            *self.rid16(reg_id) |= (value as u16) << 8;
        }
    }

    fn ld<G, S>(&mut self, lhs: S, rhs: G)
    where
        G: Get,
        S: Set,
    {
        let val = rhs.get(self);
        lhs.set(self, val);
    }

    fn ld_dhli_a(&mut self) {
        let addr = self.hl;
        self.write_mem(addr, self.a());
        self.hl = addr.wrapping_add(1);
    }

    fn ld_dhld_a(&mut self) {
        let addr = self.hl;
        self.write_mem(addr, self.a());
        self.hl = addr.wrapping_sub(1);
    }

    fn ld_a_dhli(&mut self) {
        let addr = self.hl;
        let val = self.read_mem(addr);
        self.set_a(val);
        self.hl = addr.wrapping_add(1);
    }

    fn ld_a_dhld(&mut self) {
        let addr = self.hl;
        let val = self.read_mem(addr);
        self.set_a(val);
        self.hl = addr.wrapping_sub(1);
    }

    fn ld16_sp_hl(&mut self) {
        let val = self.hl;
        self.sp = val;
        self.tick();
    }

    fn add<G>(&mut self, rhs: G)
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

    fn adc<G>(&mut self, rhs: G)
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

    fn sub<G>(&mut self, rhs: G)
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

    fn sbc<G>(&mut self, rhs: G)
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

    fn cp<G>(&mut self, rhs: G)
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

    fn and<G>(&mut self, rhs: G)
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

    fn or<G>(&mut self, rhs: G)
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

    fn xor<G>(&mut self, rhs: G)
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

    fn inc<GS>(&mut self, rhs: GS)
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

    fn inc16(&mut self, opcode: u8) {
        let register_id = (opcode >> 4) + 1;
        let reg = self.rid16(register_id);
        *reg = reg.wrapping_add(1);
        self.tick();
    }

    fn dec<GS>(&mut self, rhs: GS)
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

    fn dec16(&mut self, opcode: u8) {
        let register_id = (opcode >> 4) + 1;
        let reg = self.rid16(register_id);
        *reg = reg.wrapping_sub(1);
        self.tick();
    }

    fn ld16_hl_sp_dd(&mut self) {
        let offset = self.imm8() as i8 as u16;
        let sp = self.sp;
        let val = sp.wrapping_add(offset);
        let tmp = sp ^ val ^ offset;
        self.hl = val;
        self.set_zf(false);
        self.set_nf(false);
        self.set_hf((tmp & 0x10) == 0x10);
        self.set_cf((tmp & 0x100) == 0x100);
        self.tick();
    }

    fn rlca(&mut self) {
        let carry = (self.af & 0x8000) != 0;

        self.af = (self.af & 0xFF00) << 1;
        if carry {
            self.af |= CF_B | 0x0100;
        }
    }

    fn rrca(&mut self) {
        let carry = (self.af & 0x100) != 0;
        self.af = (self.af >> 1) & 0xFF00;
        if carry {
            self.af |= CF_B | 0x8000;
        }
    }

    fn rrc_r(&mut self, opcode: u8) {
        let value = self.get_src_value(opcode);
        let carry = (value & 0x01) != 0;
        self.af &= 0xFF00;
        let val = value >> 1 | (carry as u8) << 7;
        self.set_src_value(opcode, val);
        if carry {
            self.af |= CF_B;
        }
        if val == 0 {
            self.af |= ZF_B;
        }
    }

    fn do_jump_to_immediate(&mut self) {
        let addr = self.imm16();
        self.pc = addr;
        self.tick();
    }

    fn jp_nn(&mut self) {
        self.do_jump_to_immediate();
    }

    fn jp_f(&mut self, opcode: u8) {
        if self.condition(opcode) {
            self.do_jump_to_immediate();
        } else {
            let pc = self.pc.wrapping_add(2);
            self.pc = pc;
            self.tick();
            self.tick();
        }
    }

    fn jp_hl(&mut self) {
        let addr = self.hl;
        self.pc = addr;
    }

    fn do_jump_relative(&mut self) {
        let relative_addr = self.imm8() as i8 as u16;
        let pc = self.pc.wrapping_add(relative_addr);
        self.pc = pc;
        self.tick();
    }

    fn jr_d(&mut self) {
        self.do_jump_relative();
    }

    fn jr_f(&mut self, opcode: u8) {
        if self.condition(opcode) {
            self.do_jump_relative();
        } else {
            self.pc = self.pc.wrapping_add(1);
            self.tick();
        }
    }

    fn do_call(&mut self) {
        let addr = self.imm16();
        let pc = self.pc;
        self.internal_push(pc);
        self.pc = addr;
    }

    fn call_nn(&mut self) {
        self.do_call();
    }

    fn call_f_nn(&mut self, opcode: u8) {
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

    fn ret(&mut self) {
        self.do_return();
    }

    fn ret_f(&mut self, opcode: u8) {
        self.tick();
        if self.condition(opcode) {
            self.do_return();
        }
    }

    fn condition(&self, opcode: u8) -> bool {
        match (opcode >> 3) & 0x3 {
            0 => self.af & ZF_B == 0,
            1 => self.af & ZF_B != 0,
            2 => self.af & CF_B == 0,
            3 => self.af & CF_B != 0,
            _ => unreachable!(),
        }
    }

    fn reti(&mut self) {
        self.ret();
        self.ei();
    }

    fn rst(&mut self, addr: u16) {
        let pc = self.pc;
        self.internal_push(pc);
        self.pc = addr;
    }

    fn halt(&mut self) {
        self.cpu_halted = true;

        if self.any_interrrupt() && !self.ime {
            self.cpu_halted = false;
            self.cpu_halt_bug = true;
        }
    }

    fn stop(&mut self) {
        self.imm8();

        if self.key1 & KEY1_SWITCH_B == 0 {
            self.cpu_halted = true;
        } else {
            self.double_speed = !self.double_speed;

            self.key1 &= !KEY1_SWITCH_B;
            self.key1 ^= KEY1_SPEED_B;
        }
    }

    fn di(&mut self) {
        self.ime = false;
    }

    fn ei(&mut self) {
        self.cpu_ei_delay = true;
    }

    fn ccf(&mut self) {
        self.set_nf(false);
        self.set_hf(false);
        self.set_cf(!self.cf());
    }

    fn scf(&mut self) {
        self.set_nf(false);
        self.set_hf(false);
        self.set_cf(true);
    }

    fn nop() {}

    fn daa(&mut self) {
        let mut result = self.af >> 8;

        self.af &= !(0xFF00 | ZF_B);

        if self.af & NF_B == 0 {
            if self.af & HF_B != 0 || (result & 0x0F) > 0x09 {
                result += 0x06;
            }

            if self.af & CF_B != 0 || result > 0x9F {
                result += 0x60;
            }
        } else {
            if self.af & HF_B != 0 {
                result = result.wrapping_sub(0x06) & 0xFF;
            }

            if self.af & CF_B != 0 {
                result = result.wrapping_sub(0x60);
            }
        }

        if (result & 0xFF) == 0 {
            self.af |= ZF_B;
        }

        if (result & 0x100) == 0x100 {
            self.af |= CF_B;
        }

        self.af &= !HF_B;
        self.af |= result << 8;
    }

    fn cpl(&mut self) {
        let a = self.a();
        self.set_a(!a);
        self.set_nf(true);
        self.set_hf(true);
    }

    fn push(&mut self, opcode: u8) {
        let register_id = ((opcode >> 4) + 1) & 3;
        let val = *self.rid16(register_id);
        self.internal_push(val);
    }

    fn pop(&mut self, opcode: u8) {
        let val = self.internal_pop();
        let register_id = ((opcode >> 4) + 1) & 3;
        *self.rid16(register_id) = val;
        self.af &= 0xfff0;
    }

    fn ld16_nn(&mut self, opcode: u8) {
        let register_id = (opcode >> 4) + 1;
        let val = self.imm16();
        *self.rid16(register_id) = val;
    }

    fn ld16_nn_sp(&mut self) {
        let val = self.sp;
        let addr = self.imm16();
        self.write_mem(addr, (val & 0xff) as u8);
        self.write_mem(addr.wrapping_add(1), (val >> 8) as u8);
    }

    fn add16_sp_dd(&mut self) {
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

    fn add_hl(&mut self, opcode: u8) {
        let register_id = (opcode >> 4) + 1;
        let hl = self.hl;
        let val = *self.rid16(register_id);
        let res = hl.wrapping_add(val);
        self.hl = res;

        self.set_nf(false);

        // check carry from bit 11
        let mask = 0x7FF;
        let half_carry = (hl & mask) + (val & mask) > mask;

        self.set_hf(half_carry);
        self.set_cf(hl > 0xffff - val);

        self.tick();
    }

    fn rlc_r(&mut self, opcode: u8) {
        let value = self.get_src_value(opcode);
        let carry = (value & 0x80) != 0;
        self.af &= 0xFF00;
        self.set_src_value(opcode, (value << 1) | (carry as u8));
        if carry {
            self.af |= CF_B;
        }
        if value == 0 {
            self.af |= ZF_B;
        }
    }

    fn rra(&mut self) {
        let bit1 = (self.af & 0x0100) != 0;
        let carry = (self.af & CF_B) != 0;

        self.af = (self.af >> 1) & 0xFF00;
        if carry {
            self.af |= 0x8000;
        }
        if bit1 {
            self.af |= CF_B;
        }
    }

    fn rr_r(&mut self, opcode: u8) {
        let value = self.get_src_value(opcode);
        let carry = (self.af & CF_B) != 0;
        let bit1 = (value & 0x1) != 0;

        self.af &= 0xFF00;
        let value = (value >> 1) | ((carry as u8) << 7);
        self.set_src_value(opcode, value);
        if bit1 {
            self.af |= CF_B;
        }
        if value == 0 {
            self.af |= ZF_B;
        }
    }

    fn sla_r(&mut self, opcode: u8) {
        let value = self.get_src_value(opcode);
        let carry = (value & 0x80) != 0;
        self.af &= 0xFF00;
        self.set_src_value(opcode, value << 1);
        if carry {
            self.af |= CF_B;
        }
        if value & 0x7F == 0 {
            self.af |= ZF_B;
        }
    }

    fn sra_r(&mut self, opcode: u8) {
        let value = self.get_src_value(opcode);
        let bit7 = value & 0x80;
        self.af &= 0xFF00;
        if value & 1 != 0 {
            self.af |= CF_B;
        }
        let value = (value >> 1) | bit7;
        self.set_src_value(opcode, value);
        if value == 0 {
            self.af |= ZF_B;
        }
    }

    fn srl_r(&mut self, opcode: u8) {
        let value = self.get_src_value(opcode);
        self.af &= 0xFF00;
        self.set_src_value(opcode, value >> 1);
        if value & 1 != 0 {
            self.af |= CF_B;
        }
        if value >> 1 == 0 {
            self.af |= ZF_B;
        }
    }

    fn swap_r(&mut self, opcode: u8) {
        let value = self.get_src_value(opcode);
        self.af &= 0xFF00;
        self.set_src_value(opcode, (value >> 4) | (value << 4));
        if value == 0 {
            self.af |= ZF_B;
        }
    }

    fn bit_r(&mut self, opcode: u8) {
        let value = self.get_src_value(opcode);
        let bit = 1 << ((opcode >> 3) & 7);
        if opcode & 0xC0 == 0x40 {
            /* Bit */
            self.af &= 0xFF00 | CF_B;
            self.af |= HF_B;
            if bit & value == 0 {
                self.af |= ZF_B;
            }
        } else if opcode & 0xC0 == 0x80 {
            /* res */
            self.set_src_value(opcode, value & !bit);
        } else if opcode & 0xC0 == 0xC0 {
            /* set */
            self.set_src_value(opcode, value | bit);
        }
    }

    fn rla(&mut self) {
        let bit7 = (self.af & 0x8000) != 0;
        let carry = (self.af & CF_B) != 0;

        self.af = (self.af & 0xFF00) << 1;
        if carry {
            self.af |= 0x0100;
        }
        if bit7 {
            self.af |= CF_B;
        }
    }

    fn rl_r(&mut self, opcode: u8) {
        let value = self.get_src_value(opcode);
        let carry = (self.af & CF_B) != 0;
        let bit7 = (value & 0x80) != 0;

        self.af &= 0xFF00;
        let value = (value << 1) | carry as u8;
        self.set_src_value(opcode, value);
        if bit7 {
            self.af |= CF_B;
        }
        if value == 0 {
            self.af |= ZF_B;
        }
    }

    /// # Panics
    ///
    /// will panic on illegal opcode
    pub(super) fn exec(&mut self, opcode: u8) {
        match opcode {
            0x7f => self.ld(A, A),
            0x78 => self.ld(A, B),
            0x79 => self.ld(A, C),
            0x7a => self.ld(A, D),
            0x7b => self.ld(A, E),
            0x7c => self.ld(A, H),
            0x7d => self.ld(A, L),
            0x3e => self.ld(A, Imm),
            0x7e => self.ld(A, Indirect::HL),
            0x47 => self.ld(B, A),
            0x40 => {
                // TODO: debugger
                self.ld(B, B);
            }
            0x41 => self.ld(B, C),
            0x42 => self.ld(B, D),
            0x43 => self.ld(B, E),
            0x44 => self.ld(B, H),
            0x45 => self.ld(B, L),
            0x06 => self.ld(B, Imm),
            0x46 => self.ld(B, Indirect::HL),
            0x4f => self.ld(C, A),
            0x48 => self.ld(C, B),
            0x49 => self.ld(C, C),
            0x4a => self.ld(C, D),
            0x4b => self.ld(C, E),
            0x4c => self.ld(C, H),
            0x4d => self.ld(C, L),
            0x0e => self.ld(C, Imm),
            0x4e => self.ld(C, Indirect::HL),
            0x57 => self.ld(D, A),
            0x50 => self.ld(D, B),
            0x51 => self.ld(D, C),
            0x52 => self.ld(D, D),
            0x53 => self.ld(D, E),
            0x54 => self.ld(D, H),
            0x55 => self.ld(D, L),
            0x16 => self.ld(D, Imm),
            0x56 => self.ld(D, Indirect::HL),
            0x5f => self.ld(E, A),
            0x58 => self.ld(E, B),
            0x59 => self.ld(E, C),
            0x5a => self.ld(E, D),
            0x5b => self.ld(E, E),
            0x5c => self.ld(E, H),
            0x5d => self.ld(E, L),
            0x1e => self.ld(E, Imm),
            0x5e => self.ld(E, Indirect::HL),
            0x67 => self.ld(H, A),
            0x60 => self.ld(H, B),
            0x61 => self.ld(H, C),
            0x62 => self.ld(H, D),
            0x63 => self.ld(H, E),
            0x64 => self.ld(H, H),
            0x65 => self.ld(H, L),
            0x26 => self.ld(H, Imm),
            0x66 => self.ld(H, Indirect::HL),
            0x6f => self.ld(L, A),
            0x68 => self.ld(L, B),
            0x69 => self.ld(L, C),
            0x6a => self.ld(L, D),
            0x6b => self.ld(L, E),
            0x6c => self.ld(L, H),
            0x6d => self.ld(L, L),
            0x2e => self.ld(L, Imm),
            0x6e => self.ld(L, Indirect::HL),
            0x77 => self.ld(Indirect::HL, A),
            0x70 => self.ld(Indirect::HL, B),
            0x71 => self.ld(Indirect::HL, C),
            0x72 => self.ld(Indirect::HL, D),
            0x73 => self.ld(Indirect::HL, E),
            0x74 => self.ld(Indirect::HL, H),
            0x75 => self.ld(Indirect::HL, L),
            0x36 => self.ld(Indirect::HL, Imm),
            0x0a => self.ld(A, Indirect::BC),
            0x02 => self.ld(Indirect::BC, A),
            0x1a => self.ld(A, Indirect::DE),
            0x12 => self.ld(Indirect::DE, A),
            0xfa => self.ld(A, Indirect::Immediate),
            0xea => self.ld(Indirect::Immediate, A),
            0x3a => self.ld_a_dhld(),
            0x32 => self.ld_dhld_a(),
            0x2a => self.ld_a_dhli(),
            0x22 => self.ld_dhli_a(),
            0xf2 => self.ld(A, Indirect::HighC),
            0xe2 => self.ld(Indirect::HighC, A),
            0xf0 => self.ld(A, Indirect::HighImmediate),
            0xe0 => self.ld(Indirect::HighImmediate, A),
            0x87 => self.add(A),
            0x80 => self.add(B),
            0x81 => self.add(C),
            0x82 => self.add(D),
            0x83 => self.add(E),
            0x84 => self.add(H),
            0x85 => self.add(L),
            0x86 => self.add(Indirect::HL),
            0xc6 => self.add(Imm),
            0x8f => self.adc(A),
            0x88 => self.adc(B),
            0x89 => self.adc(C),
            0x8a => self.adc(D),
            0x8b => self.adc(E),
            0x8c => self.adc(H),
            0x8d => self.adc(L),
            0x8e => self.adc(Indirect::HL),
            0xce => self.adc(Imm),
            0x97 => self.sub(A),
            0x90 => self.sub(B),
            0x91 => self.sub(C),
            0x92 => self.sub(D),
            0x93 => self.sub(E),
            0x94 => self.sub(H),
            0x95 => self.sub(L),
            0x96 => self.sub(Indirect::HL),
            0xd6 => self.sub(Imm),
            0x9f => self.sbc(A),
            0x98 => self.sbc(B),
            0x99 => self.sbc(C),
            0x9a => self.sbc(D),
            0x9b => self.sbc(E),
            0x9c => self.sbc(H),
            0x9d => self.sbc(L),
            0x9e => self.sbc(Indirect::HL),
            0xde => self.sbc(Imm),
            0xbf => self.cp(A),
            0xb8 => self.cp(B),
            0xb9 => self.cp(C),
            0xba => self.cp(D),
            0xbb => self.cp(E),
            0xbc => self.cp(H),
            0xbd => self.cp(L),
            0xbe => self.cp(Indirect::HL),
            0xfe => self.cp(Imm),
            0xa7 => self.and(A),
            0xa0 => self.and(B),
            0xa1 => self.and(C),
            0xa2 => self.and(D),
            0xa3 => self.and(E),
            0xa4 => self.and(H),
            0xa5 => self.and(L),
            0xa6 => self.and(Indirect::HL),
            0xe6 => self.and(Imm),
            0xb7 => self.or(A),
            0xb0 => self.or(B),
            0xb1 => self.or(C),
            0xb2 => self.or(D),
            0xb3 => self.or(E),
            0xb4 => self.or(H),
            0xb5 => self.or(L),
            0xb6 => self.or(Indirect::HL),
            0xf6 => self.or(Imm),
            0xaf => self.xor(A),
            0xa8 => self.xor(B),
            0xa9 => self.xor(C),
            0xaa => self.xor(D),
            0xab => self.xor(E),
            0xac => self.xor(H),
            0xad => self.xor(L),
            0xae => self.xor(Indirect::HL),
            0xee => self.xor(Imm),
            0x3c => self.inc(A),
            0x04 => self.inc(B),
            0x0c => self.inc(C),
            0x14 => self.inc(D),
            0x1c => self.inc(E),
            0x24 => self.inc(H),
            0x2c => self.inc(L),
            0x34 => self.inc(Indirect::HL),
            0x3d => self.dec(A),
            0x05 => self.dec(B),
            0x0d => self.dec(C),
            0x15 => self.dec(D),
            0x1d => self.dec(E),
            0x25 => self.dec(H),
            0x2d => self.dec(L),
            0x35 => self.dec(Indirect::HL),
            0x07 => self.rlca(),
            0x17 => self.rla(),
            0x0f => self.rrca(),
            0x1f => self.rra(),
            0xc3 => self.jp_nn(),
            0xe9 => self.jp_hl(),
            0x18 => self.jr_d(),
            0xcd => self.call_nn(),
            0xc9 => self.ret(),
            0xd9 => self.reti(),
            0xc2 | 0xca | 0xd2 | 0xda => self.jp_f(opcode),
            0x20 | 0x28 | 0x30 | 0x38 => self.jr_f(opcode),
            0xc4 | 0xcc | 0xd4 | 0xdc => self.call_f_nn(opcode),
            0xc0 | 0xc8 | 0xd0 | 0xd8 => self.ret_f(opcode),
            0xc7 => self.rst(0x00),
            0xcf => self.rst(0x08),
            0xd7 => self.rst(0x10),
            0xdf => self.rst(0x18),
            0xe7 => self.rst(0x20),
            0xef => self.rst(0x28),
            0xf7 => self.rst(0x30),
            0xff => self.rst(0x38),
            0x76 => self.halt(),
            0x10 => self.stop(),
            0xf3 => self.di(),
            0xfb => self.ei(),
            0x3f => self.ccf(),
            0x37 => self.scf(),
            0x00 => Self::nop(),
            0x27 => self.daa(),
            0x2f => self.cpl(),
            0x01 | 0x11 | 0x21 | 0x31 => self.ld16_nn(opcode),
            0x08 => self.ld16_nn_sp(),
            0xf9 => self.ld16_sp_hl(),
            0xf8 => self.ld16_hl_sp_dd(),
            0xc5 | 0xd5 | 0xe5 | 0xf5 => self.push(opcode),
            0xc1 | 0xd1 | 0xe1 | 0xf1 => self.pop(opcode),
            0x09 | 0x19 | 0x29 | 0x39 => self.add_hl(opcode),
            0xe8 => self.add16_sp_dd(),
            0x03 | 0x13 | 0x23 | 0x33 => self.inc16(opcode),
            0x0b | 0x1b | 0x2b | 0x3b => self.dec16(opcode),
            0xcb => self.exec_cb(),
            0xd3 | 0xdb | 0xdd | 0xe3 | 0xe4 | 0xeb | 0xec | 0xed | 0xf4 | 0xfc | 0xfd => {
                panic!("Illegal opcode {}", opcode)
            }
        }
    }

    fn exec_cb(&mut self) {
        let opcode = self.imm8();
        match opcode >> 3 {
            0 => self.rlc_r(opcode),
            1 => self.rrc_r(opcode),
            2 => self.rl_r(opcode),
            3 => self.rr_r(opcode),
            4 => self.sla_r(opcode),
            5 => self.sra_r(opcode),
            6 => self.swap_r(opcode),
            7 => self.srl_r(opcode),
            _ => self.bit_r(opcode),
        }
    }
}
